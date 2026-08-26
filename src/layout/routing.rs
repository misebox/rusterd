//! Edge routing and waypoint generation.

use super::types::LayoutNode;

/// Calculate lane offset for centered lane distribution.
#[inline]
pub fn calculate_lane_offset(lane: usize, total: usize, lane_spacing: f64) -> f64 {
    if total <= 1 {
        0.0
    } else {
        (lane as f64 - (total - 1) as f64 / 2.0) * lane_spacing
    }
}

/// Distance from the entity border to the first self-reference loop.
pub const SELF_REF_LOOP_OFFSET: f64 = 25.0;

/// Shortest vertical distance between the two ends of a self-reference loop.
const SELF_REF_MIN_SPAN: f64 = 56.0;

/// Generate waypoints for a self-referential edge.
///
/// The two ends carry a cardinality label each, so they need enough vertical
/// separation for both pills to sit clear of one another. `loop_index` counts
/// the self-references already routed on this node, so several of them nest
/// instead of landing on the same path.
pub fn route_self_ref(node: &LayoutNode, loop_index: usize, lane_spacing: f64) -> Vec<(f64, f64)> {
    let span = (node.height * 0.6)
        .max(SELF_REF_MIN_SPAN)
        .min((node.height - 24.0).max(24.0));

    let x = node.x + node.width;
    let center_y = node.y + node.height / 2.0;
    let y_top = center_y - span / 2.0;
    let y_bottom = center_y + span / 2.0;
    let loop_offset = SELF_REF_LOOP_OFFSET + loop_index as f64 * lane_spacing;

    vec![
        (x, y_top),
        (x + loop_offset, y_top),
        (x + loop_offset, y_bottom),
        (x, y_bottom),
    ]
}

/// Generate waypoints for adjacent same-level edges (via sides).
pub fn route_same_level_adjacent(from_node: &LayoutNode, to_node: &LayoutNode) -> Vec<(f64, f64)> {
    let (left_node, right_node) = if from_node.x < to_node.x {
        (from_node, to_node)
    } else {
        (to_node, from_node)
    };

    let gap_between = right_node.x - (left_node.x + left_node.width);
    let mid_x = left_node.x + left_node.width + gap_between / 2.0;
    let from_y = from_node.y + from_node.height / 2.0;
    let to_y = to_node.y + to_node.height / 2.0;

    if from_node.x < to_node.x {
        vec![
            (from_node.x + from_node.width, from_y),
            (mid_x, from_y),
            (mid_x, to_y),
            (to_node.x, to_y),
        ]
    } else {
        vec![
            (from_node.x, from_y),
            (mid_x, from_y),
            (mid_x, to_y),
            (to_node.x + to_node.width, to_y),
        ]
    }
}

/// Generate waypoints for adjacent level edges with direct connection.
pub fn route_adjacent_level_direct(
    from_node: &LayoutNode,
    to_node: &LayoutNode,
    from_cx: f64,
    to_cx: f64,
    going_down: bool,
) -> Vec<(f64, f64)> {
    if going_down {
        vec![
            (from_cx, from_node.y + from_node.height),
            (to_cx, to_node.y),
        ]
    } else {
        vec![(from_cx, from_node.y), (to_cx, to_node.y + to_node.height)]
    }
}

/// Generate waypoints for adjacent level edges with channel routing.
pub fn route_adjacent_level_with_channel(
    from_node: &LayoutNode,
    to_node: &LayoutNode,
    from_cx: f64,
    to_cx: f64,
    ch_y: f64,
    going_down: bool,
) -> Vec<(f64, f64)> {
    if going_down {
        vec![
            (from_cx, from_node.y + from_node.height),
            (from_cx, ch_y),
            (to_cx, ch_y),
            (to_cx, to_node.y),
        ]
    } else {
        vec![
            (from_cx, from_node.y),
            (from_cx, ch_y),
            (to_cx, ch_y),
            (to_cx, to_node.y + to_node.height),
        ]
    }
}
