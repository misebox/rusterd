use crate::ast::Cardinality;
use crate::ir::{Edge, GraphIR};
use crate::layout::{Layout, LayoutEdge, LayoutNode};
use crate::measure::TextMetrics;
use std::collections::HashMap;
use std::fmt::Write;
use unicode_width::UnicodeWidthStr;

#[derive(Default)]
pub struct SvgRenderer {
    metrics: TextMetrics,
}

impl SvgRenderer {
    pub fn render(&self, ir: &GraphIR, layout: &Layout) -> String {
        let mut svg = String::new();

        writeln!(
            &mut svg,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">"#,
            layout.width, layout.height, layout.width, layout.height
        )
        .unwrap();

        // Style
        writeln!(
            &mut svg,
            r#"<style>
  .canvas {{ fill: #fff; }}
  .entity-bg {{ fill: #fff; }}
  .entity-header {{ fill: #e0e0e0; }}
  .entity-border {{ fill: none; stroke: #333; stroke-width: 1.5; }}
  .entity-separator {{ stroke: #333; stroke-width: 1; }}
  .entity-name {{ font-family: monospace; font-size: 14px; font-weight: bold; fill: #222; }}
  .column-text {{ font-family: monospace; font-size: 12px; fill: #222; }}
  .pk {{ font-weight: bold; }}
  .fk {{ font-style: italic; }}
  .edge {{ stroke: #666; stroke-width: 1.5; fill: none; }}
  .edge-label-bg {{ fill: rgba(234,234,234,0.9); }}
  .edge-label {{ font-family: monospace; font-size: 14px; fill: #444; }}
  .cardinality-bg {{ fill: rgba(224,224,224,0.95); }}
  .cardinality {{ font-family: monospace; font-size: 15px; font-weight: bold; fill: #222; }}
  @media (prefers-color-scheme: dark) {{
    .canvas {{ fill: #0d1117; }}
    .entity-bg {{ fill: #161b22; }}
    .entity-header {{ fill: #262c36; }}
    .entity-border {{ stroke: #6e7681; }}
    .entity-separator {{ stroke: #6e7681; }}
    .entity-name {{ fill: #e6edf3; }}
    .column-text {{ fill: #e6edf3; }}
    .edge {{ stroke: #8b949e; }}
    .edge-label-bg {{ fill: rgba(48,54,61,0.92); }}
    .edge-label {{ fill: #c9d1d9; }}
    .cardinality-bg {{ fill: rgba(60,67,76,0.95); }}
    .cardinality {{ fill: #f0f6fc; }}
  }}
</style>"#
        )
        .unwrap();

        // The diagram paints its own surface, so the dark palette does not sit
        // on whatever colour the host page happens to use.
        writeln!(
            &mut svg,
            r#"<rect class="canvas" width="100%" height="100%" />"#
        )
        .unwrap();

        // Build node lookup
        let node_map: HashMap<&str, &crate::ir::Node> =
            ir.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

        // 1. Render edge lines (behind nodes)
        for edge in &layout.edges {
            self.render_edge_line(&mut svg, edge, layout.corner_radius);
        }

        // 2. Render nodes (backgrounds, text, borders)
        for node in &layout.nodes {
            if let Some(ir_node) = node_map.get(node.id.as_str()) {
                self.render_node(&mut svg, node, ir_node);
            }
        }

        // 3. Render edge labels and cardinalities (on top of everything),
        //    after nudging apart any that would cover each other.
        let mut labels = Vec::new();
        for edge in &layout.edges {
            if let Some(ir_edge) = ir.edges.get(edge.edge_index) {
                self.plan_edge_labels(&mut labels, edge, ir_edge);
            }
        }
        resolve_label_overlaps(&mut labels, layout);
        for label in &labels {
            render_label(&mut svg, label);
        }

        writeln!(&mut svg, "</svg>").unwrap();
        svg
    }

    fn render_node(&self, svg: &mut String, layout: &LayoutNode, node: &crate::ir::Node) {
        let x = layout.x;
        let y = layout.y;
        let w = layout.width;
        let header_h = self.metrics.line_height + self.metrics.header_padding * 2.0;

        // 1. Background (white)
        writeln!(
            svg,
            r#"<rect class="entity-bg" x="{}" y="{}" width="{}" height="{}" rx="4" />"#,
            x, y, w, layout.height
        )
        .unwrap();

        // 2. Header background (gray)
        if node.columns.is_empty() {
            // No columns: header fills entire box
            writeln!(
                svg,
                r#"<rect class="entity-header" x="{}" y="{}" width="{}" height="{}" rx="4" />"#,
                x, y, w, layout.height
            )
            .unwrap();
        } else {
            // With columns: header at top with square bottom corners
            writeln!(
                svg,
                r#"<rect class="entity-header" x="{}" y="{}" width="{}" height="{}" rx="4" />"#,
                x, y, w, header_h
            )
            .unwrap();
            writeln!(
                svg,
                r#"<rect class="entity-header" x="{}" y="{}" width="{}" height="{}" />"#,
                x,
                y + header_h - 4.0,
                w,
                4.0
            )
            .unwrap();
        }

        // 3. Entity name
        let text_y = y + header_h / 2.0 + 5.0;
        writeln!(
            svg,
            r#"<text class="entity-name" x="{}" y="{}" text-anchor="middle">{}</text>"#,
            x + w / 2.0,
            text_y,
            escape_xml(&node.label)
        )
        .unwrap();

        // 4. Separator line and columns
        if !node.columns.is_empty() {
            writeln!(
                svg,
                r#"<line class="entity-separator" x1="{}" y1="{}" x2="{}" y2="{}" />"#,
                x,
                y + header_h,
                x + w,
                y + header_h
            )
            .unwrap();

            let mut col_y = y + header_h + self.metrics.padding_y + self.metrics.line_height * 0.7;
            for col in &node.columns {
                let mut class = "column-text".to_string();
                if col.is_pk {
                    class.push_str(" pk");
                }
                if col.is_fk {
                    class.push_str(" fk");
                }

                let prefix = if col.is_pk { "◆ " } else { "  " };
                let text = format!("{}{}: {}", prefix, col.name, col.typ);

                writeln!(
                    svg,
                    r#"<text class="{}" x="{}" y="{}">{}</text>"#,
                    class,
                    x + self.metrics.padding_x,
                    col_y,
                    escape_xml(&text)
                )
                .unwrap();

                col_y += self.metrics.line_height;
            }
        }

        // 5. Border (drawn last to be on top)
        writeln!(
            svg,
            r#"<rect class="entity-border" x="{}" y="{}" width="{}" height="{}" rx="4" />"#,
            x, y, w, layout.height
        )
        .unwrap();
    }

    fn render_edge_line(&self, svg: &mut String, layout: &LayoutEdge, corner_radius: f64) {
        if layout.waypoints.len() < 2 {
            return;
        }

        // Build SVG path with rounded corners at each waypoint
        let mut path = String::new();
        let r = corner_radius;

        for (i, &(x, y)) in layout.waypoints.iter().enumerate() {
            if i == 0 {
                path.push_str(&format!("M {} {}", num(x), num(y)));
            } else if i == layout.waypoints.len() - 1 {
                // Last point: just line to it
                path.push_str(&format!(" L {} {}", num(x), num(y)));
            } else {
                // Middle point: add rounded corner
                let (px, py) = layout.waypoints[i - 1];
                let (nx, ny) = layout.waypoints[i + 1];

                // Direction from previous point
                let dx1 = x - px;
                let dy1 = y - py;
                let len1 = (dx1 * dx1 + dy1 * dy1).sqrt();

                // Direction to next point
                let dx2 = nx - x;
                let dy2 = ny - y;
                let len2 = (dx2 * dx2 + dy2 * dy2).sqrt();

                if len1 > 0.0 && len2 > 0.0 {
                    // Limit radius to half the segment length
                    let effective_r = r.min(len1 / 2.0).min(len2 / 2.0);

                    // Point before corner
                    let bx = x - (dx1 / len1) * effective_r;
                    let by = y - (dy1 / len1) * effective_r;

                    // Point after corner
                    let ax = x + (dx2 / len2) * effective_r;
                    let ay = y + (dy2 / len2) * effective_r;

                    // Draw line to before corner, then arc to after corner
                    path.push_str(&format!(
                        " L {} {} Q {} {} {} {}",
                        num(bx),
                        num(by),
                        num(x),
                        num(y),
                        num(ax),
                        num(ay)
                    ));
                } else {
                    path.push_str(&format!(" L {} {}", num(x), num(y)));
                }
            }
        }

        writeln!(svg, r#"<path class="edge" d="{}" />"#, path).unwrap();
    }

    /// Work out where an edge's cardinalities and label go, without drawing
    /// them yet: they may still have to slide to avoid one another.
    fn plan_edge_labels(&self, plans: &mut Vec<LabelPlan>, layout: &LayoutEdge, edge: &Edge) {
        if layout.waypoints.len() < 2 {
            return;
        }

        let (x1, y1) = layout.waypoints[0];
        let (x2, y2) = layout.waypoints[layout.waypoints.len() - 1];

        let half_font = CARDINALITY_FONT_SIZE / 2.0;
        let margin = 4.0; // Gap between entity border and text edge

        let from_symbol = cardinality_symbol(edge.from_cardinality);
        let to_symbol = cardinality_symbol(edge.to_cardinality);

        let index = layout.edge_index;

        if layout.is_self_ref && layout.waypoints.len() >= 4 {
            // Self-referential: place cardinalities on the right side of loop
            let loop_x = layout.waypoints[1].0 + margin;

            plans.push(plan_cardinality(loop_x, y1, from_symbol, index, RIGHT, 0.0));
            plans.push(plan_cardinality(loop_x, y2, to_symbol, index, RIGHT, 0.0));

            if let Some(label) = &edge.label {
                let mid_y = (y1 + y2) / 2.0;
                // Keep the whole label right of the loop, off the entity box.
                let label_x =
                    loop_x + monospace_width(label, EDGE_LABEL_FONT_SIZE) / 2.0 + margin;
                let room = (y2 - y1).abs() / 2.0 - EDGE_LABEL_FONT_SIZE;
                plans.push(plan_edge_label(label_x, mid_y, label, index, DOWN, room));
            }
            return;
        }

        // For orthogonal edges, place cardinalities near first/last segments
        // From cardinality: near the start point
        let (p2x, p2y) = layout.waypoints[1];
        let (dx1, dy1) = (p2x - x1, p2y - y1);

        // Position cardinality so edge passes through center of background:
        // along the stub, with the other coordinate left on the edge.
        let (from_pos, from_dir) = if dy1.abs() > dx1.abs() {
            ((x1, y1 + dy1.signum() * (margin + half_font)), (0.0, dy1.signum()))
        } else {
            ((x1 + dx1.signum() * (margin + half_font), y1), (dx1.signum(), 0.0))
        };
        let from_room = dx1.abs().max(dy1.abs()) - (margin + half_font) - half_font;
        plans.push(plan_cardinality(
            from_pos.0,
            from_pos.1,
            from_symbol,
            index,
            from_dir,
            from_room,
        ));

        // To cardinality: near the end point
        let n = layout.waypoints.len();
        let (pn1x, pn1y) = layout.waypoints[n - 2];
        let (dx2, dy2) = (x2 - pn1x, y2 - pn1y);

        let (to_pos, to_dir) = if dy2.abs() > dx2.abs() {
            ((x2, y2 - dy2.signum() * (margin + half_font)), (0.0, -dy2.signum()))
        } else {
            ((x2 - dx2.signum() * (margin + half_font), y2), (-dx2.signum(), 0.0))
        };
        let to_room = dx2.abs().max(dy2.abs()) - (margin + half_font) - half_font;
        plans.push(plan_cardinality(
            to_pos.0,
            to_pos.1,
            to_symbol,
            index,
            to_dir,
            to_room,
        ));

        // Label on the longest segment that can hold it, where it has the most
        // room to slide out of the way of other edges.
        if let Some(label) = &edge.label {
            let label_width = monospace_width(label, EDGE_LABEL_FONT_SIZE) + 6.0;
            let anchor = label_anchor(&layout.waypoints, label_width).unwrap_or((
                ((x1 + x2) / 2.0, (y1 + y2) / 2.0),
                RIGHT,
                MIN_LABEL_ROOM,
            ));
            let ((mid_x, mid_y), dir, room) = anchor;
            plans.push(plan_edge_label(mid_x, mid_y, label, index, dir, room));
        }
    }
}

/// U+2731 HEAVY ASTERISK. The ASCII `*` is drawn small and near the cap height,
/// so it reads as a footnote mark next to the digits; this one sits centered.
const MANY: &str = "\u{2731}";

fn cardinality_symbol(c: Cardinality) -> &'static str {
    match c {
        Cardinality::One => "1",
        Cardinality::ZeroOrOne => "0..1",
        Cardinality::Many => MANY,
        Cardinality::OneOrMore => concat!("1..", "\u{2731}"),
    }
}

/// Format a coordinate with at most one decimal place.
fn num(v: f64) -> String {
    let rounded = (v * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 {
        format!("{}", rounded.trunc())
    } else {
        format!("{:.1}", rounded)
    }
}

/// Smallest distance a label may slide, even on a segment barely longer than
/// the label itself.
const MIN_LABEL_ROOM: f64 = 12.0;

/// Where a relationship label sits: centered on the longest segment that can
/// hold it, preferring horizontal runs so the text reads along the line.
///
/// Returns the position, the direction it may slide, and how far.
fn label_anchor(waypoints: &[(f64, f64)], width: f64) -> Option<((f64, f64), (f64, f64), f64)> {
    let segments = waypoints.windows(2).map(|seg| {
        let horizontal = (seg[0].1 - seg[1].1).abs() < 0.5;
        let length = if horizontal {
            (seg[1].0 - seg[0].0).abs()
        } else {
            (seg[1].1 - seg[0].1).abs()
        };
        let mid = ((seg[0].0 + seg[1].0) / 2.0, (seg[0].1 + seg[1].1) / 2.0);
        (mid, horizontal, length)
    });

    let longer = |a: &(_, _, f64), b: &(_, _, f64)| {
        a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal)
    };

    // A horizontal run wide enough for the whole label, else the longest
    // segment of any direction.
    let fits_horizontally = segments
        .clone()
        .filter(|(_, horizontal, length)| *horizontal && *length >= width)
        .max_by(longer);
    let (mid, horizontal, length) = fits_horizontally.or_else(|| segments.max_by(longer))?;

    let dir = if horizontal { RIGHT } else { DOWN };
    // The label may travel as far as the end of the segment: overhanging a bend
    // slightly beats covering another edge, and `is_clear` still has the veto.
    let room = (length / 2.0).max(MIN_LABEL_ROOM);

    Some((mid, dir, room))
}

/// Width of a monospace string in pixels, counting full-width characters twice.
fn monospace_width(text: &str, font_size: f64) -> f64 {
    UnicodeWidthStr::width(text) as f64 * font_size * 0.6
}


/// Font size of relationship labels, matching the `.edge-label` class.
const EDGE_LABEL_FONT_SIZE: f64 = 14.0;

/// Font size of cardinalities, matching the `.cardinality` class.
const CARDINALITY_FONT_SIZE: f64 = 15.0;

/// Slide directions for labels.
const RIGHT: (f64, f64) = (1.0, 0.0);
const DOWN: (f64, f64) = (0.0, 1.0);

/// How a label may move to escape an overlap.
#[derive(Clone, Copy)]
struct Slide {
    dir: (f64, f64),
    room: f64,
    /// Cardinalities may only move away from their entity; labels in the middle
    /// of a run can go either way.
    both_ways: bool,
    /// Relationship labels may also step off to the side of their line; a
    /// cardinality has to stay on it, since the line marks what it counts.
    sideways: bool,
}

/// A label pill, positioned but not yet drawn.
struct LabelPlan {
    /// Center of the pill.
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    text: String,
    bg_class: &'static str,
    text_class: &'static str,
    /// The edge this label belongs to; its own path is not an obstacle.
    edge_index: usize,
    slide: Option<Slide>,
}

impl LabelPlan {
    /// Pill bounds grown by `gap` on every side.
    fn bounds(&self, gap: f64) -> (f64, f64, f64, f64) {
        (
            self.x - self.width / 2.0 - gap,
            self.y - self.height / 2.0 - gap,
            self.x + self.width / 2.0 + gap,
            self.y + self.height / 2.0 + gap,
        )
    }

    fn overlaps(&self, other: &LabelPlan, gap: f64) -> bool {
        let (l1, t1, r1, b1) = self.bounds(gap);
        let (l2, t2, r2, b2) = other.bounds(0.0);
        l1 < r2 && r1 > l2 && t1 < b2 && b1 > t2
    }
}

fn plan_edge_label(
    x: f64,
    y: f64,
    label: &str,
    edge_index: usize,
    dir: (f64, f64),
    room: f64,
) -> LabelPlan {
    let padding = 3.0;
    LabelPlan {
        x,
        y,
        width: monospace_width(label, EDGE_LABEL_FONT_SIZE) + padding * 2.0,
        height: EDGE_LABEL_FONT_SIZE + padding * 2.0,
        text: label.to_string(),
        bg_class: "edge-label-bg",
        text_class: "edge-label",
        edge_index,
        slide: (room > 0.0).then_some(Slide {
            dir,
            room,
            both_ways: true,
            sideways: true,
        }),
    }
}

fn plan_cardinality(
    x: f64,
    y: f64,
    symbol: &str,
    edge_index: usize,
    dir: (f64, f64),
    room: f64,
) -> LabelPlan {
    let padding = 4.0;
    LabelPlan {
        x,
        y,
        width: monospace_width(symbol, CARDINALITY_FONT_SIZE) + padding * 2.0,
        height: CARDINALITY_FONT_SIZE + padding * 2.0,
        text: symbol.to_string(),
        bg_class: "cardinality-bg",
        text_class: "cardinality",
        edge_index,
        slide: (room > 0.0).then_some(Slide {
            dir,
            room,
            both_ways: false,
            sideways: false,
        }),
    }
}

/// Clearance kept between a label and whatever it must not cover.
const LABEL_GAP: f64 = 3.0;

/// Slide labels along their own edge until they stop covering each other, an
/// unrelated edge, or an entity. A label that finds no free spot stays put.
fn resolve_label_overlaps(plans: &mut [LabelPlan], layout: &Layout) {
    for i in 0..plans.len() {
        if is_clear(plans, i, layout) {
            continue;
        }
        let Some(slide) = plans[i].slide else {
            continue;
        };

        let origin = (plans[i].x, plans[i].y);
        // Perpendicular to the direction the label slides along.
        let side = (slide.dir.1.abs(), slide.dir.0.abs());

        let placed = candidate_offsets(slide).into_iter().any(|(along, across)| {
            plans[i].x = origin.0 + slide.dir.0 * along + side.0 * across;
            plans[i].y = origin.1 + slide.dir.1 * along + side.1 * across;
            is_clear(plans, i, layout)
        });

        if !placed {
            plans[i].x = origin.0;
            plans[i].y = origin.1;
        }
    }
}

/// Positions to try, nearest first: moving along the edge is preferred over
/// stepping off to the side of it.
fn candidate_offsets(slide: Slide) -> Vec<(f64, f64)> {
    const STEP: f64 = 6.0;
    const SIDE_STEP: f64 = 8.0;
    const SIDE_STEPS: i32 = 4;
    const SIDE_COST: f64 = 1.6;

    let along_steps = (slide.room / STEP).floor() as i32;
    let side_steps = if slide.sideways { SIDE_STEPS } else { 0 };

    let mut offsets = Vec::new();
    for a in -along_steps..=along_steps {
        if !slide.both_ways && a < 0 {
            continue;
        }
        for s in -side_steps..=side_steps {
            if a == 0 && s == 0 {
                continue;
            }
            offsets.push((a as f64 * STEP, s as f64 * SIDE_STEP));
        }
    }

    let cost = |(a, s): &(f64, f64)| a.abs() + s.abs() * SIDE_COST;
    offsets.sort_by(|a, b| cost(a).partial_cmp(&cost(b)).unwrap_or(std::cmp::Ordering::Equal));
    offsets
}

/// True when the label covers nothing it should not.
fn is_clear(plans: &[LabelPlan], i: usize, layout: &Layout) -> bool {
    let plan = &plans[i];
    let (l, t, r, b) = plan.bounds(LABEL_GAP);

    if plans[..i].iter().any(|other| plan.overlaps(other, LABEL_GAP)) {
        return false;
    }

    let hits_node = layout.nodes.iter().any(|node| {
        l < node.x + node.width && r > node.x && t < node.y + node.height && b > node.y
    });
    if hits_node {
        return false;
    }

    // The label is meant to interrupt its own edge, but not any other.
    !layout
        .edges
        .iter()
        .filter(|edge| edge.edge_index != plan.edge_index)
        .flat_map(|edge| edge.waypoints.windows(2))
        .any(|seg| {
            let stroke = 1.0;
            let (sl, sr) = (seg[0].0.min(seg[1].0) - stroke, seg[0].0.max(seg[1].0) + stroke);
            let (st, sb) = (seg[0].1.min(seg[1].1) - stroke, seg[0].1.max(seg[1].1) + stroke);
            l < sr && r > sl && t < sb && b > st
        })
}

fn render_label(svg: &mut String, plan: &LabelPlan) {
    writeln!(
        svg,
        r#"<rect class="{}" x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="8" />"#,
        plan.bg_class,
        plan.x - plan.width / 2.0,
        plan.y - plan.height / 2.0,
        plan.width,
        plan.height
    )
    .unwrap();

    writeln!(
        svg,
        r#"<text class="{}" x="{}" y="{}" text-anchor="middle" dominant-baseline="middle">{}</text>"#,
        plan.text_class,
        num(plan.x),
        num(plan.y),
        escape_xml(&plan.text)
    )
    .unwrap();
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::DetailLevel;
    use crate::layout::LayoutEngine;
    use crate::parser::Parser;

    #[test]
    fn test_render_basic() {
        let input = r#"
            entity User {
                id int pk
                name string
            }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();
        let ir = GraphIR::from_schema(&schema, None, DetailLevel::All);
        let layout = LayoutEngine::default().layout(&ir);
        let svg = SvgRenderer::default().render(&ir, &layout);

        assert!(svg.contains("<svg"));
        assert!(svg.contains("User"));
        assert!(svg.contains("</svg>"));
    }

    #[test]
    fn test_render_unicode() {
        let input = r#"
            entity ユーザー {
                名前 文字列
            }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();
        let ir = GraphIR::from_schema(&schema, None, DetailLevel::All);
        let layout = LayoutEngine::default().layout(&ir);
        let svg = SvgRenderer::default().render(&ir, &layout);

        assert!(svg.contains("ユーザー"));
        assert!(svg.contains("名前"));
    }

    #[test]
    fn test_render_with_edges() {
        let input = r#"
            entity User { id int pk }
            entity Order { id int pk }
            rel { User 1 -- * Order : "places" }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();
        let ir = GraphIR::from_schema(&schema, None, DetailLevel::All);
        let layout = LayoutEngine::default().layout(&ir);
        let svg = SvgRenderer::default().render(&ir, &layout);

        assert!(svg.contains("places"));
        assert!(svg.contains(r#"class="edge""#));
    }
}
