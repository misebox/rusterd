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

/// Group nodes by level, in the order they were written.
pub fn group_nodes_by_level<'a>(
    ir: &'a GraphIR,
    node_level: &HashMap<&str, i64>,
) -> (HashMap<i64, Vec<&'a Node>>, Vec<i64>) {
    let mut levels: HashMap<i64, Vec<&Node>> = HashMap::new();

    for node in &ir.nodes {
        let level = node_level.get(node.id.as_str()).copied().unwrap_or(0);
        levels.entry(level).or_default().push(node);
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
    let attractions = attractions(ir, &index);

    if seed != 0 {
        shuffle_levels(&mut rows, seed);
    }
    order_levels(&mut rows, &links, &attractions, lone_weight);

    level_keys
        .iter()
        .zip(rows)
        .map(|(&level, row)| (level, row.into_iter().map(|i| &ir.nodes[i]).collect()))
        .collect()
}

/// Every pair of entities the source asked to keep near one another.
pub fn attractions(ir: &GraphIR, index: &HashMap<&str, usize>) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    for set in &ir.near {
        for (i, one) in set.iter().enumerate() {
            for other in &set[i + 1..] {
                if let (Some(&a), Some(&b)) = (index.get(one.as_str()), index.get(other.as_str())) {
                    pairs.push((a, b));
                }
            }
        }
    }
    pairs
}

/// Place the entities across the page, level by level, packed to the left.
///
/// Only the columns are settled here. Where the rows fall depends on how many
/// edges end up needing room between them, which is not known until the edges
/// have been planned — and none of that planning needs to know the rows.
#[allow(clippy::too_many_arguments)]
pub fn place_columns(
    levels: &HashMap<i64, Vec<&Node>>,
    level_keys: &[i64],
    node_sizes: &HashMap<String, (f64, f64)>,
    gap_extra_width: &HashMap<usize, f64>,
    self_ref_reserve: &HashMap<&str, f64>,
    node_gap_x: f64,
) -> NodePlacement {
    let mut layout_nodes = Vec::new();
    let mut rows: Vec<Vec<usize>> = Vec::with_capacity(level_keys.len());
    let mut min_gap: Vec<f64> = Vec::new();
    let mut max_width: f64 = 0.0;

    for &level in level_keys {
        let nodes_in_level = &levels[&level];
        let gap0_extra = *gap_extra_width.get(&0).unwrap_or(&0.0);
        let mut x: f64 = 40.0 + gap0_extra;
        let mut row: Vec<usize> = Vec::with_capacity(nodes_in_level.len());

        for (node_idx, node) in nodes_in_level.iter().enumerate() {
            let (w, h) = node_sizes[&node.id];
            row.push(layout_nodes.len());
            layout_nodes.push(LayoutNode {
                id: node.id.clone(),
                x,
                y: 0.0,
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
        }

        rows.push(row);
        max_width = max_width.max(x - node_gap_x + 40.0);
    }

    NodePlacement {
        layout_nodes,
        rows,
        min_gap,
        channel_y: HashMap::new(),
        max_width,
        total_height: 0.0,
    }
}

/// Settle the rows down the page, once it is known how much room the edges
/// between them need.
///
/// A level is as tall as its tallest entity, and the shorter ones do not have
/// to hang from the top of it. An entity with more relations below than above
/// sits at the foot of its level, where its children are; one with more above
/// sits at the head. That alone takes a couple of hundred pixels out of the
/// line from a short entity to its child.
pub fn place_rows(
    placement: &mut NodePlacement,
    level_keys: &[i64],
    channel_gap: &HashMap<i64, f64>,
    node_gap_y: f64,
    base_channel_gap: f64,
    stands_low: &HashMap<&str, f64>,
) {
    let mut y: f64 = 40.0;

    for (i, &level) in level_keys.iter().enumerate() {
        let row = placement.rows[i].clone();
        let band = row
            .iter()
            .map(|&node| placement.layout_nodes[node].height)
            .fold(0.0, f64::max);

        for &node in &row {
            let entity = &mut placement.layout_nodes[node];
            let low = stands_low
                .get(entity.id.as_str())
                .copied()
                .unwrap_or(HALFWAY);
            entity.y = y + (band - entity.height) * low;
        }

        if i < level_keys.len() - 1 {
            let gap = *channel_gap.get(&level).unwrap_or(&base_channel_gap);
            let total_space = node_gap_y + gap;
            placement
                .channel_y
                .insert(level, y + band + total_space / 2.0);
            y += band + total_space;
        } else {
            y += band + node_gap_y;
        }
    }

    placement.total_height = y - node_gap_y + 40.0;
}

/// Where in its level each entity stands: 0 at the head, 1 at the foot.
pub fn stand_low<'a>(
    ir: &'a GraphIR,
    edges_per_border: &HashMap<(&str, bool), usize>,
) -> HashMap<&'a str, f64> {
    ir.nodes
        .iter()
        .map(|node| {
            let below = edges_per_border
                .get(&(node.id.as_str(), true))
                .copied()
                .unwrap_or(0);
            let above = edges_per_border
                .get(&(node.id.as_str(), false))
                .copied()
                .unwrap_or(0);
            let low = match below.cmp(&above) {
                std::cmp::Ordering::Greater => 1.0,
                std::cmp::Ordering::Less => 0.0,
                std::cmp::Ordering::Equal => HALFWAY,
            };
            (node.id.as_str(), low)
        })
        .collect()
}

/// Where an entity with as much above it as below stands in its level.
const HALFWAY: f64 = 0.5;

/// Build node position lookup from layout nodes.
pub fn build_node_positions(layout_nodes: &[LayoutNode]) -> HashMap<&str, &LayoutNode> {
    layout_nodes.iter().map(|n| (n.id.as_str(), n)).collect()
}
