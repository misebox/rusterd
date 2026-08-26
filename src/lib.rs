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

/// What the TypeScript definitions say a caller may pass.
#[wasm_bindgen(typescript_custom_section)]
const OPTIONS: &'static str = r#"
/** How to draw the diagram. Every field may be left out. */
export interface DrawOptions {
    /** Name of a `focus` block: draw only the entities it lists. */
    focus?: string | null;
    /** Which columns to draw: "tables", "pk", "pk_fk" or "all". Default "all". */
    detail?: string | null;
    /** How to draw cardinalities: "crowsfoot" or "text". Default "crowsfoot". */
    notation?: string | null;
    /** Draw a key to the cardinality symbols below the diagram. */
    legend?: boolean | null;
    /** Close up the spacing, to fit a large schema on one screen. */
    dense?: boolean | null;
}

/** How to read the SQL, and then how to draw it. */
export interface ConvertOptions extends DrawOptions {
    /** "auto", "generic", "postgres" or "mysql". Default "auto". */
    dialect?: string | null;
}
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "DrawOptions")]
    pub type DrawOptions;

    #[wasm_bindgen(typescript_type = "ConvertOptions")]
    pub type ConvertOptions;
}

/// Everything the caller asked for, read out of the object they passed.
///
/// An object is read field by field rather than taken apart wholesale, so a
/// field spelled wrong is ignored instead of refused — and, more to the point,
/// so that a caller writes what they mean rather than counting commas.
struct Asked {
    focus: Option<String>,
    detail: DetailLevel,
    notation: Notation,
    legend: bool,
    dense: bool,
    dialect: sql::Dialect,
}

impl Asked {
    fn from(options: Option<impl AsRef<JsValue>>) -> Self {
        let object = options.map(|given| given.as_ref().clone());
        let field = |name: &str| -> Option<JsValue> {
            let object = object.as_ref()?;
            js_sys::Reflect::get(object, &JsValue::from_str(name)).ok()
        };
        let text = |name: &str| -> Option<String> { field(name)?.as_string() };
        let flag = |name: &str| -> bool { field(name).and_then(|v| v.as_bool()).unwrap_or(false) };

        Self {
            focus: text("focus"),
            detail: text("detail")
                .as_deref()
                .and_then(DetailLevel::from_name)
                .unwrap_or(DetailLevel::All),
            notation: text("notation")
                .as_deref()
                .and_then(Notation::from_name)
                .unwrap_or_default(),
            legend: flag("legend"),
            dense: flag("dense"),
            dialect: text("dialect")
                .as_deref()
                .and_then(sql::Dialect::from_name)
                .unwrap_or(sql::Dialect::Auto),
        }
    }
}

/// Compile ERD source into an SVG document.
///
/// Returns the markup, `<svg ...>...</svg>`, as a string.
///
/// Throws the compiler's complaint as a **string**, not an `Error`: the source
/// would not parse, or `options.focus` names a block the source does not have.
///
/// ```js
/// erdToSvg(source);
/// erdToSvg(source, { focus: "checkout", detail: "pk_fk", dense: true });
/// ```
#[wasm_bindgen(js_name = "erdToSvg")]
pub fn render_erd(source: &str, options: Option<DrawOptions>) -> Result<String, String> {
    draw(source, &Asked::from(options))
}

/// Compile ERD source into an SVG data URI.
///
/// Returns `data:image/svg+xml,...` as a string, ready to be the `src` of an
/// `<img>`. Same options, same throw, as `erdToSvg`.
#[wasm_bindgen(js_name = "erdToDataUri")]
pub fn render_erd_data_uri(source: &str, options: Option<DrawOptions>) -> Result<String, String> {
    let svg = render_erd(source, options)?;
    Ok(format!(
        "data:image/svg+xml,{}",
        js_sys::encode_uri_component(&svg)
    ))
}

/// Draw what the caller asked for.
fn draw(source: &str, asked: &Asked) -> Result<String, String> {
    let mut parser = Parser::new(source).map_err(|e| e.to_string())?;
    let schema = parser.parse().map_err(|e| e.to_string())?;

    if let Some(name) = asked.focus.as_deref()
        && schema.find_focus(name).is_none()
    {
        return Err(format!(
            "Unknown focus: {} (available: {})",
            name,
            schema.focus_names().join(", ")
        ));
    }

    let ir = GraphIR::from_schema(&schema, asked.focus.as_deref(), asked.detail);
    let layout = LayoutEngine::default()
        .with_dense_spacing(asked.dense)
        .layout(&ir);
    Ok(SvgRenderer::default()
        .with_notation(asked.notation)
        .with_legend(asked.legend)
        .render(&ir, &layout))
}

/// Read a SQL dump and write it out as ERD source.
///
/// Returns what a `.erd` file would hold, as a string.
///
/// Statements it does not recognise are skipped, so a dump it cannot read at
/// all comes back as an empty string rather than as a throw.
///
/// ```js
/// sqlToErd(dump);
/// sqlToErd(dump, "postgres");
/// ```
#[wasm_bindgen(js_name = "sqlToErd")]
pub fn sql_to_erd(sql_source: &str, dialect: Option<String>) -> Result<String, String> {
    let dialect = dialect
        .as_deref()
        .and_then(sql::Dialect::from_name)
        .unwrap_or(sql::Dialect::Auto);

    let schema = sql::parse_sql(sql_source, dialect).map_err(|e| e.to_string())?;
    Ok(serializer::serialize(&schema))
}

/// Read a SQL dump and compile it straight to an SVG document.
///
/// `sqlToErd` followed by `erdToSvg`: returns the markup as a string, and
/// throws a string if either half of that cannot be done. Takes the options of
/// both.
///
/// ```js
/// sqlToSvg(dump, { dialect: "postgres", detail: "pk_fk" });
/// ```
#[wasm_bindgen(js_name = "sqlToSvg")]
pub fn sql_to_svg(sql_source: &str, options: Option<ConvertOptions>) -> Result<String, String> {
    let asked = Asked::from(options);
    let erd = sql::parse_sql(sql_source, asked.dialect)
        .map(|schema| serializer::serialize(&schema))
        .map_err(|e| e.to_string())?;
    draw(&erd, &asked)
}
