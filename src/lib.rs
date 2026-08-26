pub mod ast;
pub mod ir;
pub mod layout;
pub mod lexer;
pub mod measure;
pub mod ordering;
pub mod parser;
pub mod serializer;
pub mod sql;
pub mod svg;

use wasm_bindgen::prelude::*;

use ir::{DetailLevel, GraphIR};
use layout::LayoutEngine;
use parser::Parser;
use svg::{Notation, SvgRenderer};

/// Runs when the module is instantiated, so that a panic in the compiler shows
/// up in the console rather than as `unreachable executed`.
#[wasm_bindgen(start)]
fn report_panics() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();
}

/// Compile ERD source into an SVG document.
///
/// Returns the markup, `<svg ...>...</svg>`, as a string.
///
/// Throws the compiler's complaint as a **string**, not an `Error`: the source
/// would not parse, or `view` names one the source does not define.
///
/// Every argument after the source may be `null` or left out. `view` is the
/// name of a `view` block, `detail` one of `tables`, `pk`, `pk_fk`, `all`, and
/// `notation` one of `crowsfoot`, `text`.
#[wasm_bindgen(js_name = "erdToSvg")]
pub fn render_erd(
    source: &str,
    view: Option<String>,
    detail: Option<String>,
    notation: Option<String>,
    legend: Option<bool>,
    dense: Option<bool>,
) -> Result<String, String> {
    let mut parser = Parser::new(source).map_err(|e| e.to_string())?;
    let schema = parser.parse().map_err(|e| e.to_string())?;

    if let Some(name) = view.as_deref() {
        if schema.find_view(name).is_none() {
            return Err(format!(
                "Unknown view: {} (available: {})",
                name,
                schema.view_names().join(", ")
            ));
        }
    }

    let detail_level = detail
        .as_deref()
        .and_then(DetailLevel::from_str)
        .unwrap_or(DetailLevel::All);

    let notation = notation
        .as_deref()
        .and_then(Notation::from_str)
        .unwrap_or_default();
    let legend = legend.unwrap_or(false);
    let dense = dense.unwrap_or(false);

    let ir = GraphIR::from_schema(&schema, view.as_deref(), detail_level);
    let layout = LayoutEngine::default().with_dense_spacing(dense).layout(&ir);
    let svg = SvgRenderer::default().with_notation(notation).with_legend(legend).render(&ir, &layout);

    Ok(svg)
}

/// Compile ERD source into an SVG data URI.
///
/// Returns `data:image/svg+xml,...` as a string, ready to be the `src` of an
/// `<img>`. Same arguments, same throw, as `erdToSvg`.
#[wasm_bindgen(js_name = "erdToDataUri")]
pub fn render_erd_data_uri(
    source: &str,
    view: Option<String>,
    detail: Option<String>,
    notation: Option<String>,
    legend: Option<bool>,
    dense: Option<bool>,
) -> Result<String, String> {
    let svg = render_erd(source, view, detail, notation, legend, dense)?;
    Ok(format!(
        "data:image/svg+xml,{}",
        js_sys::encode_uri_component(&svg)
    ))
}

/// Read a SQL dump and write it out as ERD source.
///
/// Returns what a `.erd` file would hold, as a string. `dialect` is one of
/// `auto`, `generic`, `postgres`, `mysql`, and may be `null`.
///
/// Statements it does not recognise are skipped, so a dump it cannot read at
/// all comes back as an empty string rather than as a throw.
#[wasm_bindgen(js_name = "sqlToErd")]
pub fn sql_to_erd(sql_source: &str, dialect: Option<String>) -> Result<String, String> {
    let dialect = dialect
        .as_deref()
        .and_then(sql::Dialect::from_str)
        .unwrap_or(sql::Dialect::Auto);

    let schema = sql::parse_sql(sql_source, dialect).map_err(|e| e.to_string())?;
    Ok(serializer::serialize(&schema))
}

/// Read a SQL dump and compile it straight to an SVG document.
///
/// `sqlToErd` followed by `erdToSvg`: returns the markup as a string, and
/// throws a string if either half of that cannot be done.
#[wasm_bindgen(js_name = "sqlToSvg")]
pub fn sql_to_svg(
    sql_source: &str,
    dialect: Option<String>,
    view: Option<String>,
    detail: Option<String>,
    notation: Option<String>,
    legend: Option<bool>,
    dense: Option<bool>,
) -> Result<String, String> {
    let erd = sql_to_erd(sql_source, dialect)?;
    render_erd(&erd, view, detail, notation, legend, dense)
}
