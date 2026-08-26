# Development

```bash
cargo test                    # includes routing checks over examples/
bin/build                     # release binary + wasm-pack build
bin/svg examples/sample.erd   # render one file next to its source
bin/dev                       # render every example
bin/docs                      # regenerate the diagrams in the README
cd site && bun run dev        # docs and demo on the local wasm build
```

## Releasing

```bash
bin/release patch             # show what a patch release would do
bin/release minor --yes       # bump, commit, tag, publish to crates.io and npm, push
bin/release current --yes     # release the version already in Cargo.toml
```

## Layout

The compiler is a pipeline: lexer, parser, `GraphIR`, `LayoutEngine`,
`SvgRenderer`. The layout is where the work is, and it runs in this order:

1. **Rows** (`layout/layering.rs`) — an entity goes below the ones it
   references, on the row that keeps the relationships shortest. Network
   simplex, as `dot` solves it.
2. **Order within a row** (`ordering.rs`) — median sweeps and adjacent swaps,
   run from several fixed starting points.
3. **Columns** (`layout/placement.rs`, `layout/align.rs`) — each entity slid
   towards the ones it relates to, without reordering the row.
4. **Edges** (`layout/anchors.rs`, `layout/descent.rs`, `layout/lanes.rs`,
   `layout/waypoints.rs`) — where each edge leaves its entity, which channel it
   steps across in, which lane it takes, and the path that comes out.
5. **Rows again** (`layout/placement.rs`) — only once the edges are planned is
   it known how much room each channel needs.

Steps 2 and 3 are searched rather than computed: several arrangements are drawn
and the tidiest kept, weighed by `Layout::is_tidier_than` — a hidden relation
first, then a line cut across an entity's only relation, then crossings, then
how far apart the entities of a `@hint.near` set ended up, then corners.

## Tests

`tests/routing.rs` holds every example to a budget of crossings and bends. The
numbers are ratchets: when a change improves a diagram, tighten them so the
improvement cannot slip away. `tests/grammar.rs` generates documents from the
GBNF grammars and puts them through the compiler.
