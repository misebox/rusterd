//! Node placement and sizing.

use crate::ir::{GraphIR, Node};
use crate::measure::TextMetrics;
use crate::ordering::{order_levels, shuffle_levels};
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

/// Group nodes by level, in the order the source asked for.
pub fn group_nodes_by_level<'a>(
    ir: &'a GraphIR,
    node_level: &HashMap<&str, i64>,
) -> (HashMap<i64, Vec<&'a Node>>, Vec<i64>) {
    let mut levels: HashMap<i64, Vec<&Node>> = HashMap::new();

    for node in &ir.nodes {
        let level = node_level.get(node.id.as_str()).copied().unwrap_or(0);
        levels.entry(level).or_default().push(node);
    }

    for nodes in levels.values_mut() {
        nodes.sort_by_key(|n| n.order.unwrap_or(i64::MAX));
    }

    let mut level_keys: Vec<i64> = levels.keys().copied().collect();
    level_keys.sort();

    (levels, level_keys)
}

/// The same levels, reordered so that as few relationships as possible cross.
///
/// Only worth trying when the source did not say where its entities go: an
/// arrangement written by hand is followed as written.
pub fn reorder_levels<'a>(
    ir: &'a GraphIR,
    levels: &HashMap<i64, Vec<&'a Node>>,
    level_keys: &[i64],
    lone_weight: usize,
    seed: u64,
) -> HashMap<i64, Vec<&'a Node>> {
    let index: HashMap<&str, usize> = ir
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.id.as_str(), i))
        .collect();

    let mut rows: Vec<Vec<usize>> = level_keys
        .iter()
        .map(|level| {
            levels[level]
                .iter()
                .filter_map(|n| index.get(n.id.as_str()).copied())
                .collect()
        })
        .collect();
    let links: Vec<(usize, usize)> = ir
        .edges
        .iter()
        .filter_map(|edge| {
            Some((
                index.get(edge.from.as_str()).copied()?,
                index.get(edge.to.as_str()).copied()?,
            ))
        })
        .collect();

    if seed != 0 {
        shuffle_levels(&mut rows, seed);
    }
    order_levels(&mut rows, &links, lone_weight);

    level_keys
        .iter()
        .zip(rows)
        .map(|(&level, row)| (level, row.into_iter().map(|i| &ir.nodes[i]).collect()))
        .collect()
}

/// Place nodes with calculated gap widths.
#[allow(clippy::too_many_arguments)]
pub fn place_nodes(
    levels: &HashMap<i64, Vec<&Node>>,
    level_keys: &[i64],
    node_sizes: &HashMap<String, (f64, f64)>,
    gap_extra_width: &HashMap<usize, f64>,
    self_ref_reserve: &HashMap<&str, f64>,
    dynamic_channel_gap: &HashMap<i64, f64>,
    node_gap_x: f64,
    node_gap_y: f64,
    base_channel_gap: f64,
) -> NodePlacement {
    let mut layout_nodes = Vec::new();
    let mut channel_y: HashMap<i64, f64> = HashMap::new();
    let mut rows: Vec<Vec<usize>> = Vec::with_capacity(level_keys.len());
    let mut min_gap: Vec<f64> = Vec::new();
    let mut y: f64 = 40.0;
    let mut max_width: f64 = 0.0;

    for (i, &level) in level_keys.iter().enumerate() {
        let nodes_in_level = &levels[&level];
        let gap0_extra = *gap_extra_width.get(&0).unwrap_or(&0.0);
        let mut x: f64 = 40.0 + gap0_extra;
        let mut max_height: f64 = 0.0;
        let mut row: Vec<usize> = Vec::with_capacity(nodes_in_level.len());

        for (node_idx, node) in nodes_in_level.iter().enumerate() {
            let (w, h) = node_sizes[&node.id];
            row.push(layout_nodes.len());
            layout_nodes.push(LayoutNode {
                id: node.id.clone(),
                x,
                y,
                width: w,
                height: h,
            });

            let next_gap_idx = node_idx + 1;
            let extra_gap = *gap_extra_width.get(&next_gap_idx).unwrap_or(&0.0);
            // Self-reference loops hang off the right border and need room of
            // their own, whether the next thing is an entity or the SVG edge.
            let reserve = *self_ref_reserve.get(node.id.as_str()).unwrap_or(&0.0);
            let effective_gap_x = node_gap_x + extra_gap + reserve;
            min_gap.push(effective_gap_x);

            x += w + effective_gap_x;
            max_height = max_height.max(h);
        }

        rows.push(row);
        max_width = max_width.max(x - node_gap_x + 40.0);

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
        rows,
        min_gap,
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
