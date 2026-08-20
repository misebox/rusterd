//! Checks the GBNF grammars in `docs/` against the parsers they describe.
//!
//! A grammar is only useful if everything it can produce is accepted, so the
//! test reads the grammar, generates documents from it, and runs them through
//! the real pipeline. Failures print the offending sample and its seed.

use rusterd::ir::{DetailLevel, GraphIR};
use rusterd::layout::LayoutEngine;
use rusterd::parser::Parser;
use rusterd::serializer;
use rusterd::sql::{parse_sql, Dialect};
use rusterd::svg::SvgRenderer;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

mod gbnf;
use gbnf::Grammar;

/// Documents generated per grammar. Enough to reach every alternative several
/// times while keeping the test under a second.
const SAMPLES: u64 = 300;

fn grammar(name: &str) -> Grammar {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs").join(name);
    let source = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
    Grammar::parse(&source).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn erd_grammar_produces_documents_the_compiler_accepts() {
    let grammar = grammar("erd.gbnf");

    for seed in 0..SAMPLES {
        let document = grammar.generate(seed);
        let schema = Parser::new(&document)
            .and_then(|mut p| p.parse())
            .unwrap_or_else(|e| panic!("seed {seed}: {e}\n---\n{document}\n---"));

        // The rest of the pipeline has to survive it too.
        let ir = GraphIR::from_schema(&schema, None, DetailLevel::All);
        let layout = LayoutEngine::default().layout(&ir);
        let svg = SvgRenderer::default().render(&ir, &layout);
        assert!(
            svg.starts_with("<svg"),
            "seed {seed} rendered no diagram\n---\n{document}\n---"
        );
    }
}

#[test]
fn sql_grammar_produces_statements_the_converter_accepts() {
    let grammar = grammar("sql.gbnf");
    let mut tables = 0;

    for seed in 0..SAMPLES {
        let statements = grammar.generate(seed);
        let schema = parse_sql(&statements, Dialect::Auto)
            .unwrap_or_else(|e| panic!("seed {seed}: {e}\n---\n{statements}\n---"));
        tables += schema.entities.len();

        // Whatever it found has to survive the trip back through the ERD DSL.
        let erd = serializer::serialize(&schema);
        Parser::new(&erd)
            .and_then(|mut p| p.parse())
            .unwrap_or_else(|e| panic!("seed {seed}: {e} in generated ERD\n---\n{erd}\n---"));
    }

    assert!(
        tables >= SAMPLES as usize / 2,
        "the grammar produced {tables} tables over {SAMPLES} samples, so most \
         samples carried no schema at all"
    );
}

/// The documents in the specification are what a reader will copy, so they had
/// better compile.
#[test]
fn spec_examples_compile() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/DSL-spec.md");
    let spec = fs::read_to_string(path).expect("docs/DSL-spec.md");

    let blocks: Vec<&str> = spec
        .split("```erd\n")
        .skip(1)
        .filter_map(|rest| rest.split("```").next())
        .collect();
    assert!(blocks.len() >= 5, "expected the spec to keep its examples");

    for (index, block) in blocks.iter().enumerate() {
        // Fragments that are not whole files are marked by their first line.
        if !block.contains("entity ") && !block.contains("rel ") && !block.contains("view ") {
            continue;
        }
        Parser::new(block)
            .and_then(|mut p| p.parse())
            .unwrap_or_else(|e| panic!("example {index}: {e}\n---\n{block}\n---"));
    }
}

/// Every rule a grammar mentions must exist, or generation would silently skip
/// part of the language.
#[test]
fn grammars_are_complete() {
    for name in ["erd.gbnf", "sql.gbnf"] {
        let grammar = grammar(name);
        let missing: Vec<&String> = grammar.undefined_rules();
        assert!(missing.is_empty(), "{name} refers to undefined {missing:?}");

        let unused: Vec<&String> = grammar.unreachable_rules();
        assert!(unused.is_empty(), "{name} never reaches {unused:?}");
    }
}

/// Generation must not depend on anything but the seed.
#[test]
fn generation_is_deterministic() {
    let grammar = grammar("erd.gbnf");
    let counts: HashMap<u64, usize> = (0..20)
        .map(|seed| (seed, grammar.generate(seed).len()))
        .collect();

    for (seed, length) in counts {
        assert_eq!(grammar.generate(seed).len(), length, "seed {seed} drifted");
    }
}
