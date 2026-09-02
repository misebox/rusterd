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
use layout::{LayoutEngine, aspect_from_name};
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
/** How much of an entity to draw. */
export type Detail = "tables" | "pk" | "pk_fk" | "all";

/** How to draw the cardinalities. */
export type Notation = "crowsfoot" | "text";

/** Which SQL a dump is written in. */
export type Dialect = "auto" | "generic" | "postgres" | "mysql";

/** How to draw the diagram. Every field may be left out. */
export interface DrawOptions {
    /** Name of a `focus` block: draw only the entities it lists. */
    focus?: string | null;
    /** Which columns to draw. Default "all". */
    detail?: Detail | null;
    /** How to draw cardinalities. Default "crowsfoot". */
    notation?: Notation | null;
    /** Draw a key to the cardinality symbols below the diagram. */
    legend?: boolean | null;
    /** Close up the spacing, to fit a large schema on one screen. */
    dense?: boolean | null;
    /** Shape to aim for, as `width:height`. Default "1:1". */
    aspect?: string | null;
}

/** How to read the SQL, and then how to draw it. */
export interface ConvertOptions extends DrawOptions {
    /** Default "auto", which reads the dump and decides. */
    dialect?: Dialect | null;
}
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "DrawOptions")]
    pub type DrawOptions;

    #[wasm_bindgen(typescript_type = "ConvertOptions")]
    pub type ConvertOptions;

    /// The one option that is also asked for on its own, by `sqlToErd`.
    #[wasm_bindgen(typescript_type = "Dialect")]
    pub type DialectName;
}

/// Everything the caller asked for, read out of the object they passed.
///
/// An object is read field by field rather than taken apart wholesale, so a
/// field spelled wrong is ignored instead of refused — and, more to the point,
/// so that a caller writes what they mean rather than counting commas.
///
/// A field whose *value* is not one this compiler knows is refused, though.
/// TypeScript stops that spelling ever being written; JavaScript has to be
/// told, and quietly drawing something else is the worst of the answers.
struct Asked {
    focus: Option<String>,
    detail: DetailLevel,
    notation: Notation,
    legend: bool,
    dense: bool,
    aspect: f64,
    dialect: sql::Dialect,
}

impl Asked {
    fn from(options: Option<impl AsRef<JsValue>>) -> Result<Self, String> {
        let object = options.map(|given| given.as_ref().clone());
        let field = |name: &str| -> Option<JsValue> {
            let object = object.as_ref()?;
            js_sys::Reflect::get(object, &JsValue::from_str(name)).ok()
        };
        let text = |name: &str| -> Option<String> { field(name)?.as_string() };
        let flag = |name: &str| -> bool { field(name).and_then(|v| v.as_bool()).unwrap_or(false) };

        Ok(Self {
            focus: text("focus"),
            detail: match text("detail") {
                Some(name) => named(&name, DetailLevel::from_name, "detail", DETAIL)?,
                None => DetailLevel::All,
            },
            notation: match text("notation") {
                Some(name) => named(&name, Notation::from_name, "notation", NOTATION)?,
                None => Notation::default(),
            },
            legend: flag("legend"),
            dense: flag("dense"),
            aspect: match text("aspect") {
                Some(name) => named(&name, aspect_from_name, "aspect", ASPECT)?,
                None => 1.0,
            },
            dialect: read_dialect(text("dialect"))?,
        })
    }
}

/// What each option will answer to, for the message when it is given something
/// else. The same words the types offer and the documentation lists.
const DETAIL: &str = r#""tables", "pk", "pk_fk", "all""#;
const NOTATION: &str = r#""crowsfoot", "text""#;
const DIALECT: &str = r#""auto", "generic", "postgres", "mysql""#;
const ASPECT: &str = r#"a shape written as width:height, such as "1:1" or "16:9""#;

/// One option's value, or a complaint naming what it would have taken.
fn named<T>(
    value: &str,
    read: impl Fn(&str) -> Option<T>,
    option: &str,
    allowed: &str,
) -> Result<T, String> {
    read(value).ok_or_else(|| format!("Unknown {option}: {value:?} (expected {allowed})"))
}

/// The dialect, which is asked for on its own as well as inside an object.
fn read_dialect(given: Option<String>) -> Result<sql::Dialect, String> {
    match given {
        Some(name) => named(&name, sql::Dialect::from_name, "dialect", DIALECT),
        None => Ok(sql::Dialect::Auto),
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
    draw(source, &Asked::from(options)?)
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
        .with_aspect(asked.aspect)
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
pub fn sql_to_erd(sql_source: &str, dialect: Option<DialectName>) -> Result<String, String> {
    let named = dialect.and_then(|given| AsRef::<JsValue>::as_ref(&given).as_string());
    let schema = sql::parse_sql(sql_source, read_dialect(named)?).map_err(|e| e.to_string())?;
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
    let asked = Asked::from(options)?;
    let erd = sql::parse_sql(sql_source, asked.dialect)
        .map(|schema| serializer::serialize(&schema))
        .map_err(|e| e.to_string())?;
    draw(&erd, &asked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_the_compiler_knows_is_read() {
        let read = named("pk_fk", DetailLevel::from_name, "detail", DETAIL);
        assert_eq!(read, Ok(DetailLevel::PkFk));
    }

    /// Drawing every column when "pk_fk" was misspelt would look like the
    /// option had been ignored, which is the hardest kind of bug to see.
    #[test]
    fn a_value_it_does_not_know_is_refused_by_name() {
        let read = named("pkfk", DetailLevel::from_name, "detail", DETAIL);
        assert_eq!(
            read,
            Err(r#"Unknown detail: "pkfk" (expected "tables", "pk", "pk_fk", "all")"#.to_string())
        );
    }
}
