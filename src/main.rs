use rusterd::ir::{DetailLevel, GraphIR};
use rusterd::layout::{LayoutEngine, aspect_from_name};
use rusterd::parser::Parser;
use rusterd::serializer;
use rusterd::sql::{Dialect, parse_sql};
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

/// Help that was asked for goes to stdout and succeeds, so that piping it into
/// a pager shows something. Help that follows a mistake goes to stderr and
/// fails, so that it does not end up in whatever the output was meant for.
fn asked_for(text: &str) -> ! {
    println!("{}", text);
    process::exit(0);
}

fn after_a_mistake(text: &str) -> ! {
    eprintln!("{}", text);
    process::exit(1);
}

/// What the reader typed, not where the binary happens to live. An installed
/// `rusterd` is invoked by name, and `Usage: /Users/…/.cargo/bin/rusterd` is a
/// line nobody can copy.
fn called(argv0: &str) -> &str {
    argv0.rsplit('/').next().unwrap_or(argv0)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = called(&args[0]);

    if args.len() < 2 {
        after_a_mistake(&usage(program));
    }

    match args[1].as_str() {
        "render" => run_render(program, &args[2..]),
        "convert" => run_convert(program, &args[2..]),
        "-V" | "--version" | "version" => {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        }
        "-h" | "--help" | "help" => asked_for(&usage(program)),
        _ => {
            eprintln!("Unknown subcommand: {}", args[1]);
            eprintln!();
            after_a_mistake(&usage(program));
        }
    }
}

fn usage(program: &str) -> String {
    format!(
        "{name} {version}
Usage: {program} <subcommand> [options]

Subcommands:
  render   Render ERD file to SVG
  convert  Convert SQL dump to ERD notation

Options:
  -h, --help     Print this, or a subcommand's own
  -V, --version  Print the version

Run '{program} <subcommand> --help' for more information.",
        name = env!("CARGO_PKG_NAME"),
        version = env!("CARGO_PKG_VERSION"),
    )
}

fn run_render(program: &str, args: &[String]) {
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        let help = format!(
            "Usage: {program} render <input.erd | -> [options]

Render ERD file to SVG
Use '-' to read from stdin.

Options:
  -o, --output <file>   Output file (default: stdout)
  -f, --focus <name>    Draw only what a focus block lists
  -d, --detail <level>  Detail level: tables, pk, pk_fk, all (default: all)
  -n, --notation <n>    Cardinality notation: crowsfoot, text (default: crowsfoot)
  -l, --legend          Draw a key to the cardinality symbols
  -D, --dense           Close up the spacing, to fit more on a screen
  -a, --aspect <w:h>    Shape to aim for: 1:1, 16:9, 210:297 (default: 1:1)"
        );
        if args.is_empty() {
            after_a_mistake(&help);
        }
        asked_for(&help);
    }

    let input_path = &args[0];
    let mut output_path: Option<String> = None;
    let mut focus: Option<String> = None;
    let mut detail = DetailLevel::All;
    let mut notation = Notation::default();
    let mut legend = false;
    let mut dense = false;
    let mut aspect = 1.0;

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
                    detail = DetailLevel::from_name(&args[i]).unwrap_or_else(|| {
                        eprintln!("Invalid detail level: {}", args[i]);
                        process::exit(1);
                    });
                }
            }
            "-n" | "--notation" => {
                i += 1;
                if i < args.len() {
                    notation = Notation::from_name(&args[i]).unwrap_or_else(|| {
                        eprintln!("Invalid notation: {}", args[i]);
                        eprintln!("Valid options: crowsfoot, text");
                        process::exit(1);
                    });
                }
            }
            "-a" | "--aspect" => {
                i += 1;
                if i < args.len() {
                    aspect = aspect_from_name(&args[i]).unwrap_or_else(|| {
                        eprintln!("Invalid aspect: {}", args[i]);
                        eprintln!("Give it as width:height, such as 1:1 or 16:9.");
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

    // Both halves of reading a file fail the same way, and say so the same
    // way: the file, then where in it, then what was wrong.
    let mut parser = match Parser::new(&input) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}: {}", input_path, e);
            process::exit(1);
        }
    };

    let schema = match parser.parse() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}: {}", input_path, e);
            process::exit(1);
        }
    };

    if let Some(name) = focus.as_deref()
        && schema.find_focus(name).is_none()
    {
        eprintln!("Unknown focus: {}", name);
        let names = schema.focus_names();
        if names.is_empty() {
            eprintln!("This file defines no focus blocks.");
        } else {
            eprintln!("Available: {}", names.join(", "));
        }
        process::exit(1);
    }

    let ir = GraphIR::from_schema(&schema, focus.as_deref(), detail);
    let layout = LayoutEngine::default()
        .with_dense_spacing(dense)
        .with_aspect(aspect)
        .layout(&ir);
    let svg = SvgRenderer::default()
        .with_notation(notation)
        .with_legend(legend)
        .render(&ir, &layout);

    match output_path {
        Some(path) => {
            if let Err(e) = fs::write(&path, &svg) {
                eprintln!("Failed to write {}: {}", path, e);
                process::exit(1);
            }
        }
        None => {
            if let Err(e) = io::stdout().write_all(svg.as_bytes())
                && e.kind() != io::ErrorKind::BrokenPipe
            {
                eprintln!("Failed to write to stdout: {}", e);
                process::exit(1);
            }
        }
    }
}

fn run_convert(program: &str, args: &[String]) {
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        let help = format!(
            "Usage: {program} convert <input.sql | -> [options]

Convert SQL dump to ERD notation
Use '-' to read from stdin.

Options:
  -o, --output <file>      Output file (default: stdout)
  -d, --dialect <dialect>  SQL dialect: auto, generic, postgres, mysql (default: auto)"
        );
        if args.is_empty() {
            after_a_mistake(&help);
        }
        asked_for(&help);
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
                    dialect = Dialect::from_name(&args[i]).unwrap_or_else(|| {
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
            if let Err(e) = io::stdout().write_all(erd.as_bytes())
                && e.kind() != io::ErrorKind::BrokenPipe
            {
                eprintln!("Failed to write to stdout: {}", e);
                process::exit(1);
            }
        }
    }
}
