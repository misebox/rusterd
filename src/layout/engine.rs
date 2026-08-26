//! Layout engine core implementation.

use crate::ir::{GraphIR, Node};
use crate::measure::TextMetrics;
use std::collections::HashMap;

use super::align::align_levels;
use super::analysis::{
    analyze_channel_edges, analyze_corridors, build_node_order, calculate_dynamic_channel_gaps,
    calculate_self_ref_reserve, count_edges_per_node,
};
use super::anchors::calculate_edge_anchors;
use super::descent::plan_descents;
use super::lanes::{
    align_corridors_with_anchors, assign_channel_lanes, calculate_multi_level_corridor_x,
};
use super::layering::assign_levels;
use super::placement::{
    build_node_positions, calculate_node_sizes, group_nodes_by_level, place_columns, place_rows,
    reorder_levels, stand_low,
};
use super::straighten::straighten_edges;
use super::types::Layout;
use super::waypoints::route_edges;

/// How many orderings to try before settling. Each one is a whole layout, which
/// is cheap; past this the drawing rarely improves.
const ATTEMPTS: usize = 24;

/// What the gaps are multiplied by when the spacing is asked to be dense.
const DENSE: f64 = 0.6;

/// Narrowest the anchors may be spread, dense or not: two cardinality pills
/// side by side on one border still have to be readable.
const MIN_ANCHOR_SPACING: f64 = 40.0;

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
    /// Longest detour absorbed by the straightening pass
    pub(crate) jog_tolerance: f64,
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self {
            metrics: TextMetrics::default(),
            node_gap_x: 100.0,
            node_gap_y: 30.0,
            channel_gap: 50.0,
            lane_spacing: 24.0,
            // Wide enough for two cardinality pills (`1..*` is the widest) to
            // sit side by side on one entity border.
            anchor_spacing: 56.0,
            corner_radius: 32.0,
            entity_margin: 30.0,
            jog_tolerance: 20.0,
        }
    }
}

impl LayoutEngine {
    /// Close everything up, for fitting a large schema on one screen.
    ///
    /// Only the gaps move: the text is the size it has to be to read, which is
    /// why zooming out cannot do this and why the room for anchors has a floor.
    /// There is no setting the other way — if the ordinary spacing were wrong,
    /// the thing to change would be the ordinary spacing.
    pub fn with_dense_spacing(mut self, dense: bool) -> Self {
        if !dense {
            return self;
        }
        self.node_gap_x *= DENSE;
        self.node_gap_y *= DENSE;
        self.channel_gap *= DENSE;
        self.lane_spacing *= DENSE;
        self.entity_margin *= DENSE;
        self.anchor_spacing = (self.anchor_spacing * DENSE).max(MIN_ANCHOR_SPACING);
        self
    }

    /// Compute layout for the given graph.
    ///
    /// Where the source did not say how to arrange its entities, an arrangement
    /// with fewer crossings is worked out and drawn as well; whichever of the
    /// two reads more easily is the one returned. The heuristic proposes, the
    /// finished drawing decides.
    pub fn layout(&self, ir: &GraphIR) -> Layout {
        let node_level = assign_levels(ir);
        let (levels, level_keys) = group_nodes_by_level(ir, &node_level);
        let mut best = self.arrange(ir, &levels, &level_keys);

        // The ordering works on an idealised drawing and settles into whichever
        // arrangement is nearest to where it started, which is often not the
        // best one. So it is run again from several starting points, and the
        // drawing that comes out tidiest wins. The starts are fixed, so the
        // same source always draws the same diagram.
        for attempt in 0..ATTEMPTS {
            let lone_weight = if attempt % 2 == 0 { 1 } else { 4 };
            let reordered =
                reorder_levels(ir, &levels, &level_keys, lone_weight, attempt as u64 / 2);
            let candidate = self.arrange(ir, &reordered, &level_keys);
            if candidate.is_tidier_than(&best, &ir.near) {
                best = candidate;
            }
        }

        best
    }

    /// Draw the graph with its entities in the given order.
    fn arrange(
        &self,
        ir: &GraphIR,
        levels: &HashMap<i64, Vec<&Node>>,
        level_keys: &[i64],
    ) -> Layout {
        // Phase 1: Edge analysis
        let node_level = assign_levels(ir);
        let edge_count_per_node = count_edges_per_node(ir, &node_level);
        let (channel_edges_list, _) = analyze_channel_edges(ir, &node_level);

        // Phase 2: Node grouping
        let node_order = build_node_order(levels);

        // Phase 3: Corridor analysis
        let corridor_analysis = analyze_corridors(ir, &node_level, &node_order, self.lane_spacing);

        // Phase 4: Node sizing and columns. Nothing between here and the rows
        // depends on how far down the page anything is.
        let node_sizes =
            calculate_node_sizes(ir, &edge_count_per_node, &self.metrics, self.anchor_spacing);

        let self_ref_reserve = calculate_self_ref_reserve(ir, &self.metrics, self.lane_spacing);

        let mut node_placement = place_columns(
            levels,
            level_keys,
            &node_sizes,
            &corridor_analysis.gap_extra_width,
            &self_ref_reserve,
            self.node_gap_x,
        );

        // Phase 5: Slide each level under the entities it relates to
        align_levels(&mut node_placement, ir, self.node_gap_x);

        let node_positions = build_node_positions(&node_placement.layout_nodes);

        // Phase 6: The line each level-skipping edge runs down
        let mut multi_level_corridor_x = calculate_multi_level_corridor_x(
            ir,
            &node_level,
            &node_positions,
            &node_placement.layout_nodes,
            levels,
            self.entity_margin,
            self.lane_spacing,
        );

        // Phase 7: Where each edge meets its entities
        let node_exits = calculate_edge_anchors(
            ir,
            &node_positions,
            &node_level,
            &multi_level_corridor_x,
            self.anchor_spacing,
        );

        align_corridors_with_anchors(
            &mut multi_level_corridor_x,
            ir,
            &node_level,
            &node_exits,
            self.jog_tolerance,
        );

        // Phase 7b: Where each level-skipping edge steps sideways
        let descents = plan_descents(
            ir,
            &node_level,
            &node_placement.layout_nodes,
            levels,
            &node_exits,
            self.entity_margin,
            self.jog_tolerance,
        );

        // Phase 8: Lane assignments, which say how much room each channel needs
        let (channel_lane_assignments, lanes_in_use) = assign_channel_lanes(
            ir,
            &channel_edges_list,
            &node_positions,
            &node_level,
            &node_exits,
            &multi_level_corridor_x,
            &descents,
        );

        // Phase 8b: Now the rows can be settled
        let dynamic_channel_gap = calculate_dynamic_channel_gaps(
            level_keys,
            &lanes_in_use,
            self.entity_margin,
            self.lane_spacing,
            self.channel_gap,
        );
        place_rows(
            &mut node_placement,
            level_keys,
            &dynamic_channel_gap,
            self.node_gap_y,
            self.channel_gap,
            &stand_low(ir, &edge_count_per_node),
        );
        let node_positions = build_node_positions(&node_placement.layout_nodes);

        // Phase 9: Edge routing
        let mut layout_edges = route_edges(
            ir,
            &node_positions,
            &node_level,
            &node_exits,
            &lanes_in_use,
            &channel_lane_assignments,
            &node_placement,
            levels,
            &multi_level_corridor_x,
            &descents,
            self.lane_spacing,
            self.channel_gap,
            self.node_gap_x,
            self.entity_margin,
        );

        // Phase 10: Straighten paths that only jog by a few pixels
        straighten_edges(
            &mut layout_edges,
            &node_positions,
            self.jog_tolerance,
            // Anchors are distributed one `anchor_spacing` apart; sliding one
            // must not crowd its neighbour (their cardinality labels need the
            // same room).
            self.anchor_spacing,
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
