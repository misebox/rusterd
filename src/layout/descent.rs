//! Choosing where an edge that skips a level steps sideways.
//!
//! Such an edge drops down one column, steps across, and drops down another.
//! Which channel it steps across in is free, as long as each column clears the
//! entities it passes. The choice matters: the sideways run cuts every line
//! standing in the channel it happens in, and the one line an entity has to its
//! only relation is the worst of those to cut.

use crate::ir::GraphIR;
use std::collections::HashMap;

use super::analysis::edge_sides;
use super::anchors::{anchor_x, Anchors};
use super::corridor::{clear_at, find_safe_corridors};
use super::types::LayoutNode;
use crate::ir::Node;

/// What a line standing in a channel costs to cut. An entity with one relation
/// is read as a unit with the entity it hangs off; a line through that reading
/// is worth avoiding several ordinary crossings for.
const LONE_RELATION: usize = 12;
const ORDINARY: usize = 1;

/// What the two extra corners of a detour are worth in crossings.
const DETOUR_PENALTY: usize = 4;

/// How one level-skipping edge gets down.
#[derive(Debug, Clone, Copy)]
pub enum Descent {
    /// Down one column, one step across in this channel, down the other.
    Step(i64),
    /// Down, across into a column between the entities in the way, down that,
    /// and across again. Two more corners, for when no single step is clear.
    Detour(f64),
}

/// How each level-skipping edge gets down, by edge index.
pub type Descents = HashMap<usize, Descent>;

/// A line running the full height of a channel, which anything stepping across
/// that channel over this x will cut.
struct Standing {
    channel: i64,
    x: f64,
    cost: usize,
}

pub fn plan_descents<'a>(
    ir: &'a GraphIR,
    node_level: &HashMap<&str, i64>,
    layout_nodes: &[LayoutNode],
    levels: &HashMap<i64, Vec<&'a Node>>,
    anchors: &Anchors<'a>,
    entity_margin: f64,
    jog_tolerance: f64,
) -> Descents {
    let ends = |idx: usize| -> Option<(f64, f64)> {
        let edge = &ir.edges[idx];
        let from_level = *node_level.get(edge.from.as_str())?;
        let to_level = *node_level.get(edge.to.as_str())?;
        let (from_side, to_side) = edge_sides(from_level, to_level);
        let leaving = anchor_x(anchors, edge.from.as_str(), from_side, idx)?;
        let landing = anchor_x(anchors, edge.to.as_str(), to_side, idx)?;
        Some(if from_level <= to_level {
            (leaving, landing)
        } else {
            (landing, leaving)
        })
    };

    let lone = lone_relations(ir);
    let mut standing: Vec<Standing> = Vec::new();

    // A relation between neighbouring levels drawn as one straight line stands
    // the full height of its channel. Those are fixed before anything else is
    // decided, so they are what the choices below are made around.
    for (idx, edge) in ir.edges.iter().enumerate() {
        let (Some(&from_level), Some(&to_level)) = (
            node_level.get(edge.from.as_str()),
            node_level.get(edge.to.as_str()),
        ) else {
            continue;
        };
        if (to_level - from_level).abs() != 1 {
            continue;
        }
        let Some((top_x, bottom_x)) = ends(idx) else {
            continue;
        };
        if (top_x - bottom_x).abs() < 1.0 {
            standing.push(Standing {
                channel: from_level.min(to_level),
                x: top_x,
                cost: if lone[idx] { LONE_RELATION } else { ORDINARY },
            });
        }
    }

    // Longest first: an edge crossing many levels has the most to gain from
    // choosing well, and the most lines to disturb if it chooses badly.
    let mut skipping: Vec<usize> = ir
        .edges
        .iter()
        .enumerate()
        .filter(|(_, edge)| {
            let (Some(&from), Some(&to)) = (
                node_level.get(edge.from.as_str()),
                node_level.get(edge.to.as_str()),
            ) else {
                return false;
            };
            (to - from).abs() > 1
        })
        .map(|(idx, _)| idx)
        .collect();
    skipping.sort_by_key(|&idx| {
        let edge = &ir.edges[idx];
        let from = node_level.get(edge.from.as_str()).copied().unwrap_or(0);
        let to = node_level.get(edge.to.as_str()).copied().unwrap_or(0);
        (std::cmp::Reverse((to - from).abs()), idx)
    });

    let mut descents = Descents::new();

    for idx in skipping {
        let edge = &ir.edges[idx];
        let from_level = node_level.get(edge.from.as_str()).copied().unwrap_or(0);
        let to_level = node_level.get(edge.to.as_str()).copied().unwrap_or(0);
        let (top_level, bottom_level) = (from_level.min(to_level), from_level.max(to_level));
        let Some((top_x, bottom_x)) = ends(idx) else {
            continue;
        };

        let clear = |level: i64, x: f64| clear_at(layout_nodes, levels, level, x, entity_margin);
        let usable = |channel: i64| {
            ((top_level + 1)..=channel).all(|level| clear(level, top_x))
                && ((channel + 1)..bottom_level).all(|level| clear(level, bottom_x))
        };

        // What stepping across one channel between two columns would cut.
        let cost = |channel: i64, from: f64, to: f64| {
            let (left, right) = (from.min(to), from.max(to));
            standing
                .iter()
                .filter(|line| line.channel == channel && left < line.x && line.x < right)
                .map(|line| line.cost)
                .sum::<usize>()
        };

        let weight = if lone[idx] { LONE_RELATION } else { ORDINARY };

        let corridors =
            find_safe_corridors(layout_nodes, levels, top_level, bottom_level, entity_margin);
        // Where each corridor comes closest to either end: the places that keep
        // the two steps of a detour short. A column that lands a few pixels off
        // an anchor is stood clear of it, since a step that small reads as a
        // mistake rather than as a turn.
        let mut through: Vec<f64> = corridors
            .iter()
            .flat_map(|&(left, right)| {
                [top_x, bottom_x].map(|anchor| {
                    let x = anchor.clamp(left, right);
                    let step = (x - anchor).abs();
                    if step > 0.0 && step < jog_tolerance {
                        let away = if x > anchor {
                            anchor + jog_tolerance
                        } else {
                            anchor - jog_tolerance
                        };
                        away.clamp(left, right)
                    } else {
                        x
                    }
                })
            })
            .collect();
        through.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let best_step = (top_level..bottom_level)
            .filter(|&channel| usable(channel))
            .map(|channel| (cost(channel, top_x, bottom_x), channel))
            .min_by_key(|&(cost, channel)| (cost, channel));
        let best_detour = through
            .iter()
            .map(|&x| {
                (
                    cost(top_level, top_x, x) + cost(bottom_level - 1, x, bottom_x),
                    x,
                )
            })
            .min_by_key(|&(cost, _)| cost);

        // A detour buys its way past a crossing with two extra corners, which
        // is only worth it against several ordinary crossings or one that cuts
        // a lone relation.
        let take_detour = match (best_step, best_detour) {
            (Some((step, _)), Some((detour, _))) => detour + DETOUR_PENALTY < step,
            (None, Some(_)) => true,
            _ => false,
        };

        match (take_detour, best_step, best_detour) {
            (false, Some((_, chosen)), _) => {
                // Now that this edge is placed, its own two columns stand in
                // the channels either side of the step, for the edges decided
                // after it.
                for channel in top_level..chosen {
                    standing.push(Standing { channel, x: top_x, cost: weight });
                }
                for channel in (chosen + 1)..bottom_level {
                    standing.push(Standing { channel, x: bottom_x, cost: weight });
                }
                descents.insert(idx, Descent::Step(chosen));
            }
            (true, _, Some((_, x))) => {
                for channel in top_level..bottom_level {
                    standing.push(Standing { channel, x, cost: weight });
                }
                descents.insert(idx, Descent::Detour(x));
            }
            _ => continue,
        }
    }

    descents
}

/// Which edges are the only relation one of their entities has.
fn lone_relations(ir: &GraphIR) -> Vec<bool> {
    let mut degree: HashMap<&str, usize> = HashMap::new();
    for edge in ir.edges.iter().filter(|e| e.from != e.to) {
        *degree.entry(edge.from.as_str()).or_default() += 1;
        *degree.entry(edge.to.as_str()).or_default() += 1;
    }

    ir.edges
        .iter()
        .map(|edge| {
            edge.from != edge.to
                && (degree.get(edge.from.as_str()) == Some(&1)
                    || degree.get(edge.to.as_str()) == Some(&1))
        })
        .collect()
}
