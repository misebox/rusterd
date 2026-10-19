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

/// An option's name, and its value when it was written `--name=value`. A value
/// written as the next argument is picked up by `value`.
fn split_option(arg: &str) -> (&str, Option<&str>) {
    match arg.split_once('=') {
        Some((name, value)) => (name, Some(value)),
        None => (arg, None),
    }
}

/// The value of the option at `args[*i]`: the one written after `=`, or else
/// the next argument, which is then stepped over.
fn value<'a>(args: &'a [String], i: &mut usize, inline: Option<&'a str>) -> &'a str {
    if let Some(value) = inline {
        return value;
    }
    *i += 1;
    match args.get(*i) {
        Some(value) => value,
        None => {
            eprintln!("{} needs a value", args[*i - 1]);
            process::exit(1);
        }
    }
}

/// Whether an argument is the input rather than an option. `-` is stdin.
fn is_input(arg: &str) -> bool {
    arg == "-" || !arg.starts_with('-')
}

/// Remember the input, which there can only be one of.
fn take_input(input: &mut Option<String>, arg: &str) {
    if input.replace(arg.to_string()).is_some() {
        eprintln!("Unexpected argument: {}", arg);
        process::exit(1);
    }
}

fn input_or_help(input: Option<String>, help: &str) -> String {
    match input {
        Some(path) => path,
        None => {
            eprintln!("No input given.");
            eprintln!();
            after_a_mistake(help);
        }
    }
}

fn run_render(program: &str, args: &[String]) {
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

    let mut input_path: Option<String> = None;
    let mut output_path: Option<String> = None;
    let mut focus: Option<String> = None;
    let mut detail = DetailLevel::All;
    let mut notation = Notation::default();
    let mut legend = false;
    let mut dense = false;
    let mut aspect = 1.0;

    let mut i = 0;
    while i < args.len() {
        let (name, inline) = split_option(&args[i]);
        match name {
            _ if is_input(name) => take_input(&mut input_path, &args[i]),
            "-h" | "--help" => asked_for(&help),
            "-o" | "--output" => output_path = Some(value(args, &mut i, inline).to_string()),
            "-f" | "--focus" => focus = Some(value(args, &mut i, inline).to_string()),
            "-d" | "--detail" => {
                let given = value(args, &mut i, inline);
                detail = DetailLevel::from_name(given).unwrap_or_else(|| {
                    eprintln!("Invalid detail level: {}", given);
                    eprintln!("Valid options: tables, pk, pk_fk, all");
                    process::exit(1);
                });
            }
            "-n" | "--notation" => {
                let given = value(args, &mut i, inline);
                notation = Notation::from_name(given).unwrap_or_else(|| {
                    eprintln!("Invalid notation: {}", given);
                    eprintln!("Valid options: crowsfoot, text");
                    process::exit(1);
                });
            }
            "-a" | "--aspect" => {
                let given = value(args, &mut i, inline);
                aspect = aspect_from_name(given).unwrap_or_else(|| {
                    eprintln!("Invalid aspect: {}", given);
                    eprintln!("Give it as width:height, such as 1:1 or 16:9.");
                    process::exit(1);
                });
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
    let input_path = &input_or_help(input_path, &help);

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

    let mut input_path: Option<String> = None;
    let mut output_path: Option<String> = None;
    let mut dialect = Dialect::Auto;

    let mut i = 0;
    while i < args.len() {
        let (name, inline) = split_option(&args[i]);
        match name {
            _ if is_input(name) => take_input(&mut input_path, &args[i]),
            "-h" | "--help" => asked_for(&help),
            "-o" | "--output" => output_path = Some(value(args, &mut i, inline).to_string()),
            "-d" | "--dialect" => {
                let given = value(args, &mut i, inline);
                dialect = Dialect::from_name(given).unwrap_or_else(|| {
                    eprintln!("Invalid dialect: {}", given);
                    eprintln!("Valid options: auto, generic, postgres, mysql");
                    process::exit(1);
                });
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                process::exit(1);
            }
        }
        i += 1;
    }
    let input_path = &input_or_help(input_path, &help);

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
