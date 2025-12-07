//! Node placement and sizing.

use crate::ir::{GraphIR, Node};
use crate::measure::TextMetrics;
use std::collections::HashMap;

use super::types::{LayoutNode, NodePlacement};

/// Calculate node sizes based on content and anchor requirements.
pub fn calculate_node_sizes(
    ir: &GraphIR,
    edge_count_per_node: &HashMap<(&str, bool), usize>,
    metrics: &TextMetrics,
    anchor_spacing: f64,
) -> HashMap<String, (f64, f64)> {
    let mut node_sizes: HashMap<String, (f64, f64)> = HashMap::new();

    for node in &ir.nodes {
        let columns: Vec<(String, String)> = node
            .columns
            .iter()
            .map(|c| (c.name.clone(), c.typ.clone()))
            .collect();
        let (content_w, h) = metrics.node_size(&node.label, &columns);

        let down_edges = *edge_count_per_node
            .get(&(node.id.as_str(), true))
            .unwrap_or(&0);
        let up_edges = *edge_count_per_node
            .get(&(node.id.as_str(), false))
            .unwrap_or(&0);
        let max_edges = down_edges.max(up_edges);

        let anchor_width = if max_edges > 1 {
            (max_edges - 1) as f64 * anchor_spacing + anchor_spacing
        } else {
            0.0
        };

        let w = content_w.max(anchor_width);
        node_sizes.insert(node.id.clone(), (w, h));
    }

    node_sizes
}

/// Group nodes by level and sort within each level.
pub fn group_nodes_by_level(ir: &GraphIR) -> (HashMap<i64, Vec<&Node>>, Vec<i64>) {
    let mut levels: HashMap<i64, Vec<&Node>> = HashMap::new();

    for node in &ir.nodes {
        let level = node.level.unwrap_or(0);
        levels.entry(level).or_default().push(node);
    }

    for nodes in levels.values_mut() {
        nodes.sort_by_key(|n| n.order.unwrap_or(i64::MAX));
    }

    let mut level_keys: Vec<i64> = levels.keys().copied().collect();
    level_keys.sort();

    (levels, level_keys)
}

/// Place nodes with calculated gap widths.
pub fn place_nodes(
    levels: &HashMap<i64, Vec<&Node>>,
    level_keys: &[i64],
    node_sizes: &HashMap<String, (f64, f64)>,
    gap_extra_width: &HashMap<usize, f64>,
    dynamic_channel_gap: &HashMap<i64, f64>,
    node_gap_x: f64,
    node_gap_y: f64,
    base_channel_gap: f64,
) -> NodePlacement {
    let mut layout_nodes = Vec::new();
    let mut level_bottom_y: HashMap<i64, f64> = HashMap::new();
    let mut channel_y: HashMap<i64, f64> = HashMap::new();
    let mut y: f64 = 40.0;
    let mut max_width: f64 = 0.0;

    for (i, &level) in level_keys.iter().enumerate() {
        let nodes_in_level = &levels[&level];
        let gap0_extra = *gap_extra_width.get(&0).unwrap_or(&0.0);
        let mut x: f64 = 40.0 + gap0_extra;
        let mut max_height: f64 = 0.0;

        for (node_idx, node) in nodes_in_level.iter().enumerate() {
            let (w, h) = node_sizes[&node.id];
            layout_nodes.push(LayoutNode {
                id: node.id.clone(),
                x,
                y,
                width: w,
                height: h,
            });

            let next_gap_idx = node_idx + 1;
            let extra_gap = *gap_extra_width.get(&next_gap_idx).unwrap_or(&0.0);
            let effective_gap_x = node_gap_x + extra_gap;

            x += w + effective_gap_x;
            max_height = max_height.max(h);
        }

        max_width = max_width.max(x - node_gap_x + 40.0);
        level_bottom_y.insert(level, y + max_height);

        if i < level_keys.len() - 1 {
            let gap = *dynamic_channel_gap.get(&level).unwrap_or(&base_channel_gap);
            let total_space = node_gap_y + gap;
            let channel_center = y + max_height + total_space / 2.0;
            channel_y.insert(level, channel_center);
            y += max_height + total_space;
        } else {
            y += max_height + node_gap_y;
        }
    }

    let total_height = y - node_gap_y + 40.0;

    NodePlacement {
        layout_nodes,
        level_bottom_y,
        channel_y,
        max_width,
        total_height,
    }
}

/// Build node position lookup from layout nodes.
pub fn build_node_positions(layout_nodes: &[LayoutNode]) -> HashMap<&str, &LayoutNode> {
    layout_nodes
        .iter()
        .map(|n| (n.id.as_str(), n))
        .collect()
}
