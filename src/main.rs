use rusterd::ir::{DetailLevel, GraphIR};
use rusterd::layout::LayoutEngine;
use rusterd::parser::Parser;
use rusterd::serializer;
use rusterd::sql::{parse_sql, Dialect};
use rusterd::svg::{Notation, SvgRenderer};
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
        eprintln!("  -f, --focus <name>    Draw only what a focus block lists");
        eprintln!("  -d, --detail <level>  Detail level: tables, pk, pk_fk, all (default: all)");
        eprintln!("  -n, --notation <n>    Cardinality notation: crowsfoot, text (default: crowsfoot)");
        eprintln!("  -l, --legend          Draw a key to the cardinality symbols");
        eprintln!("  -D, --dense           Close up the spacing, to fit more on a screen");
        if args.is_empty() {
            process::exit(1);
        }
        return;
    }

    let input_path = &args[0];
    let mut output_path: Option<String> = None;
    let mut focus: Option<String> = None;
    let mut detail = DetailLevel::All;
    let mut notation = Notation::default();
    let mut legend = false;
    let mut dense = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                i += 1;
                if i < args.len() {
                    output_path = Some(args[i].clone());
                }
            }
            "-f" | "--focus" => {
                i += 1;
                if i < args.len() {
                    focus = Some(args[i].clone());
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
            "-n" | "--notation" => {
                i += 1;
                if i < args.len() {
                    notation = Notation::from_str(&args[i]).unwrap_or_else(|| {
                        eprintln!("Invalid notation: {}", args[i]);
                        eprintln!("Valid options: crowsfoot, text");
                        process::exit(1);
                    });
                }
            }
            "-l" | "--legend" => legend = true,
            "-D" | "--dense" => dense = true,
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
            eprintln!("{}", e);
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

    if let Some(name) = focus.as_deref() {
        if schema.find_focus(name).is_none() {
            eprintln!("Unknown focus: {}", name);
            let names = schema.focus_names();
            if names.is_empty() {
                eprintln!("This file defines no focus blocks.");
            } else {
                eprintln!("Available: {}", names.join(", "));
            }
            process::exit(1);
        }
    }

    let ir = GraphIR::from_schema(&schema, focus.as_deref(), detail);
    let layout = LayoutEngine::default().with_dense_spacing(dense).layout(&ir);
    let svg = SvgRenderer::default().with_notation(notation).with_legend(legend).render(&ir, &layout);

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
