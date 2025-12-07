//! Edge routing and waypoint generation.

use crate::ir::{GraphIR, Node};
use std::collections::HashMap;

use super::corridor::{find_gap_center_x, find_safe_corridors};
use super::routing::{
    calculate_lane_offset, distribute_anchor, route_adjacent_level_direct,
    route_adjacent_level_with_channel, route_same_level_adjacent, route_self_ref,
};
use super::types::{LayoutEdge, LayoutNode, NodePlacement};

/// Route all edges and generate waypoints.
#[allow(clippy::too_many_arguments)]
pub fn route_edges<'a>(
    ir: &'a GraphIR,
    node_positions: &HashMap<&str, &LayoutNode>,
    node_level: &HashMap<&str, i64>,
    node_exits: &HashMap<(&str, bool), Vec<(usize, f64)>>,
    node_order: &HashMap<&str, usize>,
    channel_edge_count: &HashMap<i64, usize>,
    channel_lane_assignments: &HashMap<(i64, usize), usize>,
    same_level_lane_assignments: &HashMap<usize, usize>,
    node_placement: &NodePlacement,
    levels: &HashMap<i64, Vec<&'a Node>>,
    multi_level_corridor_x: &HashMap<usize, f64>,
    anchor_spacing: f64,
    lane_spacing: f64,
    channel_gap: f64,
    node_gap_x: f64,
    entity_margin: f64,
) -> Vec<LayoutEdge> {
    ir.edges
        .iter()
        .enumerate()
        .filter_map(|(idx, edge)| {
            let from_node = node_positions.get(edge.from.as_str())?;
            let to_node = node_positions.get(edge.to.as_str())?;

            if edge.from == edge.to {
                return Some(LayoutEdge {
                    from: edge.from.clone(),
                    to: edge.to.clone(),
                    waypoints: route_self_ref(from_node),
                    is_self_ref: true,
                    edge_index: idx,
                });
            }

            let from_level = *node_level.get(edge.from.as_str()).unwrap_or(&0);
            let to_level = *node_level.get(edge.to.as_str()).unwrap_or(&0);
            let going_down = to_level >= from_level;

            let from_exits = node_exits.get(&(edge.from.as_str(), going_down))?;
            let from_pos = from_exits.iter().position(|(i, _)| *i == idx).unwrap_or(0);
            let from_cx = distribute_anchor(from_node, from_pos, from_exits.len(), anchor_spacing);

            let to_exits = node_exits.get(&(edge.to.as_str(), !going_down))?;
            let to_pos = to_exits.iter().position(|(i, _)| *i == idx).unwrap_or(0);
            let to_cx = distribute_anchor(to_node, to_pos, to_exits.len(), anchor_spacing);

            let waypoints = calculate_waypoints(
                idx,
                edge,
                from_node,
                to_node,
                from_cx,
                to_cx,
                from_level,
                to_level,
                node_order,
                channel_edge_count,
                channel_lane_assignments,
                same_level_lane_assignments,
                node_placement,
                levels,
                multi_level_corridor_x,
                lane_spacing,
                channel_gap,
                node_gap_x,
                entity_margin,
            );

            Some(LayoutEdge {
                from: edge.from.clone(),
                to: edge.to.clone(),
                waypoints,
                is_self_ref: false,
                edge_index: idx,
            })
        })
        .collect()
}

/// Calculate waypoints for a single edge.
#[allow(clippy::too_many_arguments)]
fn calculate_waypoints<'a>(
    idx: usize,
    edge: &crate::ir::Edge,
    from_node: &LayoutNode,
    to_node: &LayoutNode,
    from_cx: f64,
    to_cx: f64,
    from_level: i64,
    to_level: i64,
    node_order: &HashMap<&str, usize>,
    channel_edge_count: &HashMap<i64, usize>,
    channel_lane_assignments: &HashMap<(i64, usize), usize>,
    same_level_lane_assignments: &HashMap<usize, usize>,
    node_placement: &NodePlacement,
    levels: &HashMap<i64, Vec<&'a Node>>,
    multi_level_corridor_x: &HashMap<usize, f64>,
    lane_spacing: f64,
    channel_gap: f64,
    node_gap_x: f64,
    entity_margin: f64,
) -> Vec<(f64, f64)> {
    let channel_level = from_level.min(to_level);
    let total_edges = *channel_edge_count.get(&channel_level).unwrap_or(&1);
    let lane = *channel_lane_assignments
        .get(&(channel_level, idx))
        .unwrap_or(&0);
    let lane_offset = calculate_lane_offset(lane, total_edges, lane_spacing);

    if from_level == to_level {
        route_same_level(
            idx,
            edge,
            from_node,
            to_node,
            from_cx,
            to_cx,
            from_level,
            node_order,
            same_level_lane_assignments,
            node_placement,
            levels,
            lane_spacing,
            channel_gap,
            node_gap_x,
            entity_margin,
        )
    } else {
        let min_level = from_level.min(to_level);
        let max_level = from_level.max(to_level);
        let going_down = to_level > from_level;

        if max_level - min_level == 1 {
            route_adjacent_level(
                from_node,
                to_node,
                from_cx,
                to_cx,
                min_level,
                going_down,
                lane_offset,
                node_placement,
                channel_gap,
            )
        } else {
            route_multi_level(
                idx,
                from_node,
                to_node,
                from_cx,
                to_cx,
                from_level,
                to_level,
                min_level,
                max_level,
                going_down,
                channel_edge_count,
                channel_lane_assignments,
                node_placement,
                levels,
                multi_level_corridor_x,
                lane_spacing,
                channel_gap,
                entity_margin,
            )
        }
    }
}

/// Route same-level edges.
#[allow(clippy::too_many_arguments)]
fn route_same_level<'a>(
    idx: usize,
    edge: &crate::ir::Edge,
    from_node: &LayoutNode,
    to_node: &LayoutNode,
    from_cx: f64,
    to_cx: f64,
    from_level: i64,
    node_order: &HashMap<&str, usize>,
    same_level_lane_assignments: &HashMap<usize, usize>,
    node_placement: &NodePlacement,
    levels: &HashMap<i64, Vec<&'a Node>>,
    lane_spacing: f64,
    channel_gap: f64,
    node_gap_x: f64,
    entity_margin: f64,
) -> Vec<(f64, f64)> {
    let (left_node, right_node) = if from_node.x < to_node.x {
        (from_node, to_node)
    } else {
        (to_node, from_node)
    };
    let gap_between = right_node.x - (left_node.x + left_node.width);

    if gap_between <= node_gap_x * 1.5 {
        route_same_level_adjacent(from_node, to_node)
    } else {
        let same_level_lane = *same_level_lane_assignments.get(&idx).unwrap_or(&0);
        let same_level_lane_offset = same_level_lane as f64 * lane_spacing;

        let from_order = node_order.get(edge.from.as_str()).copied().unwrap_or(0);
        let to_order = node_order.get(edge.to.as_str()).copied().unwrap_or(0);
        let corridor_gap = if from_order < to_order {
            from_order + 1
        } else {
            to_order + 1
        };

        let corridor_x = find_gap_center_x(
            &node_placement.layout_nodes,
            levels,
            from_level,
            corridor_gap,
            entity_margin,
        ) + same_level_lane_offset;

        let ch_y = *node_placement
            .channel_y
            .get(&from_level)
            .unwrap_or(&(from_node.y + from_node.height + channel_gap / 2.0));

        vec![
            (from_cx, from_node.y + from_node.height),
            (from_cx, ch_y),
            (corridor_x, ch_y),
            (corridor_x, to_node.y + to_node.height),
            (to_cx, to_node.y + to_node.height),
        ]
    }
}

/// Route adjacent-level edges.
#[allow(clippy::too_many_arguments)]
fn route_adjacent_level(
    from_node: &LayoutNode,
    to_node: &LayoutNode,
    from_cx: f64,
    to_cx: f64,
    min_level: i64,
    going_down: bool,
    lane_offset: f64,
    node_placement: &NodePlacement,
    channel_gap: f64,
) -> Vec<(f64, f64)> {
    let direct_threshold = 1.0;

    if (from_cx - to_cx).abs() < direct_threshold {
        route_adjacent_level_direct(from_node, to_node, from_cx, to_cx, going_down)
    } else {
        let upper_node = if going_down { from_node } else { to_node };
        let ch_y = *node_placement
            .channel_y
            .get(&min_level)
            .unwrap_or(&(upper_node.y + upper_node.height + channel_gap / 2.0))
            + lane_offset;

        route_adjacent_level_with_channel(from_node, to_node, from_cx, to_cx, ch_y, going_down)
    }
}

/// Route multi-level edges through corridors.
#[allow(clippy::too_many_arguments)]
fn route_multi_level<'a>(
    idx: usize,
    from_node: &LayoutNode,
    to_node: &LayoutNode,
    from_cx: f64,
    to_cx: f64,
    from_level: i64,
    to_level: i64,
    min_level: i64,
    max_level: i64,
    going_down: bool,
    channel_edge_count: &HashMap<i64, usize>,
    channel_lane_assignments: &HashMap<(i64, usize), usize>,
    node_placement: &NodePlacement,
    levels: &HashMap<i64, Vec<&'a Node>>,
    multi_level_corridor_x: &HashMap<usize, f64>,
    lane_spacing: f64,
    channel_gap: f64,
    entity_margin: f64,
) -> Vec<(f64, f64)> {
    let corridor_x = multi_level_corridor_x.get(&idx).copied().unwrap_or_else(|| {
        let safe_corridors = find_safe_corridors(
            &node_placement.layout_nodes,
            levels,
            min_level,
            max_level,
            entity_margin,
        );
        safe_corridors
            .first()
            .map(|(l, r)| (l + r) / 2.0)
            .unwrap_or(100.0)
    });

    let get_channel_lane_offset = |ch_level: i64| -> f64 {
        let ch_total = *channel_edge_count.get(&ch_level).unwrap_or(&1);
        let ch_lane = *channel_lane_assignments
            .get(&(ch_level, idx))
            .unwrap_or(&0);
        calculate_lane_offset(ch_lane, ch_total, lane_spacing)
    };

    let mut waypoints = Vec::new();

    if going_down {
        waypoints.push((from_cx, from_node.y + from_node.height));

        let first_ch_level = from_level;
        let first_lane_offset = get_channel_lane_offset(first_ch_level);
        let first_ch_y = *node_placement
            .channel_y
            .get(&first_ch_level)
            .unwrap_or(&(from_node.y + from_node.height + channel_gap / 2.0))
            + first_lane_offset;
        waypoints.push((from_cx, first_ch_y));
        waypoints.push((corridor_x, first_ch_y));

        for level in (from_level + 1)..to_level {
            let ch_lane_offset = get_channel_lane_offset(level);
            let ch_y = *node_placement
                .channel_y
                .get(&level)
                .unwrap_or(&(first_ch_y + channel_gap))
                + ch_lane_offset;
            waypoints.push((corridor_x, ch_y));
        }

        let last_ch_level = to_level - 1;
        let last_lane_offset = get_channel_lane_offset(last_ch_level);
        let last_ch_y = *node_placement
            .channel_y
            .get(&last_ch_level)
            .unwrap_or(&(to_node.y - channel_gap / 2.0))
            + last_lane_offset;
        if waypoints.last().map(|(_, y)| *y) != Some(last_ch_y) {
            waypoints.push((corridor_x, last_ch_y));
        }
        waypoints.push((to_cx, last_ch_y));
        waypoints.push((to_cx, to_node.y));
    } else {
        waypoints.push((from_cx, from_node.y));

        let first_ch_level = from_level - 1;
        let first_lane_offset = get_channel_lane_offset(first_ch_level);
        let first_ch_y = *node_placement
            .channel_y
            .get(&first_ch_level)
            .unwrap_or(&(from_node.y - channel_gap / 2.0))
            + first_lane_offset;
        waypoints.push((from_cx, first_ch_y));
        waypoints.push((corridor_x, first_ch_y));

        for level in (to_level..(from_level - 1)).rev() {
            let ch_lane_offset = get_channel_lane_offset(level);
            let ch_y = *node_placement
                .channel_y
                .get(&level)
                .unwrap_or(&(first_ch_y - channel_gap))
                + ch_lane_offset;
            waypoints.push((corridor_x, ch_y));
        }

        let last_ch_level = to_level;
        let last_lane_offset = get_channel_lane_offset(last_ch_level);
        let last_ch_y = *node_placement
            .channel_y
            .get(&last_ch_level)
            .unwrap_or(&(to_node.y + to_node.height + channel_gap / 2.0))
            + last_lane_offset;
        if waypoints.last().map(|(_, y)| *y) != Some(last_ch_y) {
            waypoints.push((corridor_x, last_ch_y));
        }
        waypoints.push((to_cx, last_ch_y));
        waypoints.push((to_cx, to_node.y + to_node.height));
    }

    waypoints
}
