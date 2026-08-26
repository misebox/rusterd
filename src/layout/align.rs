//! Horizontal alignment of the entities within each level.
//!
//! Placement packs every level against the left margin, which leaves an entity
//! sitting under whichever entity happens to share its index rather than under
//! the one it is joined to. This pass slides each entity towards the average of
//! the entities it relates to, so that edges run down instead of across. The
//! order of a level and the gaps placement reserved are kept, and the drawing
//! is never made wider than the packed one it started from.

use crate::ir::GraphIR;
use std::collections::HashMap;

use super::fit::fit_in_order;
use super::placement::attractions;
use super::types::NodePlacement;

/// Rounds of relaxation. Each entity follows its neighbours, which have moved
/// in the previous round, so a few passes are needed before the levels settle.
const ROUNDS: usize = 6;

/// Margin between the drawing and the edge of the canvas.
const MARGIN: f64 = 40.0;

pub fn align_levels(placement: &mut NodePlacement, ir: &GraphIR, node_gap_x: f64) {
    // Space each entity keeps to its right beyond the ordinary gap, for a
    // self-reference loop and the labels hanging off it.
    let right_pad: Vec<f64> = placement
        .min_gap
        .iter()
        .map(|gap| (gap - node_gap_x).max(0.0))
        .collect();
    let width_budget = placement.max_width;

    let index: HashMap<&str, usize> = placement
        .layout_nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    let mut level_of = vec![0usize; placement.layout_nodes.len()];
    for (level, row) in placement.rows.iter().enumerate() {
        for &node in row {
            level_of[node] = level;
        }
    }

    // Entities on the same level pull sideways rather than up or down, so only
    // relations that cross a level guide the alignment. Entities asked to be
    // near one another pull the same way, whether or not anything relates them.
    let mut neighbours: Vec<Vec<usize>> = vec![Vec::new(); placement.layout_nodes.len()];
    let related = ir.edges.iter().filter_map(|edge| {
        Some((
            *index.get(edge.from.as_str())?,
            *index.get(edge.to.as_str())?,
        ))
    });
    for (from, to) in related.chain(attractions(ir, &index)) {
        if from == to || level_of[from] == level_of[to] {
            continue;
        }
        neighbours[from].push(to);
        neighbours[to].push(from);
    }

    for _ in 0..ROUNDS {
        for row in 0..placement.rows.len() {
            align_row(placement, &neighbours, &right_pad, row, width_budget);
        }
    }

    settle(placement, &right_pad);
}

/// Move one row's entities as close as possible to the centre of the entities
/// they relate to, without reordering them or closing the gaps between them.
fn align_row(
    placement: &mut NodePlacement,
    neighbours: &[Vec<usize>],
    right_pad: &[f64],
    row: usize,
    width_budget: f64,
) {
    let nodes = placement.rows[row].clone();
    let Some(&last) = nodes.last() else {
        return;
    };

    let wanted: Vec<f64> = nodes
        .iter()
        .map(|&node| {
            let linked = &neighbours[node];
            if linked.is_empty() {
                let n = &placement.layout_nodes[node];
                return n.x + n.width / 2.0;
            }
            let sum: f64 = linked
                .iter()
                .map(|&other| {
                    let o = &placement.layout_nodes[other];
                    o.x + o.width / 2.0
                })
                .sum();
            sum / linked.len() as f64
        })
        .collect();

    // Aligning a level may move and spread it, but must not widen the drawing:
    // the far end is bounded by where the packed layout reached.
    let span: Vec<f64> = nodes
        .iter()
        .map(|&node| placement.layout_nodes[node].width + placement.min_gap[node])
        .collect();
    let left_edges: Vec<f64> = nodes
        .iter()
        .zip(&wanted)
        .map(|(&node, want)| want - placement.layout_nodes[node].width / 2.0)
        .collect();
    let placed = fit_in_order(
        &left_edges,
        &span,
        MARGIN,
        width_budget - MARGIN - right_pad[last] - placement.layout_nodes[last].width,
    );

    for (&node, x) in nodes.iter().zip(placed) {
        placement.layout_nodes[node].x = x;
    }
}

/// Bring the whole drawing back against the left margin and re-measure it.
fn settle(placement: &mut NodePlacement, right_pad: &[f64]) {
    let leftmost = placement
        .layout_nodes
        .iter()
        .map(|n| n.x)
        .fold(f64::INFINITY, f64::min);
    if leftmost.is_finite() {
        let shift = MARGIN - leftmost;
        for node in placement.layout_nodes.iter_mut() {
            node.x += shift;
        }
    }

    placement.max_width = placement
        .layout_nodes
        .iter()
        .enumerate()
        .map(|(i, n)| n.x + n.width + right_pad[i] + MARGIN)
        .fold(0.0, f64::max);
}
