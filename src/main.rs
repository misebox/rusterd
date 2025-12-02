use rusterd::ir::{DetailLevel, GraphIR};
use rusterd::layout::LayoutEngine;
use rusterd::parser::Parser;
use rusterd::svg::SvgRenderer;
use std::env;
use std::fs;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <input.erd> [options]", args[0]);
        eprintln!();
        eprintln!("Options:");
        eprintln!("  -o, --output <file>   Output file (default: stdout)");
        eprintln!("  -v, --view <name>     Render specific view");
        eprintln!("  -d, --detail <level>  Detail level: tables, pk, pk_fk, all (default: all)");
        process::exit(1);
    }

    let input_path = &args[1];
    let mut output_path: Option<String> = None;
    let mut view: Option<String> = None;
    let mut detail = DetailLevel::All;

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                i += 1;
                if i < args.len() {
                    output_path = Some(args[i].clone());
                }
            }
            "-v" | "--view" => {
                i += 1;
                if i < args.len() {
                    view = Some(args[i].clone());
                }
            }
            "-d" | "--detail" => {
                i += 1;
                if i < args.len() {
                    detail = DetailLevel::from_str(&args[i]).unwrap_or_else(|| {
                        eprintln!("Invalid detail level: {}", args[i]);
                        process::exit(1);
                    });
                }
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                process::exit(1);
            }
        }
        i += 1;
    }

    let input = match fs::read_to_string(input_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to read {}: {}", input_path, e);
            process::exit(1);
        }
    };

    let mut parser = match Parser::new(&input) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Lex error: {}", e);
            process::exit(1);
        }
    };

    let schema = match parser.parse() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            process::exit(1);
        }
    };

    let ir = GraphIR::from_schema(&schema, view.as_deref(), detail);
    let layout = LayoutEngine::default().layout(&ir);
    let svg = SvgRenderer::default().render(&ir, &layout);

    match output_path {
        Some(path) => {
            if let Err(e) = fs::write(&path, &svg) {
                eprintln!("Failed to write {}: {}", path, e);
                process::exit(1);
            }
        }
        None => print!("{}", svg),
    }
}
