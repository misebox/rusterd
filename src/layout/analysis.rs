//! Edge and corridor analysis for layout computation.

#![allow(dead_code)]

use crate::ir::GraphIR;
use std::collections::HashMap;

use super::types::CorridorAnalysis;

/// Build node level lookup: node_id -> level.
pub fn build_node_level_lookup(ir: &GraphIR) -> HashMap<&str, i64> {
    ir.nodes
        .iter()
        .map(|n| (n.id.as_str(), n.level.unwrap_or(0)))
        .collect()
}

/// Count edges per node per direction.
/// Returns: (node_id, going_down) -> edge count
pub fn count_edges_per_node<'a>(
    ir: &'a GraphIR,
    node_level: &HashMap<&str, i64>,
) -> HashMap<(&'a str, bool), usize> {
    let mut edge_count: HashMap<(&str, bool), usize> = HashMap::new();

    for edge in &ir.edges {
        if edge.from == edge.to {
            continue;
        }
        let from_level = *node_level.get(edge.from.as_str()).unwrap_or(&0);
        let to_level = *node_level.get(edge.to.as_str()).unwrap_or(&0);
        let going_down = to_level >= from_level;

        *edge_count.entry((edge.from.as_str(), going_down)).or_insert(0) += 1;
        *edge_count.entry((edge.to.as_str(), !going_down)).or_insert(0) += 1;
    }

    edge_count
}

/// Analyze which edges pass through which channels.
/// Channel N is between level N and level N+1.
pub fn analyze_channel_edges(
    ir: &GraphIR,
    node_level: &HashMap<&str, i64>,
) -> (HashMap<i64, Vec<usize>>, HashMap<i64, usize>) {
    let mut channel_edges: HashMap<i64, Vec<usize>> = HashMap::new();

    for (idx, edge) in ir.edges.iter().enumerate() {
        if edge.from == edge.to {
            continue;
        }
        let from_level = *node_level.get(edge.from.as_str()).unwrap_or(&0);
        let to_level = *node_level.get(edge.to.as_str()).unwrap_or(&0);
        if from_level == to_level {
            continue;
        }

        let min_level = from_level.min(to_level);
        let max_level = from_level.max(to_level);

        for channel_level in min_level..max_level {
            channel_edges.entry(channel_level).or_default().push(idx);
        }
    }

    let channel_edge_count: HashMap<i64, usize> = channel_edges
        .iter()
        .map(|(&k, v)| (k, v.len()))
        .collect();

    (channel_edges, channel_edge_count)
}

/// Calculate dynamic channel gaps based on edge count.
pub fn calculate_dynamic_channel_gaps(
    level_keys: &[i64],
    channel_edge_count: &HashMap<i64, usize>,
    entity_margin: f64,
    lane_spacing: f64,
    base_channel_gap: f64,
) -> HashMap<i64, f64> {
    let mut dynamic_gaps: HashMap<i64, f64> = HashMap::new();

    for (i, &level) in level_keys.iter().enumerate() {
        if i < level_keys.len() - 1 {
            let edge_count = *channel_edge_count.get(&level).unwrap_or(&0);
            let needed_space =
                entity_margin * 2.0 + (edge_count.saturating_sub(1) as f64) * lane_spacing;
            let gap = needed_space.max(base_channel_gap);
            dynamic_gaps.insert(level, gap);
        }
    }

    dynamic_gaps
}

/// Build node order lookup: node_id -> order within level.
pub fn build_node_order<'a>(
    levels: &HashMap<i64, Vec<&'a crate::ir::Node>>,
) -> HashMap<&'a str, usize> {
    let mut node_order: HashMap<&str, usize> = HashMap::new();

    for nodes_in_level in levels.values() {
        for (idx, node) in nodes_in_level.iter().enumerate() {
            node_order.insert(node.id.as_str(), idx);
        }
    }

    node_order
}

/// Analyze corridor requirements for multi-level edges.
pub fn analyze_corridors(
    ir: &GraphIR,
    node_level: &HashMap<&str, i64>,
    node_order: &HashMap<&str, usize>,
    lane_spacing: f64,
) -> CorridorAnalysis {
    let mut corridor_edges: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut edge_gap_index: HashMap<usize, usize> = HashMap::new();

    for (idx, edge) in ir.edges.iter().enumerate() {
        if edge.from == edge.to {
            continue;
        }
        let from_level = *node_level.get(edge.from.as_str()).unwrap_or(&0);
        let to_level = *node_level.get(edge.to.as_str()).unwrap_or(&0);

        // Only multi-level edges need corridor routing
        if (to_level - from_level).abs() <= 1 {
            continue;
        }

        let from_order = node_order.get(edge.from.as_str()).copied().unwrap_or(0);
        let to_order = node_order.get(edge.to.as_str()).copied().unwrap_or(0);

        let gap_index = if from_order <= to_order {
            (from_order + 1).min(to_order)
        } else {
            from_order.max(to_order + 1)
        };

        edge_gap_index.insert(idx, gap_index);
        corridor_edges.entry(gap_index).or_default().push(idx);
    }

    let mut gap_extra_width: HashMap<usize, f64> = HashMap::new();
    for (&gap_idx, edges) in &corridor_edges {
        let extra = edges.len() as f64 * lane_spacing;
        gap_extra_width.insert(gap_idx, extra);
    }

    CorridorAnalysis {
        corridor_edges,
        edge_gap_index,
        gap_extra_width,
    }
}

