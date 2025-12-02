pub mod ast;
pub mod ir;
pub mod layout;
pub mod lexer;
pub mod measure;
pub mod parser;
pub mod svg;

use wasm_bindgen::prelude::*;

use ir::{DetailLevel, GraphIR};
use layout::LayoutEngine;
use parser::Parser;
use svg::SvgRenderer;

/// Initialize panic hook for better error messages in WASM
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();
}

/// Render ERD source to SVG
#[wasm_bindgen(js_name = "erdToSvg")]
pub fn render_erd(
    source: &str,
    view: Option<String>,
    detail: Option<String>,
) -> Result<String, String> {
    let mut parser = Parser::new(source).map_err(|e| e.to_string())?;
    let schema = parser.parse().map_err(|e| e.to_string())?;

    let detail_level = detail
        .as_deref()
        .and_then(DetailLevel::from_str)
        .unwrap_or(DetailLevel::All);

    let ir = GraphIR::from_schema(&schema, view.as_deref(), detail_level);
    let layout = LayoutEngine::default().layout(&ir);
    let svg = SvgRenderer::default().render(&ir, &layout);

    Ok(svg)
}
