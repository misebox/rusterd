//! Lane assignment for edges in channels and corridors.

use crate::ir::{GraphIR, Node};
use std::collections::HashMap;

use super::corridor::{find_gap_center_x, find_safe_corridors};
use super::routing::{calculate_lane_offset, distribute_anchor};
use super::types::LayoutNode;

/// Assign lanes for edges in channels.
#[allow(clippy::too_many_arguments)]
pub fn assign_channel_lanes<'a>(
    ir: &'a GraphIR,
    channel_edges_list: &HashMap<i64, Vec<usize>>,
    node_positions: &HashMap<&str, &LayoutNode>,
    node_level: &HashMap<&str, i64>,
    node_exits: &HashMap<(&str, bool), Vec<(usize, f64)>>,
    edge_gap_index: &HashMap<usize, usize>,
    layout_nodes: &[LayoutNode],
    levels: &HashMap<i64, Vec<&'a Node>>,
    anchor_spacing: f64,
    entity_margin: f64,
    node_gap_x: f64,
    lane_spacing: f64,
) -> (HashMap<(i64, usize), usize>, HashMap<usize, usize>) {
    let mut channel_lane_assignments: HashMap<(i64, usize), usize> = HashMap::new();
    let mut same_level_lane_assignments: HashMap<usize, usize> = HashMap::new();

    // Collect channel edges with info
    let mut channel_edges_with_info: HashMap<i64, Vec<(usize, f64, bool)>> = HashMap::new();

    for (&channel_level, edge_indices) in channel_edges_list {
        for &idx in edge_indices {
            let edge = &ir.edges[idx];
            let from_node = match node_positions.get(edge.from.as_str()) {
                Some(n) => *n,
                None => continue,
            };

            let from_level = *node_level.get(edge.from.as_str()).unwrap_or(&0);
            let to_level = *node_level.get(edge.to.as_str()).unwrap_or(&0);
            let going_down = to_level >= from_level;
            let is_going_up = to_level <= channel_level;

            let from_exits = node_exits.get(&(edge.from.as_str(), going_down));
            let from_cx = if let Some(exits) = from_exits {
                let pos = exits.iter().position(|(i, _)| *i == idx).unwrap_or(0);
                distribute_anchor(from_node, pos, exits.len(), anchor_spacing)
            } else {
                from_node.x + from_node.width / 2.0
            };

            channel_edges_with_info
                .entry(channel_level)
                .or_default()
                .push((idx, from_cx, is_going_up));
        }
    }

    // Sort and assign lanes
    for (&channel_level, edges) in channel_edges_with_info.iter_mut() {
        sort_channel_edges(
            edges,
            ir,
            node_level,
            node_positions,
            edge_gap_index,
            layout_nodes,
            levels,
            channel_level,
            entity_margin,
        );
        for (lane, (edge_idx, _, _)) in edges.iter().enumerate() {
            channel_lane_assignments.insert((channel_level, *edge_idx), lane);
        }
    }

    // Same-level edges
    assign_same_level_lanes(
        ir,
        node_positions,
        node_level,
        node_gap_x,
        lane_spacing,
        &mut same_level_lane_assignments,
    );

    (channel_lane_assignments, same_level_lane_assignments)
}

/// Assign lanes for same-level non-adjacent edges.
fn assign_same_level_lanes(
    ir: &GraphIR,
    node_positions: &HashMap<&str, &LayoutNode>,
    node_level: &HashMap<&str, i64>,
    node_gap_x: f64,
    _lane_spacing: f64,
    same_level_lane_assignments: &mut HashMap<usize, usize>,
) {
    let mut same_level_edges: HashMap<i64, Vec<(usize, f64)>> = HashMap::new();

    for (idx, edge) in ir.edges.iter().enumerate() {
        if edge.from == edge.to {
            continue;
        }
        let from_node = match node_positions.get(edge.from.as_str()) {
            Some(n) => *n,
            None => continue,
        };
        let to_node = match node_positions.get(edge.to.as_str()) {
            Some(n) => *n,
            None => continue,
        };

        let from_level = *node_level.get(edge.from.as_str()).unwrap_or(&0);
        let to_level = *node_level.get(edge.to.as_str()).unwrap_or(&0);

        if from_level == to_level {
            let (left_node, right_node) = if from_node.x < to_node.x {
                (from_node, to_node)
            } else {
                (to_node, from_node)
            };
            let gap_between = right_node.x - (left_node.x + left_node.width);

            if gap_between > node_gap_x * 1.5 {
                let from_cx = from_node.x + from_node.width / 2.0;
                same_level_edges
                    .entry(from_level)
                    .or_default()
                    .push((idx, from_cx));
            }
        }
    }

    for (_level, edges) in same_level_edges.iter_mut() {
        edges.sort_by(|a, b| match b.1.partial_cmp(&a.1) {
            Some(std::cmp::Ordering::Equal) | None => a.0.cmp(&b.0),
            Some(ord) => ord,
        });
        for (lane, (edge_idx, _)) in edges.iter().enumerate() {
            same_level_lane_assignments.insert(*edge_idx, lane);
        }
    }
}

/// Sort channel edges for lane assignment.
#[allow(clippy::too_many_arguments)]
pub fn sort_channel_edges<'a>(
    edges: &mut Vec<(usize, f64, bool)>,
    ir: &'a GraphIR,
    node_level: &HashMap<&str, i64>,
    node_positions: &HashMap<&str, &LayoutNode>,
    edge_gap_index: &HashMap<usize, usize>,
    layout_nodes: &[LayoutNode],
    levels: &HashMap<i64, Vec<&'a Node>>,
    channel_level: i64,
    entity_margin: f64,
) {
    edges.sort_by(|a, b| {
        let edge_a = &ir.edges[a.0];
        let edge_b = &ir.edges[b.0];
        let from_level_a = *node_level.get(edge_a.from.as_str()).unwrap_or(&0);
        let from_level_b = *node_level.get(edge_b.from.as_str()).unwrap_or(&0);
        let to_level_a = *node_level.get(edge_a.to.as_str()).unwrap_or(&0);
        let to_level_b = *node_level.get(edge_b.to.as_str()).unwrap_or(&0);
        let is_down_a = to_level_a > channel_level;
        let is_down_b = to_level_b > channel_level;

        let get_corridor_x = |edge_idx: usize| -> f64 {
            if let Some(&gap_idx) = edge_gap_index.get(&edge_idx) {
                find_gap_center_x(layout_nodes, levels, channel_level + 1, gap_idx, entity_margin)
            } else {
                let edge = &ir.edges[edge_idx];
                node_positions
                    .get(edge.from.as_str())
                    .map(|n| n.x + n.width / 2.0)
                    .unwrap_or(0.0)
            }
        };

        let get_to_x = |edge: &crate::ir::Edge| -> f64 {
            node_positions
                .get(edge.to.as_str())
                .map(|n| n.x + n.width / 2.0)
                .unwrap_or(0.0)
        };

        match is_down_b.cmp(&is_down_a) {
            std::cmp::Ordering::Equal => {
                let a_multi = (to_level_a - from_level_a).abs() > 1;
                let b_multi = (to_level_b - from_level_b).abs() > 1;

                if a_multi || b_multi {
                    let corridor_x_a = get_corridor_x(a.0);
                    let corridor_x_b = get_corridor_x(b.0);
                    let corridor_diff = (corridor_x_a - corridor_x_b).abs();

                    if corridor_diff > 1.0 {
                        if is_down_a {
                            corridor_x_a
                                .partial_cmp(&corridor_x_b)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        } else {
                            corridor_x_b
                                .partial_cmp(&corridor_x_a)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        }
                    } else {
                        let to_x_a = get_to_x(edge_a);
                        let to_x_b = get_to_x(edge_b);
                        let avg_to_x = (to_x_a + to_x_b) / 2.0;
                        if corridor_x_a < avg_to_x {
                            to_x_a
                                .partial_cmp(&to_x_b)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        } else {
                            to_x_b
                                .partial_cmp(&to_x_a)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        }
                    }
                } else {
                    let dist_cmp = if is_down_a {
                        to_level_a.cmp(&to_level_b)
                    } else {
                        to_level_b.cmp(&to_level_a)
                    };
                    match dist_cmp {
                        std::cmp::Ordering::Equal => {
                            let to_x_a = get_to_x(edge_a);
                            let to_x_b = get_to_x(edge_b);
                            let from_x_a = node_positions
                                .get(edge_a.from.as_str())
                                .map(|n| n.x + n.width / 2.0)
                                .unwrap_or(0.0);
                            let avg_to_x = (to_x_a + to_x_b) / 2.0;
                            if from_x_a > avg_to_x {
                                to_x_b
                                    .partial_cmp(&to_x_a)
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            } else {
                                to_x_a
                                    .partial_cmp(&to_x_b)
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            }
                        }
                        ord => ord,
                    }
                }
            }
            ord => ord,
        }
    });
}

/// Assign lanes for edges in corridors.
pub fn assign_corridor_lanes(
    corridor_edges: &HashMap<usize, Vec<usize>>,
    ir: &GraphIR,
    node_positions: &HashMap<&str, &LayoutNode>,
) -> (HashMap<(usize, usize), usize>, HashMap<usize, usize>) {
    let mut corridor_lane_assignments: HashMap<(usize, usize), usize> = HashMap::new();
    let mut corridor_total_edges: HashMap<usize, usize> = HashMap::new();

    for (&gap_idx, edge_indices) in corridor_edges {
        let mut edges_with_x: Vec<(usize, f64)> = edge_indices
            .iter()
            .filter_map(|&idx| {
                let edge = &ir.edges[idx];
                let from_node = node_positions.get(edge.from.as_str())?;
                let from_cx = from_node.x + from_node.width / 2.0;
                Some((idx, from_cx))
            })
            .collect();

        edges_with_x.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        corridor_total_edges.insert(gap_idx, edges_with_x.len());
        for (lane, (edge_idx, _)) in edges_with_x.iter().enumerate() {
            corridor_lane_assignments.insert((gap_idx, *edge_idx), lane);
        }
    }

    (corridor_lane_assignments, corridor_total_edges)
}

/// Calculate corridor X positions for multi-level edges.
pub fn calculate_multi_level_corridor_x<'a>(
    ir: &'a GraphIR,
    node_level: &HashMap<&str, i64>,
    node_positions: &HashMap<&str, &LayoutNode>,
    layout_nodes: &[LayoutNode],
    levels: &HashMap<i64, Vec<&'a Node>>,
    entity_margin: f64,
    lane_spacing: f64,
) -> HashMap<usize, f64> {
    let mut multi_level_corridor_x: HashMap<usize, f64> = HashMap::new();
    let mut corridor_groups: HashMap<(i64, i64, usize), Vec<usize>> = HashMap::new();

    for (idx, edge) in ir.edges.iter().enumerate() {
        if edge.from == edge.to {
            continue;
        }
        let from_level = *node_level.get(edge.from.as_str()).unwrap_or(&0);
        let to_level = *node_level.get(edge.to.as_str()).unwrap_or(&0);

        if (to_level - from_level).abs() <= 1 {
            continue;
        }

        let min_level = from_level.min(to_level);
        let max_level = from_level.max(to_level);

        let safe_corridors =
            find_safe_corridors(layout_nodes, levels, min_level, max_level, entity_margin);

        let from_node = match node_positions.get(edge.from.as_str()) {
            Some(n) => *n,
            None => continue,
        };
        let to_node = match node_positions.get(edge.to.as_str()) {
            Some(n) => *n,
            None => continue,
        };
        let target_x =
            (from_node.x + from_node.width / 2.0 + to_node.x + to_node.width / 2.0) / 2.0;

        let best_corridor_idx = safe_corridors
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let center_a = (a.0 + a.1.min(5000.0)) / 2.0;
                let center_b = (b.0 + b.1.min(5000.0)) / 2.0;
                let dist_a = (center_a - target_x).abs();
                let dist_b = (center_b - target_x).abs();
                dist_a
                    .partial_cmp(&dist_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(0);

        corridor_groups
            .entry((min_level, max_level, best_corridor_idx))
            .or_default()
            .push(idx);
    }

    for ((min_level, max_level, corridor_idx), edge_indices) in &corridor_groups {
        let safe_corridors =
            find_safe_corridors(layout_nodes, levels, *min_level, *max_level, entity_margin);
        let (corridor_left, corridor_right) = safe_corridors
            .get(*corridor_idx)
            .copied()
            .unwrap_or((40.0, 200.0));

        let total_lanes = edge_indices.len();
        let corridor_center = (corridor_left + corridor_right) / 2.0;

        let mut edges_sorted: Vec<(usize, f64)> = edge_indices
            .iter()
            .filter_map(|&idx| {
                let edge = &ir.edges[idx];
                let from_node = node_positions.get(edge.from.as_str())?;
                Some((idx, from_node.x + from_node.width / 2.0))
            })
            .collect();
        edges_sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        for (lane, (edge_idx, _)) in edges_sorted.iter().enumerate() {
            let lane_offset = calculate_lane_offset(lane, total_lanes, lane_spacing);
            let corridor_x = corridor_center + lane_offset;
            multi_level_corridor_x.insert(*edge_idx, corridor_x);
        }
    }

    multi_level_corridor_x
}
