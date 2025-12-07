//! Edge anchor calculation for nodes.

use crate::ir::{GraphIR, Node};
use std::collections::HashMap;

use super::corridor::find_gap_center_x;
use super::types::LayoutNode;

/// Calculate edge anchor positions on nodes.
pub fn calculate_edge_anchors<'a>(
    ir: &'a GraphIR,
    node_positions: &HashMap<&str, &LayoutNode>,
    node_level: &HashMap<&str, i64>,
    edge_gap_index: &HashMap<usize, usize>,
    layout_nodes: &[LayoutNode],
    levels: &HashMap<i64, Vec<&'a Node>>,
    entity_margin: f64,
    anchor_spacing: f64,
) -> HashMap<(&'a str, bool), Vec<(usize, f64)>> {
    let mut node_exits: HashMap<(&str, bool), Vec<(usize, f64)>> = HashMap::new();

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
        let going_down = to_level >= from_level;

        let is_multi_level = (to_level - from_level).abs() > 1;
        let sort_key_x = if is_multi_level {
            if let Some(&gap_idx) = edge_gap_index.get(&idx) {
                find_gap_center_x(layout_nodes, levels, from_level + 1, gap_idx, entity_margin)
            } else {
                to_node.x + to_node.width / 2.0
            }
        } else {
            to_node.x + to_node.width / 2.0
        };

        node_exits
            .entry((edge.from.as_str(), going_down))
            .or_default()
            .push((idx, sort_key_x));

        let entry_sort_key_x = if is_multi_level {
            if let Some(&gap_idx) = edge_gap_index.get(&idx) {
                find_gap_center_x(layout_nodes, levels, to_level - 1, gap_idx, entity_margin)
            } else {
                from_node.x + from_node.width / 2.0
            }
        } else {
            from_node.x + from_node.width / 2.0
        };

        node_exits
            .entry((edge.to.as_str(), !going_down))
            .or_default()
            .push((idx, entry_sort_key_x));
    }

    // Sort and optimize anchor order
    for edges in node_exits.values_mut() {
        edges.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    }

    // Optimize exits by destination X
    optimize_exits_by_destination(ir, node_positions, &mut node_exits, anchor_spacing);

    // Optimize entries by source X
    optimize_entries_by_source(ir, node_positions, &mut node_exits, anchor_spacing);

    node_exits
}

/// Optimize exit anchor order by destination X position.
fn optimize_exits_by_destination<'a>(
    ir: &'a GraphIR,
    node_positions: &HashMap<&str, &LayoutNode>,
    node_exits: &mut HashMap<(&'a str, bool), Vec<(usize, f64)>>,
    _anchor_spacing: f64,
) {
    for ((_node_id, going_down), edges) in node_exits.iter_mut() {
        if edges.len() < 2 || !going_down {
            continue;
        }
        let dest_positions: HashMap<usize, f64> = edges
            .iter()
            .filter_map(|(idx, _)| {
                let edge = &ir.edges[*idx];
                node_positions
                    .get(edge.to.as_str())
                    .map(|n| (*idx, n.x + n.width / 2.0))
            })
            .collect();
        edges.sort_by(|a, b| {
            let dest_a = dest_positions.get(&a.0).copied().unwrap_or(a.1);
            let dest_b = dest_positions.get(&b.0).copied().unwrap_or(b.1);
            dest_a.partial_cmp(&dest_b).unwrap_or(std::cmp::Ordering::Equal)
        });
    }
}

/// Optimize entry anchor order by source X position.
fn optimize_entries_by_source<'a>(
    ir: &'a GraphIR,
    node_positions: &HashMap<&str, &LayoutNode>,
    node_exits: &mut HashMap<(&'a str, bool), Vec<(usize, f64)>>,
    _anchor_spacing: f64,
) {
    for ((_node_id, going_down), edges) in node_exits.iter_mut() {
        if edges.len() < 2 || *going_down {
            continue;
        }
        let source_positions: HashMap<usize, f64> = edges
            .iter()
            .filter_map(|(idx, _)| {
                let edge = &ir.edges[*idx];
                node_positions
                    .get(edge.from.as_str())
                    .map(|n| (*idx, n.x + n.width / 2.0))
            })
            .collect();
        edges.sort_by(|a, b| {
            let src_a = source_positions.get(&a.0).copied().unwrap_or(a.1);
            let src_b = source_positions.get(&b.0).copied().unwrap_or(b.1);
            src_a.partial_cmp(&src_b).unwrap_or(std::cmp::Ordering::Equal)
        });
    }
}
