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
  .entity-bg {{ fill: #fff; }}
  .entity-header {{ fill: #e0e0e0; }}
  .entity-border {{ fill: none; stroke: #333; stroke-width: 1.5; }}
  .entity-name {{ font-family: monospace; font-size: 14px; font-weight: bold; }}
  .column-text {{ font-family: monospace; font-size: 12px; }}
  .pk {{ font-weight: bold; }}
  .fk {{ font-style: italic; }}
  .edge {{ stroke: #666; stroke-width: 1.5; fill: none; }}
  .edge-label-bg {{ fill: rgba(234,234,234,0.9); }}
  .edge-label {{ font-family: monospace; font-size: 14px; fill: #444; }}
  .cardinality-bg {{ fill: rgba(224,224,224,0.95); }}
  .cardinality {{ font-family: monospace; font-size: 15px; font-weight: bold; fill: #222; }}
</style>"#
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

        // 3. Render edge labels and cardinalities (on top of everything)
        for edge in &layout.edges {
            if let Some(ir_edge) = ir.edges.get(edge.edge_index) {
                self.render_edge_labels(&mut svg, edge, ir_edge);
            }
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
                r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="#333" stroke-width="1" />"##,
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

    fn render_edge_labels(&self, svg: &mut String, layout: &LayoutEdge, edge: &Edge) {
        if layout.waypoints.len() < 2 {
            return;
        }

        let (x1, y1) = layout.waypoints[0];
        let (x2, y2) = layout.waypoints[layout.waypoints.len() - 1];

        let font_size = 15.0;
        let half_font = font_size / 2.0;
        let margin = 4.0; // Gap between entity border and text edge

        let from_symbol = cardinality_symbol(edge.from_cardinality);
        let to_symbol = cardinality_symbol(edge.to_cardinality);

        if layout.is_self_ref && layout.waypoints.len() >= 4 {
            // Self-referential: place cardinalities on the right side of loop
            let loop_x = layout.waypoints[1].0 + margin;

            render_cardinality(svg, loop_x, y1, "start", from_symbol, font_size);
            render_cardinality(svg, loop_x, y2, "start", to_symbol, font_size);

            if let Some(label) = &edge.label {
                let mid_y = (y1 + y2) / 2.0;
                // Keep the whole label right of the loop, off the entity box.
                let label_x =
                    loop_x + monospace_width(label, EDGE_LABEL_FONT_SIZE) / 2.0 + margin;
                render_edge_label(svg, label_x, mid_y, label);
            }
        } else {
            // For orthogonal edges, place cardinalities near first/last segments
            // From cardinality: near the start point
            let (p1x, p1y) = layout.waypoints[0];
            let (p2x, p2y) = layout.waypoints[1];
            let dx1 = p2x - p1x;
            let dy1 = p2y - p1y;

            // Position cardinality so edge passes through center of background
            // For vertical edges: x = edge x, offset y only
            // For horizontal edges: y = edge y, offset x only
            let (from_x, from_y) = if dy1.abs() > dx1.abs() {
                // Vertical edge: keep x aligned with edge, offset y
                (p1x, p1y + dy1.signum() * (margin + half_font))
            } else {
                // Horizontal edge: keep y aligned with edge, offset x
                (p1x + dx1.signum() * (margin + half_font), p1y)
            };

            render_cardinality(svg, from_x, from_y, "middle", from_symbol, font_size);

            // To cardinality: near the end point
            let n = layout.waypoints.len();
            let (pnx, pny) = layout.waypoints[n - 1];
            let (pn1x, pn1y) = layout.waypoints[n - 2];
            let dx2 = pnx - pn1x;
            let dy2 = pny - pn1y;

            // Position cardinality so edge passes through center of background
            let (to_x, to_y) = if dy2.abs() > dx2.abs() {
                // Vertical edge: keep x aligned with edge, offset y
                (pnx, pny - dy2.signum() * (margin + half_font))
            } else {
                // Horizontal edge: keep y aligned with edge, offset x
                (pnx - dx2.signum() * (margin + half_font), pny)
            };

            render_cardinality(svg, to_x, to_y, "middle", to_symbol, font_size);

            // Label on the longest horizontal run, where it has the most room
            if let Some(label) = &edge.label {
                let (mid_x, mid_y) = longest_horizontal_midpoint(&layout.waypoints)
                    .unwrap_or(((x1 + x2) / 2.0, (y1 + y2) / 2.0));
                render_edge_label(svg, mid_x, mid_y, label);
            }
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

/// Midpoint of the longest horizontal segment, if the path has one.
fn longest_horizontal_midpoint(waypoints: &[(f64, f64)]) -> Option<(f64, f64)> {
    waypoints
        .windows(2)
        .filter(|seg| (seg[0].1 - seg[1].1).abs() < 0.5)
        .max_by(|a, b| {
            let len = |s: &[(f64, f64)]| (s[1].0 - s[0].0).abs();
            len(a).partial_cmp(&len(b)).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|seg| ((seg[0].0 + seg[1].0) / 2.0, seg[0].1))
}

/// Width of a monospace string in pixels, counting full-width characters twice.
fn monospace_width(text: &str, font_size: f64) -> f64 {
    UnicodeWidthStr::width(text) as f64 * font_size * 0.6
}


/// Font size of relationship labels, matching the `.edge-label` class.
const EDGE_LABEL_FONT_SIZE: f64 = 14.0;

/// Render edge label with semi-transparent background
fn render_edge_label(svg: &mut String, x: f64, y: f64, label: &str) {
    let font_size = EDGE_LABEL_FONT_SIZE;
    let text_width = monospace_width(label, font_size);
    let text_height = font_size;
    let padding = 3.0;

    // Rect centered on (x, y)
    let rect_x = x - text_width / 2.0 - padding;
    let rect_y = y - text_height / 2.0 - padding;
    let rect_w = text_width + padding * 2.0;
    let rect_h = text_height + padding * 2.0;

    // Background rect
    writeln!(
        svg,
        r#"<rect class="edge-label-bg" x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="8" />"#,
        rect_x, rect_y, rect_w, rect_h
    )
    .unwrap();

    // Text centered at (x, y)
    writeln!(
        svg,
        r#"<text class="edge-label" x="{}" y="{}" text-anchor="middle" dominant-baseline="middle">{}</text>"#,
        num(x),
        num(y),
        escape_xml(label)
    )
    .unwrap();
}

/// Render cardinality label with semi-transparent background
/// Text is always centered (text-anchor="middle") so edge passes through center
fn render_cardinality(
    svg: &mut String,
    x: f64,
    y: f64,
    _anchor: &str, // Always use "middle"
    symbol: &str,
    font_size: f64,
) {
    let text_width = monospace_width(symbol, font_size);
    let text_height = font_size;
    let padding = 4.0;

    // Rect centered on (x, y)
    let rect_x = x - text_width / 2.0 - padding;
    let rect_y = y - text_height / 2.0 - padding;
    let rect_w = text_width + padding * 2.0;
    let rect_h = text_height + padding * 2.0;

    // Background rect
    writeln!(
        svg,
        r#"<rect class="cardinality-bg" x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="8" />"#,
        rect_x, rect_y, rect_w, rect_h
    )
    .unwrap();

    // Text centered at (x, y)
    writeln!(
        svg,
        r#"<text class="cardinality" x="{}" y="{}" text-anchor="middle" dominant-baseline="middle">{}</text>"#,
        num(x),
        num(y),
        symbol
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
