//! Layout engine core implementation.

use crate::ir::GraphIR;
use crate::measure::TextMetrics;

use super::analysis::{
    analyze_channel_edges, analyze_corridors, build_node_level_lookup, build_node_order,
    calculate_dynamic_channel_gaps, count_edges_per_node,
};
use super::anchors::calculate_edge_anchors;
use super::lanes::{assign_channel_lanes, assign_corridor_lanes, calculate_multi_level_corridor_x};
use super::placement::{build_node_positions, calculate_node_sizes, group_nodes_by_level, place_nodes};
use super::types::Layout;
use super::waypoints::route_edges;

/// Layout engine configuration and computation.
pub struct LayoutEngine {
    pub(crate) metrics: TextMetrics,
    pub(crate) node_gap_x: f64,
    pub(crate) node_gap_y: f64,
    pub(crate) channel_gap: f64,
    pub(crate) lane_spacing: f64,
    pub(crate) anchor_spacing: f64,
    pub(crate) corner_radius: f64,
    pub(crate) entity_margin: f64,
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self {
            metrics: TextMetrics::default(),
            node_gap_x: 100.0,
            node_gap_y: 30.0,
            channel_gap: 50.0,
            lane_spacing: 24.0,
            anchor_spacing: 40.0,
            corner_radius: 32.0,
            entity_margin: 30.0,
        }
    }
}

impl LayoutEngine {
    /// Compute layout for the given graph.
    pub fn layout(&self, ir: &GraphIR) -> Layout {
        // Phase 1: Edge analysis
        let node_level = build_node_level_lookup(ir);
        let edge_count_per_node = count_edges_per_node(ir, &node_level);
        let (channel_edges_list, channel_edge_count) = analyze_channel_edges(ir, &node_level);

        // Phase 2: Node grouping
        let (levels, level_keys) = group_nodes_by_level(ir);
        let node_order = build_node_order(&levels);

        // Phase 3: Corridor analysis
        let corridor_analysis =
            analyze_corridors(ir, &node_level, &node_order, self.lane_spacing);

        // Phase 4: Dynamic channel gaps
        let dynamic_channel_gap = calculate_dynamic_channel_gaps(
            &level_keys,
            &channel_edge_count,
            self.entity_margin,
            self.lane_spacing,
            self.channel_gap,
        );

        // Phase 5: Node sizing and placement
        let node_sizes = calculate_node_sizes(
            ir,
            &edge_count_per_node,
            &self.metrics,
            self.anchor_spacing,
        );

        let node_placement = place_nodes(
            &levels,
            &level_keys,
            &node_sizes,
            &corridor_analysis.gap_extra_width,
            &dynamic_channel_gap,
            self.node_gap_x,
            self.node_gap_y,
            self.channel_gap,
        );

        let node_positions = build_node_positions(&node_placement.layout_nodes);

        // Phase 6: Edge anchor distribution
        let node_exits = calculate_edge_anchors(
            ir,
            &node_positions,
            &node_level,
            &corridor_analysis.edge_gap_index,
            &node_placement.layout_nodes,
            &levels,
            self.entity_margin,
            self.anchor_spacing,
        );

        // Phase 7: Lane assignments
        let (channel_lane_assignments, same_level_lane_assignments) = assign_channel_lanes(
            ir,
            &channel_edges_list,
            &node_positions,
            &node_level,
            &node_exits,
            &corridor_analysis.edge_gap_index,
            &node_placement.layout_nodes,
            &levels,
            self.anchor_spacing,
            self.entity_margin,
            self.node_gap_x,
            self.lane_spacing,
        );

        let (_corridor_lane_assignments, _corridor_total_edges) = assign_corridor_lanes(
            &corridor_analysis.corridor_edges,
            ir,
            &node_positions,
        );

        // Phase 8: Multi-level corridor X calculation
        let multi_level_corridor_x = calculate_multi_level_corridor_x(
            ir,
            &node_level,
            &node_positions,
            &node_placement.layout_nodes,
            &levels,
            self.entity_margin,
            self.lane_spacing,
        );

        // Phase 9: Edge routing
        let layout_edges = route_edges(
            ir,
            &node_positions,
            &node_level,
            &node_exits,
            &node_order,
            &channel_edge_count,
            &channel_lane_assignments,
            &same_level_lane_assignments,
            &node_placement,
            &levels,
            &multi_level_corridor_x,
            self.anchor_spacing,
            self.lane_spacing,
            self.channel_gap,
            self.node_gap_x,
            self.entity_margin,
        );

        Layout {
            nodes: node_placement.layout_nodes,
            edges: layout_edges,
            width: node_placement.max_width,
            height: node_placement.total_height,
            channel_gap: self.channel_gap,
            corner_radius: self.corner_radius,
        }
    }
}
