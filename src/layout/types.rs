//! Data structures for layout computation.

#![allow(dead_code)]

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

/// Result of edge analysis phase.
pub struct EdgeAnalysis<'a> {
    /// Node ID -> level
    pub node_level: HashMap<&'a str, i64>,
    /// (Node ID, going_down) -> edge count
    pub edge_count_per_node: HashMap<(&'a str, bool), usize>,
    /// Channel level -> list of edge indices passing through
    pub channel_edges: HashMap<i64, Vec<usize>>,
    /// Channel level -> edge count
    pub channel_edge_count: HashMap<i64, usize>,
}

/// Result of corridor analysis phase.
pub struct CorridorAnalysis {
    /// Gap index -> list of edge indices using this corridor
    pub corridor_edges: HashMap<usize, Vec<usize>>,
    /// Edge index -> gap index
    pub edge_gap_index: HashMap<usize, usize>,
    /// Gap index -> extra width needed
    pub gap_extra_width: HashMap<usize, f64>,
}

/// Result of node placement phase.
pub struct NodePlacement {
    pub layout_nodes: Vec<LayoutNode>,
    /// Level -> bottom Y coordinate
    pub level_bottom_y: HashMap<i64, f64>,
    /// Channel level -> Y coordinate
    pub channel_y: HashMap<i64, f64>,
    pub max_width: f64,
    pub total_height: f64,
}

/// Lane assignments for edge routing.
pub struct LaneAssignments {
    /// (Channel level, edge index) -> lane number
    pub channel_lanes: HashMap<(i64, usize), usize>,
    /// Edge index -> lane number (for same-level edges)
    pub same_level_lanes: HashMap<usize, usize>,
    /// (Gap index, edge index) -> lane number
    pub corridor_lanes: HashMap<(usize, usize), usize>,
    /// Gap index -> total edges in corridor
    pub corridor_total_edges: HashMap<usize, usize>,
    /// Edge index -> pre-calculated corridor X position
    pub multi_level_corridor_x: HashMap<usize, f64>,
}

/// Node order information within levels.
pub struct NodeOrder<'a> {
    /// Node ID -> order within level
    pub order: HashMap<&'a str, usize>,
}
