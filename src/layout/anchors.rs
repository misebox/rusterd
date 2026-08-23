//! Where each edge meets the border of its entities.

use crate::ir::GraphIR;
use std::collections::HashMap;

use super::analysis::edge_sides;
use super::fit::fit_in_order;
use super::types::{Corridor, LayoutNode};

/// Anchors along one border, as (edge index, x), left to right.
pub type Anchors<'a> = HashMap<(&'a str, bool), Vec<(usize, f64)>>;

/// Place every edge where it meets its entities.
///
/// An edge wants to leave facing the entity it runs to, so that a target
/// standing directly below its source is reached by going straight down. Where
/// several edges want the same spot they are spread apart just enough for their
/// cardinality labels, and the order they were sorted into — by the position of
/// the far end — keeps them from crossing on the way out.
#[allow(clippy::too_many_arguments)]
pub fn calculate_edge_anchors<'a>(
    ir: &'a GraphIR,
    node_positions: &HashMap<&str, &LayoutNode>,
    node_level: &HashMap<&str, i64>,
    corridors: &HashMap<usize, Corridor>,
    anchor_spacing: f64,
) -> Anchors<'a> {
    let mut anchors: Anchors<'a> = HashMap::new();

    for (idx, edge) in ir.edges.iter().enumerate() {
        if edge.from == edge.to {
            continue;
        }
        let (Some(from_node), Some(to_node)) = (
            node_positions.get(edge.from.as_str()),
            node_positions.get(edge.to.as_str()),
        ) else {
            continue;
        };

        let from_level = *node_level.get(edge.from.as_str()).unwrap_or(&0);
        let to_level = *node_level.get(edge.to.as_str()).unwrap_or(&0);
        let (from_side, to_side) = edge_sides(from_level, to_level);

        // An edge that skips a level runs down a corridor between the entities
        // in between, so both of its ends aim at that corridor rather than at
        // each other: aiming at an entity it cannot reach directly would only
        // drag the anchor across its neighbours.
        let (wants_from, wants_to) = match corridors.get(&idx) {
            Some(corridor) => (corridor.x, corridor.x),
            None => (
                to_node.x + to_node.width / 2.0,
                from_node.x + from_node.width / 2.0,
            ),
        };

        anchors
            .entry((edge.from.as_str(), from_side))
            .or_default()
            .push((idx, wants_from));

        anchors
            .entry((edge.to.as_str(), to_side))
            .or_default()
            .push((idx, wants_to));
    }

    // A crowded border has little say in where its anchors end up, so it is
    // settled first and the quieter borders line up with what it decided. Doing
    // it the other way round leaves both ends aiming at the middle of an entity
    // that no anchor is standing in.
    let mut order: Vec<(&'a str, bool)> = anchors.keys().copied().collect();
    order.sort_by_key(|key| (std::cmp::Reverse(anchors[key].len()), *key));

    let mut settled: HashMap<usize, f64> = HashMap::new();
    for key in order {
        let Some(node) = node_positions.get(key.0) else {
            continue;
        };
        let border = &mut anchors.get_mut(&key).expect("key came from the map");

        // Where the other end has already been placed, aim at it: an edge with
        // both ends free ends up drawn as one straight line.
        for (idx, wanted) in border.iter_mut() {
            if corridors.contains_key(idx) {
                continue;
            }
            if let Some(&facing) = settled.get(idx) {
                *wanted = facing;
            }
        }
        border.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let wanted: Vec<f64> = border.iter().map(|(_, x)| *x).collect();
        let span = vec![anchor_spacing; border.len()];
        // Half a slot of clearance at each end keeps the outermost cardinality
        // label from hanging over the corner. Entities are sized to hold every
        // anchor they carry, so this range is always wide enough.
        let inset = anchor_spacing / 2.0;
        let placed = fit_in_order(&wanted, &span, node.x + inset, node.x + node.width - inset);

        for ((idx, x), placed) in border.iter_mut().zip(placed) {
            *x = placed;
            settled.insert(*idx, placed);
        }
    }

    anchors
}

/// Where an edge meets one of its entities, if it was given an anchor there.
pub fn anchor_x<'a>(
    anchors: &Anchors<'a>,
    node_id: &'a str,
    side: bool,
    edge_index: usize,
) -> Option<f64> {
    anchors
        .get(&(node_id, side))?
        .iter()
        .find(|(idx, _)| *idx == edge_index)
        .map(|(_, x)| *x)
}
