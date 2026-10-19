//! Rasterising a drawing, for the viewers that cannot show an SVG.
//!
//! The SVG names its font only as `monospace`, which a browser resolves to
//! whatever the machine has. A PNG is made once and looked at anywhere, so it
//! has to be made the same everywhere: the text is set in a font carried with
//! the program, and no font on the machine is consulted.

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};
use std::sync::Arc;

/// DejaVu Sans Mono, whose advance is close enough to the width the layout
/// assumes for a character that the text stays inside its boxes.
const FONT: &[u8] = include_bytes!("../fonts/DejaVuSansMono.ttf");
const FAMILY: &str = "DejaVu Sans Mono";

/// Render SVG markup to a PNG, `scale` pixels per SVG unit.
pub fn svg_to_png(svg: &str, scale: f32) -> Result<Vec<u8>, String> {
    let mut options = Options::default();
    let fontdb = Arc::make_mut(&mut options.fontdb);
    fontdb.load_font_data(FONT.to_vec());
    fontdb.set_monospace_family(FAMILY);

    let tree = Tree::from_str(svg, &options).map_err(|e| format!("Unreadable SVG: {}", e))?;

    let size = tree.size();
    let width = (size.width() * scale).ceil() as u32;
    let height = (size.height() * scale).ceil() as u32;
    let mut pixmap = Pixmap::new(width, height)
        .ok_or_else(|| format!("A {}x{} image is too large to draw", width, height))?;
    resvg::render(
        &tree,
        Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    pixmap
        .encode_png()
        .map_err(|e| format!("Failed to encode PNG: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{DetailLevel, GraphIR};
    use crate::layout::LayoutEngine;
    use crate::parser::Parser;
    use crate::svg::SvgRenderer;

    fn sample_svg() -> String {
        let source = "entity User {\n    id int pk\n    name varchar\n}\n";
        let schema = Parser::new(source).unwrap().parse().unwrap();
        let ir = GraphIR::from_schema(&schema, None, DetailLevel::All);
        let layout = LayoutEngine::default().layout(&ir);
        SvgRenderer::default().render(&ir, &layout)
    }

    fn dimensions(png: &[u8]) -> (u32, u32) {
        let at = |i: usize| u32::from_be_bytes(png[i..i + 4].try_into().unwrap());
        (at(16), at(20))
    }

    #[test]
    fn makes_a_png_the_size_of_the_drawing() {
        let svg = sample_svg();
        let png = svg_to_png(&svg, 1.0).unwrap();

        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let tree = Tree::from_str(&svg, &Options::default()).unwrap();
        let expected = (
            tree.size().width().ceil() as u32,
            tree.size().height().ceil() as u32,
        );
        assert_eq!(dimensions(&png), expected);
    }

    #[test]
    fn scales_the_image() {
        let svg = sample_svg();
        let (w1, h1) = dimensions(&svg_to_png(&svg, 1.0).unwrap());
        let (w2, h2) = dimensions(&svg_to_png(&svg, 2.0).unwrap());
        assert!(w2 >= 2 * w1 - 1 && h2 >= 2 * h1 - 1);
    }

    #[test]
    fn draws_the_text() {
        // A drawing with text has more than the two greys of the boxes
        let png = svg_to_png(&sample_svg(), 1.0).unwrap();
        let pixmap = Pixmap::decode_png(&png).unwrap();
        let mut shades: Vec<u8> = pixmap.pixels().iter().map(|p| p.red()).collect();
        shades.sort_unstable();
        shades.dedup();
        assert!(
            shades.len() > 4,
            "only {} shades: no text was drawn",
            shades.len()
        );
    }

    #[test]
    fn rejects_what_is_not_svg() {
        assert!(svg_to_png("<not svg", 1.0).is_err());
    }
}
