//! Edge anchor calculation for nodes.

use crate::ir::{GraphIR, Node};
use std::collections::HashMap;

use super::analysis::edge_sides;
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
        let (from_side, to_side) = edge_sides(from_level, to_level);

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
            .entry((edge.from.as_str(), from_side))
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
            .entry((edge.to.as_str(), to_side))
            .or_default()
            .push((idx, entry_sort_key_x));
    }

    // Every anchor list is ordered by where the other end of its edge sits, so
    // the edges leaving one border keep the same left-to-right order as the
    // entities they run to and do not cross each other on the way out.
    for edges in node_exits.values_mut() {
        edges.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    }

    node_exits
}
