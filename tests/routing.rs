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

/// Length two axis-aligned segments share while lying on the same line.
fn shared_length(a: (&(f64, f64), &(f64, f64)), b: (&(f64, f64), &(f64, f64))) -> f64 {
    let ((a1, a2), (b1, b2)) = (a, b);
    let on_same_line = |p: f64, q: f64| (p - q).abs() < 1.0;

    if a1.0 == a2.0 && b1.0 == b2.0 && on_same_line(a1.0, b1.0) {
        let (lo_a, hi_a) = (a1.1.min(a2.1), a1.1.max(a2.1));
        let (lo_b, hi_b) = (b1.1.min(b2.1), b1.1.max(b2.1));
        return hi_a.min(hi_b) - lo_a.max(lo_b);
    }
    if a1.1 == a2.1 && b1.1 == b2.1 && on_same_line(a1.1, b1.1) {
        let (lo_a, hi_a) = (a1.0.min(a2.0), a1.0.max(a2.0));
        let (lo_b, hi_b) = (b1.0.min(b2.0), b1.0.max(b2.0));
        return hi_a.min(hi_b) - lo_a.max(lo_b);
    }
    f64::NEG_INFINITY
}

#[test]
fn paths_do_not_run_on_top_of_each_other() {
    // Two edges may cross, but one hiding inside another leaves the reader
    // unable to tell how many relations are drawn.
    for (name, layout) in examples() {
        for (i, first) in layout.edges.iter().enumerate() {
            for second in layout.edges.iter().skip(i + 1) {
                for a in first.waypoints.windows(2) {
                    for b in second.waypoints.windows(2) {
                        let shared = shared_length((&a[0], &a[1]), (&b[0], &b[1]));
                        assert!(
                            shared <= MIN_JOG,
                            "{name}: {} -> {} and {} -> {} run together for {shared}: {:?} / {:?}",
                            first.from,
                            first.to,
                            second.from,
                            second.to,
                            first.waypoints,
                            second.waypoints
                        );
                    }
                }
            }
        }
    }
}

/// True when two axis-aligned segments cross at a point interior to both.
fn crosses(a: (&(f64, f64), &(f64, f64)), b: (&(f64, f64), &(f64, f64))) -> bool {
    let vertical = |(p, q): (&(f64, f64), &(f64, f64))| p.0 == q.0;
    if vertical(a) == vertical(b) {
        return false;
    }
    let (v, h) = if vertical(a) { (a, b) } else { (b, a) };
    let x = v.0 .0;
    let y = h.0 .1;
    let (top, bottom) = (v.0 .1.min(v.1 .1), v.0 .1.max(v.1 .1));
    let (left, right) = (h.0 .0.min(h.1 .0), h.0 .0.max(h.1 .0));
    top < y && y < bottom && left < x && x < right
}

fn crossings(layout: &Layout) -> usize {
    let mut count = 0;
    for (i, first) in layout.edges.iter().enumerate() {
        for second in layout.edges.iter().skip(i + 1) {
            for a in first.waypoints.windows(2) {
                for b in second.waypoints.windows(2) {
                    if crosses((&a[0], &a[1]), (&b[0], &b[1])) {
                        count += 1;
                    }
                }
            }
        }
    }
    count
}

fn bends(layout: &Layout) -> usize {
    layout
        .edges
        .iter()
        .map(|e| e.waypoints.len().saturating_sub(2))
        .sum()
}

/// The crossings and bends each example is allowed to draw. These are not
/// targets, they are ratchets: when a change improves a diagram, tighten its
/// numbers so the improvement cannot quietly slip away again.
const BUDGET: &[(&str, usize, usize)] = &[
    ("01_many_columns.erd", 0, 0),
    ("02_wide_horizontal.erd", 0, 0),
    ("03_long_names.erd", 0, 0),
    ("04_deep_hierarchy.erd", 0, 0),
    ("05_dense_relations.erd", 11, 22),
    ("06_unicode_cjk.erd", 0, 4),
    ("07_all_cardinalities.erd", 1, 8),
    ("08_mixed_sizes.erd", 0, 0),
    ("09_orphan_entities.erd", 0, 0),
    ("10_ecommerce_full.erd", 12, 24),
    ("11_arrangement.erd", 19, 28),
    ("21_idp.erd", 40, 34),
    ("sample.erd", 0, 6),
];

#[test]
fn layouts_stay_within_their_crossing_and_bend_budget() {
    for (name, layout) in examples() {
        let Some(&(_, max_crossings, max_bends)) = BUDGET.iter().find(|(n, _, _)| *n == name)
        else {
            panic!("{name}: no budget recorded; add one to BUDGET");
        };
        let (crossings, bends) = (crossings(&layout), bends(&layout));
        assert!(
            crossings <= max_crossings && bends <= max_bends,
            "{name}: {crossings} crossings and {bends} bends, over the budget of \
             {max_crossings} and {max_bends}"
        );
        if crossings < max_crossings || bends < max_bends {
            println!("{name}: down to {crossings} crossings and {bends} bends; tighten BUDGET");
        }
    }
}
