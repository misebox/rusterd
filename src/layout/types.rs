//! Data structures for layout computation.

use std::collections::HashMap;

/// A positioned node in the layout.
#[derive(Debug, Clone)]
pub struct LayoutNode {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// An edge with computed waypoints for orthogonal routing.
#[derive(Debug, Clone)]
pub struct LayoutEdge {
    pub from: String,
    pub to: String,
    /// Orthogonal path points (start, turns, end)
    pub waypoints: Vec<(f64, f64)>,
    pub is_self_ref: bool,
    /// Index into GraphIR.edges
    pub edge_index: usize,
}

/// The complete layout result.
#[derive(Debug, Clone)]
pub struct Layout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub width: f64,
    pub height: f64,
    /// Gap for routing channels between levels
    pub channel_gap: f64,
    /// Radius for rounded corners
    pub corner_radius: f64,
}

impl Layout {
    /// How many times one edge crosses another. Every crossing is a place the
    /// reader has to work out which line is which.
    pub fn crossings(&self) -> usize {
        let segments: Vec<((f64, f64), (f64, f64))> = self
            .edges
            .iter()
            .enumerate()
            .flat_map(|(i, edge)| edge.waypoints.windows(2).map(move |w| (i, (w[0], w[1]))))
            .map(|(_, s)| s)
            .collect();
        let owners: Vec<usize> = self
            .edges
            .iter()
            .enumerate()
            .flat_map(|(i, edge)| edge.waypoints.windows(2).map(move |_| i))
            .collect();

        let mut count = 0;
        for (i, first) in segments.iter().enumerate() {
            for (j, second) in segments.iter().enumerate().skip(i + 1) {
                if owners[i] != owners[j] && crosses(*first, *second) {
                    count += 1;
                }
            }
        }
        count
    }

    /// Places where two edges run along each other close enough to read as one
    /// line, which hides a relation completely rather than merely obscuring it.
    pub fn parallel_overlaps(&self) -> usize {
        let mut count = 0;
        for (i, edge) in self.edges.iter().enumerate() {
            for other in self.edges.iter().skip(i + 1) {
                for a in edge.waypoints.windows(2) {
                    for b in other.waypoints.windows(2) {
                        if runs_alongside((a[0], a[1]), (b[0], b[1])) {
                            count += 1;
                        }
                    }
                }
            }
        }
        count
    }

    /// Crossings that land on the only relation an entity has.
    ///
    /// An entity joined to just one other is read as a unit with it, and a line
    /// cutting across that relation breaks the reading for no reason the
    /// diagram can explain. These crossings are worth more than the others.
    pub fn lone_relation_crossings(&self) -> usize {
        let mut degree: HashMap<&str, usize> = HashMap::new();
        for edge in self.edges.iter().filter(|e| e.from != e.to) {
            *degree.entry(edge.from.as_str()).or_default() += 1;
            *degree.entry(edge.to.as_str()).or_default() += 1;
        }
        let lone = |edge: &LayoutEdge| {
            edge.from != edge.to
                && (degree.get(edge.from.as_str()) == Some(&1)
                    || degree.get(edge.to.as_str()) == Some(&1))
        };

        let mut count = 0;
        for (i, edge) in self.edges.iter().enumerate() {
            if !lone(edge) {
                continue;
            }
            for (j, other) in self.edges.iter().enumerate() {
                if i == j {
                    continue;
                }
                // A crossing between two lone relations is counted once from
                // each side, which is fair: it spoils both of them.
                count += edge
                    .waypoints
                    .windows(2)
                    .flat_map(|a| {
                        other
                            .waypoints
                            .windows(2)
                            .filter(move |b| crosses((a[0], a[1]), (b[0], b[1])))
                    })
                    .count();
            }
        }
        count
    }

    /// How many corners the edges turn.
    pub fn bends(&self) -> usize {
        self.edges
            .iter()
            .map(|e| e.waypoints.len().saturating_sub(2))
            .sum()
    }

    /// Whether this drawing reads more easily than `other`: fewer crossings
    /// first, since a crossing costs the reader more than a corner does.
    pub fn is_tidier_than(&self, other: &Layout, near: &[Vec<String>]) -> bool {
        self.quality(near) < other.quality(near)
    }

    /// Whether this drawing is closer to the shape asked for than `other`.
    ///
    /// Measured as a factor rather than a difference, so that being twice as
    /// wide as wanted counts the same as being half as wide, and folding is not
    /// tempted to trade a wide drawing for an equally awkward tall one.
    pub fn is_better_shaped_than(&self, other: &Layout, aspect: f64) -> bool {
        let out_of_shape = |drawing: &Layout| {
            if drawing.height <= 0.0 || aspect <= 0.0 {
                return f64::INFINITY;
            }
            (drawing.width / drawing.height / aspect).ln().abs()
        };
        out_of_shape(self) < out_of_shape(other)
    }

    /// What to weigh a drawing by, worst first: a hidden relation, then a line
    /// cut across a lone relation, then crossings, then how far apart the
    /// entities asked to be near one another ended up, then corners.
    fn quality(&self, near: &[Vec<String>]) -> (usize, usize, usize, usize, usize) {
        (
            self.parallel_overlaps(),
            self.lone_relation_crossings(),
            self.crossings(),
            self.spread(near),
            self.bends(),
        )
    }

    /// How much room the entities of each set are spread over, in steps of
    /// `SPREAD_STEP` so that a pixel here or there does not outweigh a corner.
    fn spread(&self, near: &[Vec<String>]) -> usize {
        let mut total = 0.0;
        for set in near {
            let placed = || self.nodes.iter().filter(|node| set.contains(&node.id));
            let left = placed().map(|n| n.x).fold(f64::INFINITY, f64::min);
            let right = placed()
                .map(|n| n.x + n.width)
                .fold(f64::NEG_INFINITY, f64::max);
            let top = placed().map(|n| n.y).fold(f64::INFINITY, f64::min);
            let bottom = placed()
                .map(|n| n.y + n.height)
                .fold(f64::NEG_INFINITY, f64::max);
            if left.is_finite() && top.is_finite() {
                total += (right - left) + (bottom - top);
            }
        }
        (total / SPREAD_STEP) as usize
    }
}

/// How much closer a set of entities has to be drawn before it counts as an
/// improvement. Finer than this and the search would chase pixels.
const SPREAD_STEP: f64 = 50.0;

/// How far apart two lines have to be to read as two lines.
const APART: f64 = 6.0;

/// How much of a run they have to share before it matters.
const SHARED: f64 = 8.0;

/// True when two segments lie along each other close enough, and far enough,
/// to be mistaken for one line.
fn runs_alongside(a: ((f64, f64), (f64, f64)), b: ((f64, f64), (f64, f64))) -> bool {
    let overlap = |(a1, a2): (f64, f64), (b1, b2): (f64, f64)| {
        a1.max(a2).min(b1.max(b2)) - a1.min(a2).max(b1.min(b2))
    };

    if a.0.0 == a.1.0 && b.0.0 == b.1.0 {
        return (a.0.0 - b.0.0).abs() < APART && overlap((a.0.1, a.1.1), (b.0.1, b.1.1)) > SHARED;
    }
    if a.0.1 == a.1.1 && b.0.1 == b.1.1 {
        return (a.0.1 - b.0.1).abs() < APART && overlap((a.0.0, a.1.0), (b.0.0, b.1.0)) > SHARED;
    }
    false
}

/// True when two axis-aligned segments cross at a point interior to both.
fn crosses(a: ((f64, f64), (f64, f64)), b: ((f64, f64), (f64, f64))) -> bool {
    let vertical = |(p, q): ((f64, f64), (f64, f64))| p.0 == q.0;
    if vertical(a) == vertical(b) {
        return false;
    }
    let (v, h) = if vertical(a) { (a, b) } else { (b, a) };
    let x = v.0.0;
    let y = h.0.1;
    let (top, bottom) = (v.0.1.min(v.1.1), v.0.1.max(v.1.1));
    let (left, right) = (h.0.0.min(h.1.0), h.0.0.max(h.1.0));
    top < y && y < bottom && left < x && x < right
}

/// The vertical line a level-skipping edge runs down, with the range it has to
/// stay inside to clear the entities it passes.
#[derive(Debug, Clone, Copy)]
pub struct Corridor {
    pub x: f64,
    pub left: f64,
    pub right: f64,
}

impl Corridor {
    /// Move the line onto `x` if that still clears the entities beside it.
    pub fn snap_to(&mut self, x: f64) -> bool {
        let reachable = self.left <= x && x <= self.right;
        if reachable {
            self.x = x;
        }
        reachable
    }

    /// Move the line at least `distance` away from `x`, as far as the corridor
    /// allows. Used when it cannot reach `x` at all: a step of a few pixels
    /// reads as a mistake, where a clear one reads as a turn.
    pub fn stand_clear_of(&mut self, x: f64, distance: f64) {
        let away = if self.x >= x {
            x + distance
        } else {
            x - distance
        };
        self.x = away.clamp(self.left, self.right);
    }
}

/// Result of corridor analysis phase.
pub struct CorridorAnalysis {
    /// Gap index -> extra width needed
    pub gap_extra_width: HashMap<usize, f64>,
}

/// Result of node placement phase.
pub struct NodePlacement {
    pub layout_nodes: Vec<LayoutNode>,
    /// One row per level, holding indices into `layout_nodes` left to right
    pub rows: Vec<Vec<usize>>,
    /// Space that must stay clear to the right of each node, indexed like
    /// `layout_nodes`
    pub min_gap: Vec<f64>,
    /// Channel level -> Y coordinate
    pub channel_y: HashMap<i64, f64>,
    pub max_width: f64,
    pub total_height: f64,
}
