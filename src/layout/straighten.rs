//! Post-processing pass that straightens edge paths.
//!
//! Routing derives waypoints from independent constraints (anchor centers,
//! channel Y, corridor X). When two of those constraints differ only slightly,
//! the path gains a "jog": a few-pixel step that could just as well be a
//! straight line. This pass removes redundant waypoints and absorbs such jogs
//! by sliding the endpoint anchors along the node border they sit on.

use std::collections::HashMap;

use super::types::{LayoutEdge, LayoutNode};

const EPS: f64 = 1e-6;

/// Clearance kept between an anchor and the corner of its border.
const BORDER_MARGIN: f64 = 4.0;

/// Which border of a node an endpoint sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Border {
    Top,
    Bottom,
    Left,
    Right,
}

impl Border {
    /// True when an anchor on this border slides along the X axis.
    fn slides_horizontally(self) -> bool {
        matches!(self, Border::Top | Border::Bottom)
    }
}

/// An edge endpoint that may be slid along a node border.
#[derive(Debug, Clone, Copy)]
struct Anchor<'a> {
    node_id: &'a str,
    node: &'a LayoutNode,
    border: Border,
    /// Index into the anchor slots of this (node, border) pair.
    slot: usize,
}

impl Anchor<'_> {
    /// True when an anchor at `target` stays inside the border it slides along.
    fn accepts(&self, target: f64) -> bool {
        let (min, max) = if self.border.slides_horizontally() {
            (self.node.x, self.node.x + self.node.width)
        } else {
            (self.node.y, self.node.y + self.node.height)
        };
        target >= min + BORDER_MARGIN && target <= max - BORDER_MARGIN
    }
}

/// Anchor coordinates per node border, used to keep anchors from colliding.
#[derive(Default)]
struct AnchorMap<'a> {
    slots: HashMap<(&'a str, Border), Vec<f64>>,
}

/// Remove redundant waypoints and collapse micro-jogs.
///
/// `jog_tolerance` is the longest jog segment that gets absorbed and
/// `min_anchor_gap` the clearance kept between anchors on the same border.
pub fn straighten_edges<'a>(
    edges: &mut [LayoutEdge],
    node_positions: &HashMap<&'a str, &'a LayoutNode>,
    jog_tolerance: f64,
    min_anchor_gap: f64,
) {
    for edge in edges.iter_mut() {
        simplify(&mut edge.waypoints);
    }

    let mut anchors = AnchorMap::default();
    let endpoints: Vec<Option<(Anchor<'a>, Anchor<'a>)>> = edges
        .iter()
        .map(|edge| {
            if edge.is_self_ref {
                return None;
            }
            let from = anchors.register(node_positions, &edge.from, *edge.waypoints.first()?)?;
            let to = anchors.register(node_positions, &edge.to, *edge.waypoints.last()?)?;
            Some((from, to))
        })
        .collect();

    for (edge, ends) in edges.iter_mut().zip(endpoints) {
        let Some((from, to)) = ends else { continue };

        // Every absorbed jog removes at least one waypoint, so the path length
        // bounds the iteration count.
        for _ in 0..edge.waypoints.len() {
            let Some(jog) = find_jog(&edge.waypoints, jog_tolerance) else {
                break;
            };
            if !absorb_jog(&mut edge.waypoints, jog, from, to, &mut anchors, min_anchor_gap) {
                break;
            }
            simplify(&mut edge.waypoints);
        }
    }
}

impl<'a> AnchorMap<'a> {
    /// Record an endpoint and return a handle to its slot.
    fn register(
        &mut self,
        node_positions: &HashMap<&'a str, &'a LayoutNode>,
        id: &str,
        point: (f64, f64),
    ) -> Option<Anchor<'a>> {
        let (&node_id, &node) = node_positions.get_key_value(id)?;
        let border = classify_border(node, point)?;
        let coord = if border.slides_horizontally() {
            point.0
        } else {
            point.1
        };
        let slots = self.slots.entry((node_id, border)).or_default();
        slots.push(coord);
        Some(Anchor {
            node_id,
            node,
            border,
            slot: slots.len() - 1,
        })
    }

    /// True when `coord` keeps `min_gap` from every other anchor on the border.
    fn is_free(&self, anchor: Anchor, coord: f64, min_gap: f64) -> bool {
        match self.slots.get(&(anchor.node_id, anchor.border)) {
            Some(slots) => slots
                .iter()
                .enumerate()
                .all(|(i, &other)| i == anchor.slot || (other - coord).abs() >= min_gap),
            None => true,
        }
    }

    fn move_anchor(&mut self, anchor: Anchor<'a>, coord: f64) {
        if let Some(slot) = self
            .slots
            .get_mut(&(anchor.node_id, anchor.border))
            .and_then(|slots| slots.get_mut(anchor.slot))
        {
            *slot = coord;
        }
    }
}

/// Determine which border of `node` the point sits on.
fn classify_border(node: &LayoutNode, (x, y): (f64, f64)) -> Option<Border> {
    let on = |a: f64, b: f64| (a - b).abs() < 0.5;
    if on(y, node.y) {
        Some(Border::Top)
    } else if on(y, node.y + node.height) {
        Some(Border::Bottom)
    } else if on(x, node.x) {
        Some(Border::Left)
    } else if on(x, node.x + node.width) {
        Some(Border::Right)
    } else {
        None
    }
}

/// Drop duplicate and collinear waypoints.
fn simplify(waypoints: &mut Vec<(f64, f64)>) {
    waypoints.dedup_by(|a, b| (a.0 - b.0).abs() < EPS && (a.1 - b.1).abs() < EPS);

    let mut i = 1;
    while waypoints.len() > 2 && i + 1 < waypoints.len() {
        let (px, py) = waypoints[i - 1];
        let (x, y) = waypoints[i];
        let (nx, ny) = waypoints[i + 1];
        let collinear = ((x - px) * (ny - y) - (y - py) * (nx - x)).abs() < EPS;
        if collinear {
            waypoints.remove(i);
        } else {
            i += 1;
        }
    }
}

/// Index of the first interior segment short enough to be absorbed.
///
/// After `simplify` every adjacent segment pair alternates direction, so a
/// short interior segment is always a jog between two parallel lines.
fn find_jog(waypoints: &[(f64, f64)], tolerance: f64) -> Option<usize> {
    if waypoints.len() < 4 {
        return None;
    }
    let last_segment = waypoints.len() - 2;
    (1..last_segment).find(|&s| {
        let (x1, y1) = waypoints[s];
        let (x2, y2) = waypoints[s + 1];
        let len = (x2 - x1).abs().max((y2 - y1).abs());
        len > EPS && len <= tolerance
    })
}

/// Try to absorb the jog at segment `s` by sliding one of the endpoint anchors.
///
/// Only endpoint anchors move: shifting an interior line would break the
/// channel and corridor lane assignments it shares with other edges.
fn absorb_jog<'a>(
    waypoints: &mut [(f64, f64)],
    s: usize,
    from: Anchor<'a>,
    to: Anchor<'a>,
    anchors: &mut AnchorMap<'a>,
    min_anchor_gap: f64,
) -> bool {
    let last = waypoints.len() - 1;
    let horizontal = (waypoints[s].0 - waypoints[s + 1].0).abs() > EPS;
    let coord = |p: (f64, f64)| if horizontal { p.0 } else { p.1 };

    // Sliding the head means moving it onto the line after the jog, and vice
    // versa. Only a jog next to an endpoint can be absorbed this way.
    let head = (s == 1).then_some(([0, 1], coord(waypoints[s + 1]), from));
    let tail = (s + 2 == last).then_some(([last - 1, last], coord(waypoints[s]), to));

    let mut candidates = [head, tail];
    if to.node.width > from.node.width {
        // The wider node absorbs the offset: the same shift is less noticeable
        // relative to its width, so the narrower node keeps its centered anchor.
        candidates.swap(0, 1);
    }

    for (indices, target, anchor) in candidates.into_iter().flatten() {
        if anchor.border.slides_horizontally() != horizontal
            || !anchor.accepts(target)
            || !anchors.is_free(anchor, target, min_anchor_gap)
        {
            continue;
        }
        for i in indices {
            if horizontal {
                waypoints[i].0 = target;
            } else {
                waypoints[i].1 = target;
            }
        }
        anchors.move_anchor(anchor, target);
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, x: f64, width: f64, y: f64) -> LayoutNode {
        LayoutNode {
            id: id.to_string(),
            x,
            y,
            width,
            height: 84.0,
        }
    }

    fn edge(from: &str, to: &str, waypoints: Vec<(f64, f64)>) -> LayoutEdge {
        LayoutEdge {
            from: from.to_string(),
            to: to.to_string(),
            waypoints,
            is_self_ref: false,
            edge_index: 0,
        }
    }

    fn straighten(nodes: &[LayoutNode], edges: &mut [LayoutEdge]) {
        let positions: HashMap<&str, &LayoutNode> =
            nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        straighten_edges(edges, &positions, 20.0, 12.0);
    }

    #[test]
    fn absorbs_micro_jog_into_straight_line() {
        let nodes = vec![node("A", 40.0, 120.0, 40.0), node("B", 40.0, 136.0, 214.0)];
        let mut edges = vec![edge(
            "A",
            "B",
            vec![(100.0, 124.0), (100.0, 169.0), (108.0, 169.0), (108.0, 214.0)],
        )];

        straighten(&nodes, &mut edges);

        // The wider node (B) absorbs the offset, so the line stays at A's center.
        assert_eq!(edges[0].waypoints, vec![(100.0, 124.0), (100.0, 214.0)]);
    }

    #[test]
    fn keeps_jog_wider_than_tolerance() {
        let nodes = vec![node("A", 40.0, 200.0, 40.0), node("B", 300.0, 200.0, 214.0)];
        let waypoints = vec![
            (140.0, 124.0),
            (140.0, 169.0),
            (400.0, 169.0),
            (400.0, 214.0),
        ];
        let mut edges = vec![edge("A", "B", waypoints.clone())];

        straighten(&nodes, &mut edges);

        assert_eq!(edges[0].waypoints, waypoints);
    }

    #[test]
    fn slides_the_other_endpoint_when_the_first_is_crowded() {
        let nodes = vec![node("A", 40.0, 200.0, 40.0), node("B", 40.0, 200.0, 214.0)];
        let mut edges = vec![
            edge(
                "A",
                "B",
                vec![(120.0, 124.0), (120.0, 169.0), (130.0, 169.0), (130.0, 214.0)],
            ),
            edge(
                "A",
                "B",
                vec![(140.0, 124.0), (140.0, 169.0), (150.0, 169.0), (150.0, 214.0)],
            ),
        ];

        straighten(&nodes, &mut edges);

        // Moving A's anchor to 130 would crowd the second edge's anchor at 140,
        // so B's anchor slides to 120 instead.
        assert_eq!(edges[0].waypoints, vec![(120.0, 124.0), (120.0, 214.0)]);
        assert_eq!(edges[1].waypoints, vec![(150.0, 124.0), (150.0, 214.0)]);
    }

    #[test]
    fn keeps_the_jog_when_both_anchors_are_crowded() {
        let nodes = vec![node("A", 40.0, 200.0, 40.0), node("B", 40.0, 200.0, 214.0)];
        let crowded = vec![
            (100.0, 124.0),
            (100.0, 169.0),
            (110.0, 169.0),
            (110.0, 214.0),
        ];
        let mut edges = vec![
            edge("A", "B", crowded.clone()),
            // Jog too wide to absorb, but its anchors block both candidate slides.
            edge(
                "A",
                "B",
                vec![(118.0, 124.0), (118.0, 169.0), (92.0, 169.0), (92.0, 214.0)],
            ),
        ];

        straighten(&nodes, &mut edges);

        assert_eq!(edges[0].waypoints, crowded);
    }

    #[test]
    fn drops_collinear_waypoints() {
        let nodes = vec![node("A", 40.0, 120.0, 40.0), node("B", 40.0, 120.0, 300.0)];
        let mut edges = vec![edge(
            "A",
            "B",
            vec![
                (100.0, 124.0),
                (100.0, 169.0),
                (100.0, 240.0),
                (100.0, 300.0),
            ],
        )];

        straighten(&nodes, &mut edges);

        assert_eq!(edges[0].waypoints, vec![(100.0, 124.0), (100.0, 300.0)]);
    }

    #[test]
    fn leaves_self_reference_untouched() {
        let nodes = vec![node("A", 40.0, 120.0, 40.0)];
        let waypoints = vec![(160.0, 65.0), (185.0, 65.0), (185.0, 99.0), (160.0, 99.0)];
        let mut edges = vec![LayoutEdge {
            from: "A".to_string(),
            to: "A".to_string(),
            waypoints: waypoints.clone(),
            is_self_ref: true,
            edge_index: 0,
        }];

        straighten(&nodes, &mut edges);

        assert_eq!(edges[0].waypoints, waypoints);
    }
}
