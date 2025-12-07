//! Corridor computation for multi-level edge routing.

use crate::ir::Node;
use std::collections::HashMap;

use super::types::LayoutNode;

/// Find safe corridor X ranges that don't intersect any entity at intermediate levels.
/// Returns Vec of (left_x, right_x) ranges.
pub fn find_safe_corridors(
    layout_nodes: &[LayoutNode],
    levels: &HashMap<i64, Vec<&Node>>,
    min_level: i64,
    max_level: i64,
    entity_margin: f64,
) -> Vec<(f64, f64)> {
    let node_positions: HashMap<&str, &LayoutNode> = layout_nodes
        .iter()
        .map(|n| (n.id.as_str(), n))
        .collect();

    // Collect all entity boundaries across intermediate levels
    let mut all_boundaries: Vec<(f64, f64)> = Vec::new();

    for level in (min_level + 1)..max_level {
        if let Some(nodes_at_level) = levels.get(&level) {
            for node in nodes_at_level {
                if let Some(layout_node) = node_positions.get(node.id.as_str()) {
                    all_boundaries.push((
                        layout_node.x - entity_margin,
                        layout_node.x + layout_node.width + entity_margin,
                    ));
                }
            }
        }
    }

    // Sort and merge overlapping boundaries
    all_boundaries.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut merged: Vec<(f64, f64)> = Vec::new();
    for (left, right) in all_boundaries {
        if let Some(last) = merged.last_mut() {
            if left <= last.1 {
                last.1 = last.1.max(right);
            } else {
                merged.push((left, right));
            }
        } else {
            merged.push((left, right));
        }
    }

    // Find gaps between merged boundaries
    let mut gaps: Vec<(f64, f64)> = Vec::new();

    if let Some(&(first_left, _)) = merged.first() {
        if first_left > 40.0 {
            gaps.push((40.0, first_left));
        }
    } else {
        gaps.push((40.0, 10000.0));
    }

    for i in 0..merged.len().saturating_sub(1) {
        let gap_left = merged[i].1;
        let gap_right = merged[i + 1].0;
        if gap_right > gap_left {
            gaps.push((gap_left, gap_right));
        }
    }

    if let Some(&(_, last_right)) = merged.last() {
        gaps.push((last_right, 10000.0));
    }

    gaps
}

/// Find the center X coordinate of a specific gap at a given level.
/// gap_index: 0 = before first entity, 1 = between first and second, etc.
pub fn find_gap_center_x(
    layout_nodes: &[LayoutNode],
    levels: &HashMap<i64, Vec<&Node>>,
    level: i64,
    gap_index: usize,
    entity_margin: f64,
) -> f64 {
    let nodes_at_level = match levels.get(&level) {
        Some(nodes) => nodes,
        None => return 100.0,
    };

    let node_positions: HashMap<&str, &LayoutNode> = layout_nodes
        .iter()
        .map(|n| (n.id.as_str(), n))
        .collect();

    let mut boundaries: Vec<(f64, f64)> = Vec::new();

    for node in nodes_at_level {
        if let Some(layout_node) = node_positions.get(node.id.as_str()) {
            boundaries.push((layout_node.x, layout_node.x + layout_node.width));
        }
    }

    boundaries.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    if boundaries.is_empty() {
        return 100.0;
    }

    if gap_index == 0 {
        let first_left = boundaries[0].0;
        return (40.0 + first_left) / 2.0;
    }

    if gap_index >= boundaries.len() {
        if let Some(&(_, last_right)) = boundaries.last() {
            return last_right + entity_margin + 50.0;
        }
    }

    if gap_index > 0 && gap_index < boundaries.len() {
        let left_entity_right = boundaries[gap_index - 1].1;
        let right_entity_left = boundaries[gap_index].0;
        return (left_entity_right + right_entity_left) / 2.0;
    }

    if gap_index < boundaries.len().saturating_sub(1) {
        let left_entity_right = boundaries[gap_index].1;
        let right_entity_left = boundaries[gap_index + 1].0;
        return (left_entity_right + right_entity_left) / 2.0;
    }

    100.0
}
