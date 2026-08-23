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

/// Pick the corridor that comes closest to `wanted`, and the point in it that
/// gets there. A corridor containing `wanted` wins outright, which is what lets
/// an edge run straight down onto what it is aiming for.
pub fn nearest_corridor(corridors: &[(f64, f64)], wanted: f64) -> Option<(usize, f64)> {
    corridors
        .iter()
        .enumerate()
        .map(|(i, &(left, right))| (i, wanted.clamp(left, right)))
        .min_by(|(_, a), (_, b)| {
            (a - wanted)
                .abs()
                .partial_cmp(&(b - wanted).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}
