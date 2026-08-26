//! Lane assignment for edges in channels and corridors.

use crate::ir::{GraphIR, Node};
use std::collections::HashMap;

use super::analysis::edge_sides;
use super::anchors::{Anchors, anchor_x};
use super::descent::{Descent, Descents};

use super::corridor::{find_safe_corridors, nearest_corridor};
use super::types::{Corridor, LayoutNode};

/// Order the edges crossing each channel into lanes.
///
/// Every edge crossing a channel comes down at one x, runs sideways, and leaves
/// at another. Two of them cross when one runs over the point where the other
/// comes down, so within a channel the edge that finishes furthest along its
/// direction of travel takes the lane nearest the level it came from: its
/// sideways run is over and done with before any of the others has dropped past
/// it. Edges coming up from below are the mirror image and take the far lanes.
pub fn assign_channel_lanes<'a>(
    ir: &'a GraphIR,
    channel_edges_list: &HashMap<i64, Vec<usize>>,
    node_positions: &HashMap<&str, &LayoutNode>,
    node_level: &HashMap<&str, i64>,
    anchors: &Anchors<'a>,
    corridors: &HashMap<usize, Corridor>,
    descents: &Descents,
) -> (HashMap<(i64, usize), usize>, HashMap<i64, usize>) {
    let mut lanes: HashMap<(i64, usize), usize> = HashMap::new();
    let mut used: HashMap<i64, usize> = HashMap::new();

    for (&channel, edge_indices) in channel_edges_list {
        let mut from_above: Vec<(usize, Run)> = Vec::new();
        let mut from_below: Vec<(usize, Run)> = Vec::new();

        for &idx in edge_indices {
            let edge = &ir.edges[idx];
            let from_level = *node_level.get(edge.from.as_str()).unwrap_or(&0);
            let to_level = *node_level.get(edge.to.as_str()).unwrap_or(&0);
            let (from_side, to_side) = edge_sides(from_level, to_level);

            let fallback = |id: &str| {
                node_positions
                    .get(id)
                    .map(|n| n.x + n.width / 2.0)
                    .unwrap_or(0.0)
            };
            let leaving = anchor_x(anchors, edge.from.as_str(), from_side, idx)
                .unwrap_or_else(|| fallback(edge.from.as_str()));
            let landing = anchor_x(anchors, edge.to.as_str(), to_side, idx)
                .unwrap_or_else(|| fallback(edge.to.as_str()));
            let corridor = corridors.get(&idx).map(|c| c.x);

            // An edge steps sideways in one channel and runs straight through
            // the rest, so in those it arrives and leaves at the same x.
            let run = if from_level == to_level {
                Run {
                    entry: leaving,
                    exit: landing,
                }
            } else if let Some(&Descent::Step(step)) = descents.get(&idx) {
                let (above, below) = if to_level > from_level {
                    (leaving, landing)
                } else {
                    (landing, leaving)
                };
                let (entry, exit) = match channel.cmp(&step) {
                    std::cmp::Ordering::Less => (above, above),
                    std::cmp::Ordering::Equal if to_level > from_level => (above, below),
                    std::cmp::Ordering::Equal => (below, above),
                    std::cmp::Ordering::Greater => (below, below),
                };
                Run { entry, exit }
            } else {
                // Down a corridor: only the first and last channel meet an
                // anchor, the rest stay in the corridor.
                let at = |level_touched: bool, anchor: f64| match (level_touched, corridor) {
                    (true, _) | (_, None) => anchor,
                    (false, Some(x)) => x,
                };
                if to_level > from_level {
                    Run {
                        entry: at(channel == from_level, leaving),
                        exit: at(channel == to_level - 1, landing),
                    }
                } else {
                    Run {
                        entry: at(channel == from_level - 1, leaving),
                        exit: at(channel == to_level, landing),
                    }
                }
            };

            // An edge running straight through a channel needs no lane of its
            // own: it is the ones that step sideways that have to be kept
            // apart, and only they should be paid for in the space between the
            // levels.
            if (run.entry - run.exit).abs() < 1.0 {
                continue;
            }
            if to_level >= from_level {
                from_above.push((idx, run));
            } else {
                from_below.push((idx, run));
            }
        }

        from_above.sort_by(|a, b| a.1.lane_order().cmp(&b.1.lane_order()));
        from_below.sort_by(|a, b| b.1.lane_order().cmp(&a.1.lane_order()));

        let (above, above_lanes) = pack(&from_above, 0);
        let (below, below_lanes) = pack(&from_below, above_lanes);
        for (idx, lane) in above.into_iter().chain(below) {
            lanes.insert((channel, idx), lane);
        }
        used.insert(channel, above_lanes + below_lanes);
    }

    (lanes, used)
}

/// Clear space kept between two runs sharing a lane, so that they read as two
/// lines end to end rather than one line straight through.
const RUN_CLEARANCE: f64 = 24.0;

/// Share a lane between runs that never pass over each other.
///
/// Two edges stepping across opposite ends of a wide diagram have nothing to do
/// with one another and can step at the same height. Only the ones passing over
/// each other need a lane apiece, so a channel ends up as deep as the runs are
/// piled rather than as numerous as they are.
///
/// A run still has to stay below every earlier run it passes over, or the order
/// worked out above — which is what keeps the edges from crossing — would be
/// undone by the sharing.
fn pack(runs: &[(usize, Run)], first_lane: usize) -> (Vec<(usize, usize)>, usize) {
    let mut lanes: Vec<Vec<&Run>> = Vec::new();
    let mut placed: Vec<(usize, usize)> = Vec::with_capacity(runs.len());

    for (idx, run) in runs {
        let below = lanes
            .iter()
            .enumerate()
            .filter(|(_, held)| held.iter().any(|other| !run.clear_of(other)))
            .map(|(lane, _)| lane + 1)
            .max()
            .unwrap_or(0);

        let lane = (below..lanes.len())
            .find(|&lane| lanes[lane].iter().all(|other| run.clear_of(other)))
            .unwrap_or_else(|| {
                while lanes.len() <= below {
                    lanes.push(Vec::new());
                }
                lanes.len() - 1
            });
        lanes[lane].push(run);
        placed.push((*idx, first_lane + lane));
    }

    (placed, lanes.len())
}

/// One edge's passage through one channel: where it arrives and where it goes.
struct Run {
    entry: f64,
    exit: f64,
}

impl Run {
    /// Whether this run stays clear of another, so the two can share a lane.
    fn clear_of(&self, other: &Run) -> bool {
        let (left, right) = (self.entry.min(self.exit), self.entry.max(self.exit));
        let (their_left, their_right) = (other.entry.min(other.exit), other.entry.max(other.exit));
        right + RUN_CLEARANCE < their_left || left - RUN_CLEARANCE > their_right
    }

    /// Sorts edges into the order that leaves the fewest crossings behind.
    fn lane_order(&self) -> (u8, ordered_float::Key) {
        let travelling_left = self.exit <= self.entry;
        let group = if travelling_left { 0 } else { 1 };
        let reach = if travelling_left {
            self.exit
        } else {
            -self.exit
        };
        (group, ordered_float::Key(reach))
    }
}

/// Comparing f64 keys, which do not implement `Ord`, without pulling in a crate.
mod ordered_float {
    #[derive(PartialEq, PartialOrd)]
    pub struct Key(pub f64);

    impl Eq for Key {}

    #[allow(clippy::derive_ord_xor_partial_ord)]
    impl Ord for Key {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            self.partial_cmp(other).unwrap_or(std::cmp::Ordering::Equal)
        }
    }
}

/// Calculate corridor X positions for multi-level edges.
///
/// An edge that skips a level has to pass the entities in between somewhere.
/// It runs down whichever safe corridor comes closest to the entity it is
/// heading for, so a target sitting in open space is reached by dropping
/// straight onto it rather than by touring the diagram.
#[allow(clippy::too_many_arguments)]
pub fn calculate_multi_level_corridor_x<'a>(
    ir: &'a GraphIR,
    node_level: &HashMap<&str, i64>,
    node_positions: &HashMap<&str, &LayoutNode>,
    layout_nodes: &[LayoutNode],
    levels: &HashMap<i64, Vec<&'a Node>>,
    entity_margin: f64,
    lane_spacing: f64,
) -> HashMap<usize, Corridor> {
    let mut multi_level_corridor_x: HashMap<usize, Corridor> = HashMap::new();
    let mut corridor_groups: HashMap<(i64, i64, usize), Vec<(usize, f64)>> = HashMap::new();

    for (idx, edge) in ir.edges.iter().enumerate() {
        if edge.from == edge.to {
            continue;
        }
        let from_level = *node_level.get(edge.from.as_str()).unwrap_or(&0);
        let to_level = *node_level.get(edge.to.as_str()).unwrap_or(&0);

        if (to_level - from_level).abs() <= 1 {
            continue;
        }

        let min_level = from_level.min(to_level);
        let max_level = from_level.max(to_level);

        let to_node = match node_positions.get(edge.to.as_str()) {
            Some(n) => *n,
            None => continue,
        };
        // The corridor lines up with the entity the edge ends on, which turns
        // the last hop into the descent itself instead of a sidestep. Both ends
        // of the edge then anchor onto the same line.
        let wanted_x = to_node.x + to_node.width / 2.0;

        let safe_corridors =
            find_safe_corridors(layout_nodes, levels, min_level, max_level, entity_margin);

        let best_corridor_idx = nearest_corridor(&safe_corridors, wanted_x)
            .map(|(i, _)| i)
            .unwrap_or(0);

        corridor_groups
            .entry((min_level, max_level, best_corridor_idx))
            .or_default()
            .push((idx, wanted_x));
    }

    for ((min_level, max_level, corridor_idx), edges) in &corridor_groups {
        let safe_corridors =
            find_safe_corridors(layout_nodes, levels, *min_level, *max_level, entity_margin);
        let (corridor_left, corridor_right) = safe_corridors
            .get(*corridor_idx)
            .copied()
            .unwrap_or((40.0, 200.0));

        let mut edges_sorted: Vec<(usize, f64)> = edges
            .iter()
            .map(|&(idx, wanted_x)| (idx, wanted_x.clamp(corridor_left, corridor_right)))
            .collect();
        edges_sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Edges that want the same spot — because the corridor is too narrow to
        // hold all of them where they would like to be — take a lane each.
        for i in 1..edges_sorted.len() {
            let floor = edges_sorted[i - 1].1 + lane_spacing;
            if edges_sorted[i].1 < floor {
                edges_sorted[i].1 = floor;
            }
        }

        if let (Some(&(_, first_x)), Some(&(_, last_x))) =
            (edges_sorted.first(), edges_sorted.last())
        {
            let overflow = (last_x - corridor_right).min(first_x - corridor_left);
            if overflow > 0.0 {
                for (_, x) in edges_sorted.iter_mut() {
                    *x -= overflow;
                }
            }
        }

        for (edge_idx, x) in edges_sorted {
            multi_level_corridor_x.insert(
                edge_idx,
                Corridor {
                    x,
                    left: corridor_left,
                    right: corridor_right,
                },
            );
        }
    }

    multi_level_corridor_x
}

/// Line up each corridor with one of the anchors its edge ends on.
///
/// The corridor is chosen before the anchors are, from where the target entity
/// stands. An anchor that cannot reach it — a narrow entity, or a border
/// already full — would leave the edge stepping sideways by a few pixels on its
/// way down. Landing the corridor on an anchor instead turns that step into
/// nothing at all; where no anchor can reach, the step is at least made big
/// enough to read as a deliberate turn.
pub fn align_corridors_with_anchors<'a>(
    corridors: &mut HashMap<usize, Corridor>,
    ir: &'a GraphIR,
    node_level: &HashMap<&str, i64>,
    anchors: &Anchors<'a>,
    jog_tolerance: f64,
) {
    for (&idx, corridor) in corridors.iter_mut() {
        let edge = &ir.edges[idx];
        let from_level = *node_level.get(edge.from.as_str()).unwrap_or(&0);
        let to_level = *node_level.get(edge.to.as_str()).unwrap_or(&0);
        let (from_side, to_side) = edge_sides(from_level, to_level);

        let landing = anchor_x(anchors, edge.to.as_str(), to_side, idx);
        if landing.is_some_and(|x| corridor.snap_to(x)) {
            continue;
        }
        let leaving = anchor_x(anchors, edge.from.as_str(), from_side, idx);
        if leaving.is_some_and(|x| corridor.snap_to(x)) {
            continue;
        }
        if let Some(x) = leaving {
            corridor.stand_clear_of(x, jog_tolerance);
        }
    }
}
