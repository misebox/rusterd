pub mod ast;
pub mod ir;
pub mod layout;
pub mod lexer;
pub mod measure;
pub mod parser;
pub mod serializer;
pub mod sql;
pub mod svg;

use wasm_bindgen::prelude::*;

use ir::{DetailLevel, GraphIR};
use layout::LayoutEngine;
use parser::Parser;
use svg::SvgRenderer;

#[wasm_bindgen(start)]
fn init() {
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

/// Render ERD source to SVG data URI (for use with <img src={...}>)
#[wasm_bindgen(js_name = "erdToDataUri")]
pub fn render_erd_data_uri(
    source: &str,
    view: Option<String>,
    detail: Option<String>,
) -> Result<String, String> {
    let svg = render_erd(source, view, detail)?;
    Ok(format!(
        "data:image/svg+xml,{}",
        js_sys::encode_uri_component(&svg)
    ))
}

/// Convert SQL dump to ERD notation
#[wasm_bindgen(js_name = "sqlToErd")]
pub fn sql_to_erd(sql_source: &str, dialect: Option<String>) -> Result<String, String> {
    let dialect = dialect
        .as_deref()
        .and_then(sql::Dialect::from_str)
        .unwrap_or(sql::Dialect::Auto);

    let schema = sql::parse_sql(sql_source, dialect).map_err(|e| e.to_string())?;
    Ok(serializer::serialize(&schema))
}

/// Convert SQL dump directly to SVG
#[wasm_bindgen(js_name = "sqlToSvg")]
pub fn sql_to_svg(
    sql_source: &str,
    dialect: Option<String>,
    view: Option<String>,
    detail: Option<String>,
) -> Result<String, String> {
    let erd = sql_to_erd(sql_source, dialect)?;
    render_erd(&erd, view, detail)
}
