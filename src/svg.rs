use crate::ast::Cardinality;
use crate::ir::{Edge, GraphIR};
use crate::layout::{Layout, LayoutEdge, LayoutNode};
use crate::measure::{BADGE_SLOT, BADGE_WIDTH, NOTE_LINE, TextMetrics};
use crate::theme::{GROUP_COLOURS, Theme};
use std::collections::HashMap;
use std::fmt::Write;
use unicode_width::UnicodeWidthStr;

/// How the cardinality of each edge end is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Notation {
    /// Crow's foot (IE) symbols on the line itself.
    #[default]
    CrowsFoot,
    /// `1`, `0..1`, `*`, `1..*` written in a pill next to the line.
    Text,
}

impl Notation {
    /// The name the CLI and the options object use for each notation.
    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "crowsfoot" | "crows-foot" | "ie" => Some(Self::CrowsFoot),
            "text" => Some(Self::Text),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct SvgRenderer {
    metrics: TextMetrics,
    notation: Notation,
    legend: bool,
    theme: Theme,
}

impl SvgRenderer {
    pub fn with_notation(mut self, notation: Notation) -> Self {
        self.notation = notation;
        self
    }

    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Add a key to the four cardinalities, in the notation being drawn.
    pub fn with_legend(mut self, legend: bool) -> Self {
        self.legend = legend;
        self
    }

    pub fn render(&self, ir: &GraphIR, layout: &Layout) -> String {
        let mut svg = String::new();

        // The keys stand in bands below the diagram, where they cannot land on
        // an entity whatever the diagram turns out to look like: first what
        // each group's colour stands for, then the cardinalities if asked
        // for. A narrow diagram widens to hold them.
        let (group_starts, group_width) = group_key_layout(ir);
        let group_band = if ir.groups.is_empty() {
            0.0
        } else {
            GROUP_KEY_HEIGHT
        };
        let mut height = layout.height + group_band;
        let mut width = layout.width.max(group_width);
        if self.legend {
            height += LEGEND_HEIGHT;
            width = width.max(self.legend_layout().1);
        }

        writeln!(
            &mut svg,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">"#,
            num(width), num(height), num(width), num(height)
        )
        .unwrap();

        // Style
        writeln!(
            &mut svg,
            r#"<style>
  .entity-border {{ fill: none; stroke-width: 1.5; }}
  .entity-separator {{ stroke-width: 1; }}
  .entity-name {{ font-family: monospace; font-size: 14px; font-weight: bold; }}
  .column-text {{ font-family: monospace; font-size: 12px; }}
  .column-type {{ font-family: monospace; font-size: 12px; }}
  .entity-note {{ font-family: monospace; font-size: 11px; }}
  .column-note {{ font-family: monospace; font-size: 12px; font-style: italic; }}
  .pk {{ font-weight: bold; }}
  .badge-flag {{ fill: none; stroke-width: 1; }}
  .badge-pk-text, .badge-fk-text, .badge-flag-text {{ font-family: monospace; font-size: 9px; font-weight: bold; }}
  .edge {{ stroke-width: 1.5; fill: none; }}
  .edge-symbol {{ stroke-width: 1.5; fill: none; }}
  .edge-symbol-zero {{ stroke-width: 1.5; }}
  .edge-label {{ font-family: monospace; font-size: 14px; }}
  .cardinality {{ font-family: monospace; font-size: 15px; font-weight: bold; }}
  .group-frame {{ fill-opacity: 0.05; stroke-width: 1.5; stroke-dasharray: 6 4; }}
  .group-label {{ font-family: monospace; font-size: 13px; font-weight: bold; }}
{}</style>"#,
            self.theme.css()
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

        // 0. Frames around the groups, behind everything else
        for frame in plan_group_frames(ir, layout) {
            render_group_frame(&mut svg, &frame);
        }

        // 1. Render edge lines (behind nodes)
        for edge in &layout.edges {
            self.render_edge_line(&mut svg, edge, layout.corner_radius);
            if self.notation == Notation::CrowsFoot
                && let Some(ir_edge) = ir.edges.get(edge.edge_index)
            {
                self.render_edge_ends(&mut svg, edge, ir_edge);
            }
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

        // 4. The keys
        if !ir.groups.is_empty() {
            render_group_key(&mut svg, ir, &group_starts, layout.height, width);
        }
        if self.legend {
            self.render_legend(&mut svg, layout.height + group_band, width);
        }

        writeln!(&mut svg, "</svg>").unwrap();
        svg
    }

    /// Where each entry in the key starts, and how much room the key needs.
    fn legend_layout(&self) -> (Vec<f64>, f64) {
        let mut starts = Vec::with_capacity(CARDINALITY_MEANINGS.len());
        let mut x = LEGEND_MARGIN;
        for (_, meaning) in CARDINALITY_MEANINGS {
            starts.push(x);
            x += LEGEND_STUB
                + LEGEND_ITEM_GAP / 2.0
                + monospace_width(meaning, LEGEND_FONT_SIZE)
                + LEGEND_ITEM_GAP;
        }
        (starts, x - LEGEND_ITEM_GAP + LEGEND_MARGIN)
    }

    /// Draw a key to the four cardinalities, each shown the way the diagram
    /// draws it, so the reader can match a mark on a line to its meaning.
    fn render_legend(&self, svg: &mut String, top: f64, width: f64) {
        let y = top + LEGEND_HEIGHT / 2.0;
        let (starts, _) = self.legend_layout();

        writeln!(
            svg,
            r#"<line class="entity-separator" x1="{}" y1="{}" x2="{}" y2="{}" />"#,
            num(LEGEND_MARGIN),
            num(top),
            num(width - LEGEND_MARGIN),
            num(top)
        )
        .unwrap();

        for (&x, (cardinality, meaning)) in starts.iter().zip(CARDINALITY_MEANINGS) {
            let symbol_x = x + LEGEND_STUB;

            if self.notation == Notation::CrowsFoot {
                // A stub of line running into a piece of entity border, so the
                // mark stands exactly as it does in the diagram.
                writeln!(
                    svg,
                    r#"<path class="edge" d="M {} {} L {} {}" />"#,
                    num(x),
                    num(y),
                    num(symbol_x),
                    num(y)
                )
                .unwrap();
                writeln!(
                    svg,
                    r#"<line class="entity-border" x1="{}" y1="{}" x2="{}" y2="{}" />"#,
                    num(symbol_x),
                    num(y - LEGEND_BORDER_HALF),
                    num(symbol_x),
                    num(y + LEGEND_BORDER_HALF)
                )
                .unwrap();
                render_crows_foot(svg, (symbol_x, y), (-1.0, 0.0), *cardinality);
            } else {
                let symbol = cardinality_symbol(*cardinality);
                render_label(
                    svg,
                    &plan_cardinality(x + LEGEND_STUB / 2.0, y, symbol, 0, RIGHT, 0.0),
                );
            }

            writeln!(
                svg,
                r#"<text class="column-text" x="{}" y="{}">{}</text>"#,
                num(symbol_x + LEGEND_ITEM_GAP / 2.0),
                num(y + LEGEND_TEXT_BASELINE),
                escape_xml(meaning)
            )
            .unwrap();
        }
    }

    fn render_node(&self, svg: &mut String, layout: &LayoutNode, node: &crate::ir::Node) {
        let x = layout.x;
        let y = layout.y;
        let w = layout.width;
        let header_h = self.metrics.header_height(node.note.is_some());

        // 1. Background (white)
        writeln!(
            svg,
            r#"<rect class="entity-bg" x="{}" y="{}" width="{}" height="{}" rx="4" />"#,
            x, y, w, layout.height
        )
        .unwrap();

        // 2. Header background, in its group's colour if it has one
        let header_class = match node.group {
            Some(group) => format!("entity-header group-{}", group % GROUP_COLOURS),
            None => "entity-header".to_string(),
        };
        if node.columns.is_empty() {
            // No columns: header fills entire box
            writeln!(
                svg,
                r#"<rect class="{}" x="{}" y="{}" width="{}" height="{}" rx="4" />"#,
                header_class, x, y, w, layout.height
            )
            .unwrap();
        } else {
            // With columns: header at top with square bottom corners
            writeln!(
                svg,
                r#"<rect class="{}" x="{}" y="{}" width="{}" height="{}" rx="4" />"#,
                header_class, x, y, w, header_h
            )
            .unwrap();
            writeln!(
                svg,
                r#"<rect class="{}" x="{}" y="{}" width="{}" height="{}" />"#,
                header_class,
                x,
                y + header_h - 4.0,
                w,
                4.0
            )
            .unwrap();
        }

        // 3. Entity name, and its note under it. With no columns under them
        // they stand in the middle of the box, which is all header.
        let name_h = self.metrics.header_height(false);
        let top = if node.columns.is_empty() {
            y + (layout.height - header_h) / 2.0
        } else {
            y
        };
        let text_y = top + name_h / 2.0 + 5.0;
        writeln!(
            svg,
            r#"<text class="entity-name" x="{}" y="{}" text-anchor="middle">{}</text>"#,
            x + w / 2.0,
            text_y,
            escape_xml(&node.label)
        )
        .unwrap();

        if let Some(note) = &node.note {
            writeln!(
                svg,
                r#"<text class="entity-note" x="{}" y="{}" text-anchor="middle">{}</text>"#,
                num(x + w / 2.0),
                num(top + name_h + NOTE_LINE / 2.0 - 4.0),
                escape_xml(note)
            )
            .unwrap();
        }

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

            let grid = self.metrics.column_grid(&node.columns);
            let mut col_y = y + header_h + self.metrics.padding_y + self.metrics.line_height * 0.7;
            for col in &node.columns {
                let mut keys = Vec::new();
                if col.is_pk {
                    keys.push(("PK", "badge-pk"));
                }
                if col.is_fk {
                    keys.push(("FK", "badge-fk"));
                }
                let mut flags = Vec::new();
                if col.is_unique {
                    flags.push(("UQ", "badge-flag"));
                }
                for (i, (text, class)) in keys.iter().enumerate() {
                    render_badge(
                        svg,
                        x + grid.key_x + i as f64 * BADGE_SLOT,
                        col_y,
                        text,
                        class,
                    );
                }
                for (i, (text, class)) in flags.iter().enumerate() {
                    render_badge(
                        svg,
                        x + grid.flag_x + i as f64 * BADGE_SLOT,
                        col_y,
                        text,
                        class,
                    );
                }

                let class = if col.is_pk {
                    "column-text pk"
                } else {
                    "column-text"
                };
                writeln!(
                    svg,
                    r#"<text class="{}" x="{}" y="{}">{}</text>"#,
                    class,
                    num(x + grid.name_x),
                    num(col_y),
                    escape_xml(&col.name)
                )
                .unwrap();
                writeln!(
                    svg,
                    r#"<text class="column-type" x="{}" y="{}">{}</text>"#,
                    num(x + grid.type_x),
                    num(col_y),
                    escape_xml(&col.typ)
                )
                .unwrap();
                if let Some(note) = &col.note {
                    writeln!(
                        svg,
                        r#"<text class="column-note" x="{}" y="{}">{}</text>"#,
                        num(x + grid.note_x),
                        num(col_y),
                        escape_xml(note)
                    )
                    .unwrap();
                }

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

    /// Draw the crow's foot symbol at both ends of an edge.
    fn render_edge_ends(&self, svg: &mut String, layout: &LayoutEdge, edge: &Edge) {
        let points = &layout.waypoints;
        if points.len() < 2 {
            return;
        }

        // Unit direction from an entity border along the line, away from it.
        let outward = |from: (f64, f64), towards: (f64, f64)| {
            let (dx, dy) = (towards.0 - from.0, towards.1 - from.1);
            let length = (dx * dx + dy * dy).sqrt();
            if length < f64::EPSILON {
                (0.0, 0.0)
            } else {
                (dx / length, dy / length)
            }
        };

        let last = points.len() - 1;
        render_crows_foot(
            svg,
            points[0],
            outward(points[0], points[1]),
            edge.from_cardinality,
        );
        render_crows_foot(
            svg,
            points[last],
            outward(points[last], points[last - 1]),
            edge.to_cardinality,
        );
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

        // Crow's foot draws the cardinality on the line, so only the
        // relationship label needs a pill.
        let text_cardinalities = self.notation == Notation::Text;

        if layout.is_self_ref && layout.waypoints.len() >= 4 {
            // Self-referential: place cardinalities on the right side of loop
            let loop_x = layout.waypoints[1].0 + margin;

            if text_cardinalities {
                plans.push(plan_cardinality(loop_x, y1, from_symbol, index, RIGHT, 0.0));
                plans.push(plan_cardinality(loop_x, y2, to_symbol, index, RIGHT, 0.0));
            }

            if let Some(label) = &edge.label {
                let mid_y = (y1 + y2) / 2.0;
                // Keep the whole label right of the loop, off the entity box.
                let label_x = loop_x + monospace_width(label, EDGE_LABEL_FONT_SIZE) / 2.0 + margin;
                let room = (y2 - y1).abs() / 2.0 - EDGE_LABEL_FONT_SIZE;
                plans.push(plan_edge_label(label_x, mid_y, label, index, DOWN, room));
            }
            return;
        }

        if text_cardinalities {
            // For orthogonal edges, place cardinalities near first/last segments
            // From cardinality: near the start point
            let (p2x, p2y) = layout.waypoints[1];
            let (dx1, dy1) = (p2x - x1, p2y - y1);

            // Position cardinality so edge passes through center of background:
            // along the stub, with the other coordinate left on the edge.
            let (from_pos, from_dir) = if dy1.abs() > dx1.abs() {
                (
                    (x1, y1 + dy1.signum() * (margin + half_font)),
                    (0.0, dy1.signum()),
                )
            } else {
                (
                    (x1 + dx1.signum() * (margin + half_font), y1),
                    (dx1.signum(), 0.0),
                )
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
                (
                    (x2, y2 - dy2.signum() * (margin + half_font)),
                    (0.0, -dy2.signum()),
                )
            } else {
                (
                    (x2 - dx2.signum() * (margin + half_font), y2),
                    (-dx2.signum(), 0.0),
                )
            };
            let to_room = dx2.abs().max(dy2.abs()) - (margin + half_font) - half_font;
            plans.push(plan_cardinality(
                to_pos.0, to_pos.1, to_symbol, index, to_dir, to_room,
            ));
        }

        // Label on the longest segment that can hold it, where it has the most
        // room to slide out of the way of other edges.
        if let Some(label) = &edge.label {
            let label_width = monospace_width(label, EDGE_LABEL_FONT_SIZE) + 6.0;
            let anchor = label_anchor(&layout.waypoints, label_width).unwrap_or(LabelAnchor {
                at: ((x1 + x2) / 2.0, (y1 + y2) / 2.0),
                along: RIGHT,
                room: MIN_LABEL_ROOM,
            });
            plans.push(plan_edge_label(
                anchor.at.0,
                anchor.at.1,
                label,
                index,
                anchor.along,
                anchor.room,
            ));
        }
    }
}

/// How far out along the line the crow's foot reaches.
const FOOT_LENGTH: f64 = 13.0;

/// Half the width of a tick.
const SYMBOL_HALF_WIDTH: f64 = 7.5;

/// Half the spread of the crow's foot where it meets the entity. Wider than a
/// tick, or the two read as the same mark at a glance.
const FOOT_HALF_WIDTH: f64 = 9.0;

/// Distance from the entity border to a tick drawn on its own.
const TICK_INSET: f64 = 10.0;

/// Gap between the crow's foot and the tick that makes it "one or more".
const TICK_GAP: f64 = 6.0;

/// Radius of the "zero" circle.
const ZERO_RADIUS: f64 = 4.0;

/// Draw the crow's foot symbol for one end of an edge.
///
/// `at` is the point on the entity border and `out` the unit direction along
/// the line, away from that entity.
fn render_crows_foot(svg: &mut String, at: (f64, f64), out: (f64, f64), cardinality: Cardinality) {
    // Along the line, and across it.
    let along = |d: f64| (at.0 + out.0 * d, at.1 + out.1 * d);
    let across = (-out.1, out.0);

    let tick = |svg: &mut String, distance: f64| {
        let (cx, cy) = along(distance);
        writeln!(
            svg,
            r#"<path class="edge-symbol" d="M {} {} L {} {}" />"#,
            num(cx - across.0 * SYMBOL_HALF_WIDTH),
            num(cy - across.1 * SYMBOL_HALF_WIDTH),
            num(cx + across.0 * SYMBOL_HALF_WIDTH),
            num(cy + across.1 * SYMBOL_HALF_WIDTH)
        )
        .unwrap();
    };

    // The toes rest on the entity and the ankle sits out on the line: the line
    // itself is the middle toe. Drawn the other way up it reads as an arrowhead
    // pointing at the entity rather than as a foot standing on it.
    let foot = |svg: &mut String| {
        let (ankle_x, ankle_y) = along(FOOT_LENGTH);
        for side in [-1.0, 1.0] {
            writeln!(
                svg,
                r#"<path class="edge-symbol" d="M {} {} L {} {}" />"#,
                num(ankle_x),
                num(ankle_y),
                num(at.0 + across.0 * FOOT_HALF_WIDTH * side),
                num(at.1 + across.1 * FOOT_HALF_WIDTH * side)
            )
            .unwrap();
        }
    };

    // The circle is opaque so the line does not show through it.
    let zero = |svg: &mut String, distance: f64| {
        let (cx, cy) = along(distance + ZERO_RADIUS);
        writeln!(
            svg,
            r#"<circle class="edge-symbol-zero" cx="{}" cy="{}" r="{}" />"#,
            num(cx),
            num(cy),
            num(ZERO_RADIUS)
        )
        .unwrap();
    };

    match cardinality {
        Cardinality::One => tick(svg, TICK_INSET),
        Cardinality::ZeroOrOne => {
            tick(svg, TICK_INSET);
            zero(svg, TICK_INSET + 2.0);
        }
        Cardinality::Many => foot(svg),
        Cardinality::OneOrMore => {
            foot(svg);
            // Clear of the ankle, or the bar closes the foot into a triangle.
            tick(svg, FOOT_LENGTH + TICK_GAP);
        }
    }
}

/// Height of the band the key stands in, below the diagram.
const LEGEND_HEIGHT: f64 = 56.0;

/// Length of the stub of line the key draws each mark on.
const LEGEND_STUB: f64 = 34.0;

/// Half the height of the piece of entity border in the key.
const LEGEND_BORDER_HALF: f64 = 14.0;

/// Space between one entry in the key and the next.
const LEGEND_ITEM_GAP: f64 = 32.0;

/// Margin between the key and the edge of the drawing.
const LEGEND_MARGIN: f64 = 40.0;

/// Size of the explanations in the key, matching a column of an entity.
const LEGEND_FONT_SIZE: f64 = 12.0;

/// Where the baseline of an explanation sits relative to the line it labels.
const LEGEND_TEXT_BASELINE: f64 = 4.0;

/// What each cardinality means, for the key.
const CARDINALITY_MEANINGS: &[(Cardinality, &str)] = &[
    (Cardinality::One, "exactly one"),
    (Cardinality::ZeroOrOne, "zero or one"),
    (Cardinality::Many, "many"),
    (Cardinality::OneOrMore, "one or more"),
];

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

/// Where a relationship label sits, and how far it may move from there.
struct LabelAnchor {
    /// The middle of the segment the label is centered on.
    at: (f64, f64),
    /// The direction it may slide along, away from other edges.
    along: (f64, f64),
    /// How far it may slide, either way.
    room: f64,
}

/// Where a relationship label sits: centered on the longest segment that can
/// hold it, preferring horizontal runs so the text reads along the line.
fn label_anchor(waypoints: &[(f64, f64)], width: f64) -> Option<LabelAnchor> {
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

    Some(LabelAnchor {
        at: mid,
        along: dir,
        room,
    })
}

/// Width of a monospace string in pixels, counting full-width characters twice.
const GROUP_KEY_HEIGHT: f64 = 40.0;
const GROUP_SWATCH: f64 = 14.0;

/// Where each group's entry in the key starts, and how wide the key is.
fn group_key_layout(ir: &GraphIR) -> (Vec<f64>, f64) {
    let mut starts = Vec::with_capacity(ir.groups.len());
    let mut x = LEGEND_MARGIN;
    for group in &ir.groups {
        starts.push(x);
        x += GROUP_SWATCH
            + LEGEND_ITEM_GAP / 4.0
            + monospace_width(&group.name, LEGEND_FONT_SIZE)
            + LEGEND_ITEM_GAP;
    }
    (starts, x - LEGEND_ITEM_GAP + LEGEND_MARGIN)
}

/// Say which colour is which group: a swatch of its headers and its name.
fn render_group_key(svg: &mut String, ir: &GraphIR, starts: &[f64], top: f64, width: f64) {
    let y = top + GROUP_KEY_HEIGHT / 2.0;
    writeln!(
        svg,
        r#"<line class="entity-separator" x1="{}" y1="{}" x2="{}" y2="{}" />"#,
        num(LEGEND_MARGIN),
        num(top),
        num(width - LEGEND_MARGIN),
        num(top)
    )
    .unwrap();
    for (i, (group, &x)) in ir.groups.iter().zip(starts).enumerate() {
        writeln!(
            svg,
            r#"<rect class="entity-header group-{} entity-border" x="{}" y="{}" width="{}" height="{}" rx="3" />"#,
            i % GROUP_COLOURS,
            num(x),
            num(y - GROUP_SWATCH / 2.0),
            num(GROUP_SWATCH),
            num(GROUP_SWATCH)
        )
        .unwrap();
        writeln!(
            svg,
            r#"<text class="column-text" x="{}" y="{}">{}</text>"#,
            num(x + GROUP_SWATCH + LEGEND_ITEM_GAP / 4.0),
            num(y + LEGEND_TEXT_BASELINE),
            escape_xml(&group.name)
        )
        .unwrap();
    }
}

/// Room a group frame leaves around its entities, and above them for its name.
const FRAME_PAD: f64 = 14.0;
const FRAME_TITLE: f64 = 22.0;

/// A frame drawn around one group's entities.
struct GroupFrame<'a> {
    colour: usize,
    name: &'a str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl GroupFrame<'_> {
    fn overlaps(&self, x: f64, y: f64, width: f64, height: f64) -> bool {
        self.x < x + width
            && x < self.x + self.width
            && self.y < y + height
            && y < self.y + self.height
    }
}

/// The frames worth drawing. Placement does not know about frames, so a
/// group's entities may end up with another entity among them; a frame that
/// would take in an entity from outside the group, or cross another frame,
/// would say something untrue, and is left out. The colour of the headers
/// still says which entities belong together.
fn plan_group_frames<'a>(ir: &'a GraphIR, layout: &Layout) -> Vec<GroupFrame<'a>> {
    let mut frames: Vec<GroupFrame> = ir
        .groups
        .iter()
        .enumerate()
        .filter_map(|(i, group)| {
            let members: Vec<&LayoutNode> = layout
                .nodes
                .iter()
                .filter(|n| group.members.contains(&n.id))
                .collect();
            let left = members.iter().map(|n| n.x).reduce(f64::min)?;
            let top = members.iter().map(|n| n.y).reduce(f64::min)?;
            let right = members.iter().map(|n| n.x + n.width).reduce(f64::max)?;
            let bottom = members.iter().map(|n| n.y + n.height).reduce(f64::max)?;
            let frame = GroupFrame {
                colour: i % GROUP_COLOURS,
                name: &group.name,
                x: left - FRAME_PAD,
                y: top - FRAME_PAD - FRAME_TITLE,
                width: right - left + FRAME_PAD * 2.0,
                height: bottom - top + FRAME_PAD * 2.0 + FRAME_TITLE,
            };
            let intruder = layout.nodes.iter().any(|n| {
                !group.members.contains(&n.id) && frame.overlaps(n.x, n.y, n.width, n.height)
            });
            (!intruder).then_some(frame)
        })
        .collect();

    let crossed: Vec<bool> = (0..frames.len())
        .map(|i| {
            (0..frames.len()).any(|j| {
                let other = &frames[j];
                i != j && frames[i].overlaps(other.x, other.y, other.width, other.height)
            })
        })
        .collect();
    let mut keep = crossed.iter().map(|c| !c);
    frames.retain(|_| keep.next().unwrap_or(false));
    frames
}

fn render_group_frame(svg: &mut String, frame: &GroupFrame) {
    writeln!(
        svg,
        r#"<rect class="group-frame group-frame-{}" x="{}" y="{}" width="{}" height="{}" rx="8" />"#,
        frame.colour,
        num(frame.x),
        num(frame.y),
        num(frame.width),
        num(frame.height)
    )
    .unwrap();
    writeln!(
        svg,
        r#"<text class="group-label group-label-{}" x="{}" y="{}">{}</text>"#,
        frame.colour,
        num(frame.x + FRAME_PAD),
        num(frame.y + FRAME_TITLE - 4.0),
        escape_xml(frame.name)
    )
    .unwrap();
}

/// A small label in a rounded box, `PK` or `UQ`, standing on the row whose
/// baseline is `baseline`.
fn render_badge(svg: &mut String, x: f64, baseline: f64, text: &str, class: &str) {
    writeln!(
        svg,
        r#"<rect class="{}" x="{}" y="{}" width="{}" height="{}" rx="3" />"#,
        class,
        num(x),
        num(baseline - BADGE_HEIGHT + 2.5),
        num(BADGE_WIDTH),
        num(BADGE_HEIGHT)
    )
    .unwrap();
    writeln!(
        svg,
        r#"<text class="{}-text" x="{}" y="{}" text-anchor="middle">{}</text>"#,
        class,
        num(x + BADGE_WIDTH / 2.0),
        num(baseline - 0.5),
        text
    )
    .unwrap();
}

const BADGE_HEIGHT: f64 = 13.0;

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
    offsets.sort_by(|a, b| {
        cost(a)
            .partial_cmp(&cost(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    offsets
}

/// True when the label covers nothing it should not.
fn is_clear(plans: &[LabelPlan], i: usize, layout: &Layout) -> bool {
    let plan = &plans[i];
    let (l, t, r, b) = plan.bounds(LABEL_GAP);

    if plans[..i]
        .iter()
        .any(|other| plan.overlaps(other, LABEL_GAP))
    {
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
            let (sl, sr) = (
                seg[0].0.min(seg[1].0) - stroke,
                seg[0].0.max(seg[1].0) + stroke,
            );
            let (st, sb) = (
                seg[0].1.min(seg[1].1) - stroke,
                seg[0].1.max(seg[1].1) + stroke,
            );
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

    fn node(id: &str, x: f64, y: f64) -> LayoutNode {
        LayoutNode {
            id: id.to_string(),
            x,
            y,
            width: 100.0,
            height: 60.0,
        }
    }

    fn grouped(groups: &[&[&str]], nodes: Vec<LayoutNode>) -> (GraphIR, Layout) {
        let ir = GraphIR {
            nodes: Vec::new(),
            edges: Vec::new(),
            near: Vec::new(),
            groups: groups
                .iter()
                .enumerate()
                .map(|(i, members)| crate::ir::GroupIR {
                    name: format!("g{i}"),
                    members: members.iter().map(|m| m.to_string()).collect(),
                })
                .collect(),
        };
        let layout = Layout {
            nodes,
            edges: Vec::new(),
            width: 800.0,
            height: 600.0,
            channel_gap: 60.0,
            corner_radius: 8.0,
        };
        (ir, layout)
    }

    #[test]
    fn frames_a_group_with_nothing_else_among_it() {
        let (ir, layout) = grouped(
            &[&["A", "B"]],
            vec![
                node("A", 40.0, 80.0),
                node("B", 40.0, 240.0),
                node("C", 400.0, 80.0),
            ],
        );
        let frames = plan_group_frames(&ir, &layout);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].x, 40.0 - FRAME_PAD);
        assert_eq!(frames[0].y, 80.0 - FRAME_PAD - FRAME_TITLE);
        assert_eq!(frames[0].height, 220.0 + FRAME_PAD * 2.0 + FRAME_TITLE);
    }

    #[test]
    fn leaves_out_a_frame_that_would_take_in_an_outsider() {
        let (ir, layout) = grouped(
            &[&["A", "B"]],
            vec![
                node("A", 40.0, 80.0),
                node("C", 200.0, 160.0),
                node("B", 400.0, 240.0),
            ],
        );
        assert!(plan_group_frames(&ir, &layout).is_empty());
    }

    #[test]
    fn leaves_out_frames_that_cross() {
        let (ir, layout) = grouped(
            &[&["A"], &["B"]],
            vec![node("A", 40.0, 80.0), node("B", 150.0, 80.0)],
        );
        assert!(plan_group_frames(&ir, &layout).is_empty());
    }

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

    fn render(source: &str, renderer: SvgRenderer) -> String {
        let schema = Parser::new(source).unwrap().parse().unwrap();
        let ir = GraphIR::from_schema(&schema, None, DetailLevel::All);
        let layout = LayoutEngine::default().layout(&ir);
        renderer.render(&ir, &layout)
    }

    #[test]
    fn the_legend_explains_every_cardinality_and_stays_out_of_the_diagram() {
        let source = "entity User { id int pk }";
        let plain = render(source, SvgRenderer::default());
        let keyed = render(source, SvgRenderer::default().with_legend(true));

        for (_, meaning) in CARDINALITY_MEANINGS {
            assert!(!plain.contains(meaning), "{meaning} drawn without asking");
            assert!(keyed.contains(meaning), "{meaning} missing from the key");
        }

        let height = |svg: &str| {
            let start = svg.find("height=\"").unwrap() + 8;
            let end = svg[start..].find('"').unwrap() + start;
            svg[start..end].parse::<f64>().unwrap()
        };
        assert_eq!(height(&keyed), height(&plain) + LEGEND_HEIGHT);
    }

    #[test]
    fn the_legend_is_drawn_in_the_notation_it_explains() {
        let source = "entity User { id int pk }";
        let feet = render(source, SvgRenderer::default().with_legend(true));
        let text = render(
            source,
            SvgRenderer::default()
                .with_notation(Notation::Text)
                .with_legend(true),
        );

        assert!(
            feet.contains(r#"class="edge-symbol-zero""#),
            "no zero circle"
        );
        assert!(
            !feet.contains(">0..1<"),
            "text cardinality in a crow's foot key"
        );
        assert!(text.contains(">0..1<"), "no text cardinality in a text key");
        assert!(
            !text.contains(r#"class="edge-symbol""#),
            "a foot in a text key"
        );
    }
}
