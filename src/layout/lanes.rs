//! Lane assignment for edges in channels and corridors.

use crate::ir::{GraphIR, Node};
use std::collections::HashMap;

use super::analysis::edge_sides;
use super::corridor::{find_gap_center_x, find_safe_corridors};
use super::routing::distribute_anchor;
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
) -> HashMap<(i64, usize), usize> {
    let mut channel_lane_assignments: HashMap<(i64, usize), usize> = HashMap::new();

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

    channel_lane_assignments
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

/// Calculate corridor X positions for multi-level edges.
///
/// An edge that skips a level has to pass the entities in between somewhere.
/// It runs down whichever safe corridor comes closest to the anchor it is
/// heading for, so a target sitting in open space is reached by dropping
/// straight onto it rather than by touring the diagram.
#[allow(clippy::too_many_arguments)]
pub fn calculate_multi_level_corridor_x<'a>(
    ir: &'a GraphIR,
    node_level: &HashMap<&str, i64>,
    node_positions: &HashMap<&str, &LayoutNode>,
    node_exits: &HashMap<(&str, bool), Vec<(usize, f64)>>,
    layout_nodes: &[LayoutNode],
    levels: &HashMap<i64, Vec<&'a Node>>,
    entity_margin: f64,
    lane_spacing: f64,
    anchor_spacing: f64,
) -> HashMap<usize, f64> {
    let mut multi_level_corridor_x: HashMap<usize, f64> = HashMap::new();
    let mut corridor_groups: HashMap<(i64, i64, usize), Vec<(usize, f64)>> = HashMap::new();

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

        let to_node = match node_positions.get(edge.to.as_str()) {
            Some(n) => *n,
            None => continue,
        };
        // The corridor lines up with the anchor the edge ends on, which turns
        // the last hop into the descent itself instead of a sidestep.
        let (_, to_side) = edge_sides(from_level, to_level);
        let wanted_x = match node_exits.get(&(edge.to.as_str(), to_side)) {
            Some(exits) => {
                let pos = exits.iter().position(|(i, _)| *i == idx).unwrap_or(0);
                distribute_anchor(to_node, pos, exits.len(), anchor_spacing)
            }
            None => to_node.x + to_node.width / 2.0,
        };

        let safe_corridors =
            find_safe_corridors(layout_nodes, levels, min_level, max_level, entity_margin);

        let best_corridor_idx = safe_corridors
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let dist_a = (wanted_x.clamp(a.0, a.1) - wanted_x).abs();
                let dist_b = (wanted_x.clamp(b.0, b.1) - wanted_x).abs();
                dist_a
                    .partial_cmp(&dist_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(0);

        corridor_groups
            .entry((min_level, max_level, best_corridor_idx))
            .or_default()
            .push((idx, wanted_x));
    }

    for ((min_level, max_level, corridor_idx), edges) in &corridor_groups {
        let safe_corridors =
            find_safe_corridors(layout_nodes, levels, *min_level, *max_level, entity_margin);
        let (corridor_left, corridor_right) = safe_corridors
            .get(*corridor_idx)
            .copied()
            .unwrap_or((40.0, 200.0));

        let mut edges_sorted: Vec<(usize, f64)> = edges
            .iter()
            .map(|&(idx, wanted_x)| (idx, wanted_x.clamp(corridor_left, corridor_right)))
            .collect();
        edges_sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Edges that want the same spot — because the corridor is too narrow to
        // hold all of them where they would like to be — take a lane each.
        for i in 1..edges_sorted.len() {
            let floor = edges_sorted[i - 1].1 + lane_spacing;
            if edges_sorted[i].1 < floor {
                edges_sorted[i].1 = floor;
            }
        }

        if let (Some(&(_, first_x)), Some(&(_, last_x))) =
            (edges_sorted.first(), edges_sorted.last())
        {
            let overflow = (last_x - corridor_right).min(first_x - corridor_left);
            if overflow > 0.0 {
                for (_, x) in edges_sorted.iter_mut() {
                    *x -= overflow;
                }
            }
        }

        for (edge_idx, corridor_x) in edges_sorted {
            multi_level_corridor_x.insert(edge_idx, corridor_x);
        }
    }

    multi_level_corridor_x
}
