//! SQL dump to ERD conversion module.

mod dialect;
mod lexer;
mod parser;
mod types;

pub use dialect::Dialect;
pub use parser::{SqlParseError, parse_sql};
