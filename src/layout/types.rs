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

/// Result of corridor analysis phase.
pub struct CorridorAnalysis {
    /// Edge index -> gap index
    pub edge_gap_index: HashMap<usize, usize>,
    /// Gap index -> extra width needed
    pub gap_extra_width: HashMap<usize, f64>,
}

/// Result of node placement phase.
pub struct NodePlacement {
    pub layout_nodes: Vec<LayoutNode>,
    /// Channel level -> Y coordinate
    pub channel_y: HashMap<i64, f64>,
    pub max_width: f64,
    pub total_height: f64,
}

