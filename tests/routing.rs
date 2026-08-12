//! Routing quality checks over the bundled examples.

use rusterd::ir::{DetailLevel, GraphIR};
use rusterd::layout::{Layout, LayoutEngine, LayoutNode};
use rusterd::parser::Parser;
use std::fs;
use std::path::PathBuf;

/// Detours shorter than this read as an accidental wiggle rather than a turn.
const MIN_JOG: f64 = 20.0;

fn examples() -> Vec<(String, Layout)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .expect("examples directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "erd"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no examples found");

    paths
        .into_iter()
        .map(|path| {
            let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let input = fs::read_to_string(&path).expect(&name);
            let schema = Parser::new(&input)
                .expect(&name)
                .parse()
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            let ir = GraphIR::from_schema(&schema, None, DetailLevel::All);
            (name, LayoutEngine::default().layout(&ir))
        })
        .collect()
}

/// True when the axis-aligned segment overlaps the node's interior.
fn overlaps(node: &LayoutNode, (x1, y1): (f64, f64), (x2, y2): (f64, f64)) -> bool {
    let margin = 0.5;
    x1.min(x2) < node.x + node.width - margin
        && x1.max(x2) > node.x + margin
        && y1.min(y2) < node.y + node.height - margin
        && y1.max(y2) > node.y + margin
}

#[test]
fn paths_have_no_micro_jogs() {
    for (name, layout) in examples() {
        for edge in &layout.edges {
            for (i, segment) in edge.waypoints.windows(2).enumerate() {
                let is_interior = i > 0 && i + 2 < edge.waypoints.len();
                if !is_interior {
                    continue;
                }
                let len = (segment[1].0 - segment[0].0)
                    .abs()
                    .max((segment[1].1 - segment[0].1).abs());
                assert!(
                    len >= MIN_JOG,
                    "{name}: {} -> {} detours by {len} at segment {i}: {:?}",
                    edge.from,
                    edge.to,
                    edge.waypoints
                );
            }
        }
    }
}

#[test]
fn paths_have_no_redundant_waypoints() {
    for (name, layout) in examples() {
        for edge in &layout.edges {
            for turn in edge.waypoints.windows(3) {
                let (px, py) = turn[0];
                let (x, y) = turn[1];
                let (nx, ny) = turn[2];
                let cross = (x - px) * (ny - y) - (y - py) * (nx - x);
                assert!(
                    cross.abs() > f64::EPSILON,
                    "{name}: {} -> {} has a redundant waypoint: {:?}",
                    edge.from,
                    edge.to,
                    edge.waypoints
                );
            }
        }
    }
}

#[test]
fn paths_do_not_cross_entities() {
    for (name, layout) in examples() {
        for edge in &layout.edges {
            for node in &layout.nodes {
                // The endpoints touch their own entities by design.
                if node.id == edge.from || node.id == edge.to {
                    continue;
                }
                for segment in edge.waypoints.windows(2) {
                    assert!(
                        !overlaps(node, segment[0], segment[1]),
                        "{name}: {} -> {} crosses {}: {:?}",
                        edge.from,
                        edge.to,
                        node.id,
                        edge.waypoints
                    );
                }
            }
        }
    }
}
