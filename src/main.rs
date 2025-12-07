use rusterd::ir::{DetailLevel, GraphIR};
use rusterd::layout::LayoutEngine;
use rusterd::parser::Parser;
use rusterd::serializer;
use rusterd::sql::{parse_sql, Dialect};
use rusterd::svg::SvgRenderer;
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::process;

fn read_input(path: &str) -> Result<String, String> {
    if path == "-" {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("Failed to read stdin: {}", e))?;
        Ok(buf)
    } else {
        fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {}", path, e))
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage(&args[0]);
        process::exit(1);
    }

    match args[1].as_str() {
        "render" => run_render(&args[0], &args[2..]),
        "convert" => run_convert(&args[0], &args[2..]),
        "-h" | "--help" | "help" => {
            print_usage(&args[0]);
        }
        _ => {
            eprintln!("Unknown subcommand: {}", args[1]);
            eprintln!();
            print_usage(&args[0]);
            process::exit(1);
        }
    }
}

fn print_usage(program: &str) {
    eprintln!("Usage: {} <subcommand> [options]", program);
    eprintln!();
    eprintln!("Subcommands:");
    eprintln!("  render   Render ERD file to SVG");
    eprintln!("  convert  Convert SQL dump to ERD notation");
    eprintln!();
    eprintln!("Run '{} <subcommand> --help' for more information.", program);
}

fn run_render(program: &str, args: &[String]) {
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        eprintln!("Usage: {} render <input.erd | -> [options]", program);
        eprintln!();
        eprintln!("Render ERD file to SVG");
        eprintln!("Use '-' to read from stdin.");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  -o, --output <file>   Output file (default: stdout)");
        eprintln!("  -v, --view <name>     Render specific view");
        eprintln!("  -d, --detail <level>  Detail level: tables, pk, pk_fk, all (default: all)");
        if args.is_empty() {
            process::exit(1);
        }
        return;
    }

    let input_path = &args[0];
    let mut output_path: Option<String> = None;
    let mut view: Option<String> = None;
    let mut detail = DetailLevel::All;

    let mut i = 1;
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

    let input = match read_input(input_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e);
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
        None => {
            if let Err(e) = io::stdout().write_all(svg.as_bytes()) {
                if e.kind() != io::ErrorKind::BrokenPipe {
                    eprintln!("Failed to write to stdout: {}", e);
                    process::exit(1);
                }
            }
        }
    }
}

fn run_convert(program: &str, args: &[String]) {
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        eprintln!("Usage: {} convert <input.sql | -> [options]", program);
        eprintln!();
        eprintln!("Convert SQL dump to ERD notation");
        eprintln!("Use '-' to read from stdin.");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  -o, --output <file>      Output file (default: stdout)");
        eprintln!("  -d, --dialect <dialect>  SQL dialect: auto, generic, postgres, mysql (default: auto)");
        if args.is_empty() {
            process::exit(1);
        }
        return;
    }

    let input_path = &args[0];
    let mut output_path: Option<String> = None;
    let mut dialect = Dialect::Auto;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                i += 1;
                if i < args.len() {
                    output_path = Some(args[i].clone());
                }
            }
            "-d" | "--dialect" => {
                i += 1;
                if i < args.len() {
                    dialect = Dialect::from_str(&args[i]).unwrap_or_else(|| {
                        eprintln!("Invalid dialect: {}", args[i]);
                        eprintln!("Valid options: auto, generic, postgres, mysql");
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

    let input = match read_input(input_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    };

    let schema = match parse_sql(&input, dialect) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("SQL parse error: {}", e);
            process::exit(1);
        }
    };

    let erd = serializer::serialize(&schema);

    match output_path {
        Some(path) => {
            if let Err(e) = fs::write(&path, &erd) {
                eprintln!("Failed to write {}: {}", path, e);
                process::exit(1);
            }
        }
        None => {
            if let Err(e) = io::stdout().write_all(erd.as_bytes()) {
                if e.kind() != io::ErrorKind::BrokenPipe {
                    eprintln!("Failed to write to stdout: {}", e);
                    process::exit(1);
                }
            }
        }
    }
}
