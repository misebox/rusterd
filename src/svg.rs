use crate::ast::Cardinality;
use crate::ir::{Edge, GraphIR};
use crate::layout::{Layout, LayoutEdge, LayoutNode};
use crate::measure::TextMetrics;
use std::collections::HashMap;
use std::fmt::Write;

pub struct SvgRenderer {
    metrics: TextMetrics,
}

impl Default for SvgRenderer {
    fn default() -> Self {
        Self {
            metrics: TextMetrics::default(),
        }
    }
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
  .edge-label {{ font-family: monospace; font-size: 12px; fill: #666; paint-order: stroke; stroke: rgba(255,255,255,0.85); stroke-width: 3px; }}
  .cardinality {{ font-family: monospace; font-size: 15px; font-weight: bold; fill: #333; paint-order: stroke; stroke: rgba(255,255,255,0.85); stroke-width: 4px; }}
</style>"#
        )
        .unwrap();

        // Build node lookup
        let node_map: HashMap<&str, &crate::ir::Node> =
            ir.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

        // 1. Render edge lines (behind nodes)
        for edge in &layout.edges {
            self.render_edge_line(&mut svg, edge);
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

    fn render_edge_line(&self, svg: &mut String, layout: &LayoutEdge) {
        let (x1, y1) = layout.from_point;
        let (x2, y2) = layout.to_point;

        if layout.is_self_ref {
            if let Some([(cx1, cy1), (cx2, cy2)]) = layout.control_points {
                writeln!(
                    svg,
                    r#"<path class="edge" d="M {} {} C {} {}, {} {}, {} {}" />"#,
                    x1, y1, cx1, cy1, cx2, cy2, x2, y2
                )
                .unwrap();
            }
        } else {
            writeln!(
                svg,
                r#"<line class="edge" x1="{}" y1="{}" x2="{}" y2="{}" />"#,
                x1, y1, x2, y2
            )
            .unwrap();
        }
    }

    fn render_edge_labels(&self, svg: &mut String, layout: &LayoutEdge, edge: &Edge) {
        let (x1, y1) = layout.from_point;
        let (x2, y2) = layout.to_point;

        let font_size = 15.0;
        let margin = font_size * 0.5;

        let from_symbol = cardinality_symbol(edge.from_cardinality);
        let to_symbol = cardinality_symbol(edge.to_cardinality);

        if layout.is_self_ref {
            // Self-referential: place cardinalities on the loop
            if let Some([(cx1, _), (cx2, _)]) = layout.control_points {
                let loop_x = (cx1 + cx2) / 2.0 + margin;

                // From cardinality near top of loop
                writeln!(
                    svg,
                    r#"<text class="cardinality" x="{}" y="{}" text-anchor="start" dominant-baseline="middle">{}</text>"#,
                    loop_x, y1, from_symbol
                )
                .unwrap();

                // To cardinality near bottom of loop
                writeln!(
                    svg,
                    r#"<text class="cardinality" x="{}" y="{}" text-anchor="start" dominant-baseline="middle">{}</text>"#,
                    loop_x, y2, to_symbol
                )
                .unwrap();

                // Label at center of loop
                if let Some(label) = &edge.label {
                    let mid_y = (y1 + y2) / 2.0;
                    writeln!(
                        svg,
                        r#"<text class="edge-label" x="{}" y="{}" text-anchor="start" dominant-baseline="middle">{}</text>"#,
                        loop_x,
                        mid_y,
                        escape_xml(label)
                    )
                    .unwrap();
                }
            }
        } else {
            let dx = x2 - x1;
            let dy = y2 - y1;
            let len = (dx * dx + dy * dy).sqrt();
            if len > 0.0 {
                let ux = dx / len;
                let uy = dy / len;

                let from_anchor = if ux >= 0.0 { "start" } else { "end" };
                let from_baseline = if uy >= 0.0 { "hanging" } else { "alphabetic" };
                let from_x = x1 + ux * margin;
                let from_y = y1 + uy * margin;

                writeln!(
                    svg,
                    r#"<text class="cardinality" x="{}" y="{}" text-anchor="{}" dominant-baseline="{}">{}</text>"#,
                    from_x, from_y, from_anchor, from_baseline, from_symbol
                )
                .unwrap();

                let to_anchor = if ux >= 0.0 { "end" } else { "start" };
                let to_baseline = if uy >= 0.0 { "alphabetic" } else { "hanging" };
                let to_x = x2 - ux * margin;
                let to_y = y2 - uy * margin;

                writeln!(
                    svg,
                    r#"<text class="cardinality" x="{}" y="{}" text-anchor="{}" dominant-baseline="{}">{}</text>"#,
                    to_x, to_y, to_anchor, to_baseline, to_symbol
                )
                .unwrap();

                if let Some(label) = &edge.label {
                    let mid_x = (x1 + x2) / 2.0;
                    let mid_y = (y1 + y2) / 2.0;
                    writeln!(
                        svg,
                        r#"<text class="edge-label" x="{}" y="{}" text-anchor="middle" dominant-baseline="middle">{}</text>"#,
                        mid_x,
                        mid_y,
                        escape_xml(label)
                    )
                    .unwrap();
                }
            }
        }
    }
}

fn cardinality_symbol(c: Cardinality) -> &'static str {
    match c {
        Cardinality::One => "1",
        Cardinality::ZeroOrOne => "0..1",
        Cardinality::Many => "*",
        Cardinality::OneOrMore => "1..*",
    }
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
