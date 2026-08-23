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
        let away = if self.x >= x { x + distance } else { x - distance };
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

