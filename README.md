# rusterd

[![check](https://github.com/misebox/rusterd/actions/workflows/check.yml/badge.svg)](https://github.com/misebox/rusterd/actions/workflows/check.yml)
[![crates.io](https://img.shields.io/crates/v/rusterd.svg)](https://crates.io/crates/rusterd)
[![npm](https://img.shields.io/npm/v/rusterd.svg)](https://www.npmjs.com/package/rusterd)
[![docs.rs](https://img.shields.io/docsrs/rusterd)](https://docs.rs/rusterd)
[![license](https://img.shields.io/crates/l/rusterd.svg)](LICENSE)

ER diagram DSL compiler that renders to SVG. Written in Rust, compiles to WASM for browser use.

Documentation and demo: https://misebox.github.io/rusterd/

## Features

- **Entities**: Define tables with typed columns
- **Column types**: `int`, `string`, `decimal`, `timestamp`, `boolean`, `text`
- **Constraints**: `pk`, `fk -> Entity.column`, `not null`, `unique`
- **Relationships**: Support all cardinalities (`1`, `*`, `0..1`, `1..*`)
- **Self-references**: Entities can reference themselves
- **Automatic layout**: Levels, order and position are all worked out; hints say what matters, not where things go
- **Focus**: Draw one part of the schema with a `focus` block
- **Detail levels**: Control what's shown (tables only, pk, pk+fk, all columns)

## Example

`examples/sample.erd`, and what `rusterd render` makes of it:

```erd
# Self-referential entity
entity Category {
    id int pk
    parent_id int fk -> Category.id
    name string not null
}

entity User {
    id int pk
    email string unique not null
    name string
    created_at timestamp
}

entity Product {
    id int pk
    category_id int fk -> Category.id
    name string not null
    price decimal
    is_active boolean
}

entity Order {
    id int pk
    user_id int fk -> User.id
    total decimal
    status string not null
}

# All cardinality types: 1, *, 0..1, 1..*
rel {
    Category 1 -- * Category : "parent"
    Category 1 -- * Product
    User 1 -- * Order : "places"
    User 0..1 -- 1..* Product : "favorites"
}

# A named part of the diagram
focus simple {
    include User, Order
}
```

![Rendered ERD](https://raw.githubusercontent.com/misebox/rusterd/main/docs/sample.svg)

## Install

| Use it as | How |
| --- | --- |
| Browser / bundler | `npm i rusterd` (also `bun add` / `pnpm add`) |
| CLI | `cargo install rusterd` |
| Rust library | `rusterd = { git = "https://github.com/misebox/rusterd" }` |

## CLI Usage

```bash
# Render to file
rusterd render input.erd -o output.svg

# Draw only what a focus block lists
rusterd render input.erd -f simple -o output.svg

# Control detail level
rusterd render input.erd -d pk_fk -o output.svg

# Cardinality notation
rusterd render input.erd -n text -o output.svg

# Add a key to the cardinality symbols
rusterd render input.erd --legend -o output.svg

# Close up the spacing, to fit more on a screen
rusterd render input.erd --dense -o output.svg

# Read from stdin
cat input.erd | rusterd render - -o output.svg

# Convert a SQL dump to ERD notation
rusterd convert schema.sql -o schema.erd
rusterd convert schema.sql -d postgres

# Which version this is
rusterd --version
```

## Options

The same six wherever they are given: a flag on the command line, a field in
the options object in the browser. All are optional, and leaving one out is how
you ask for its default.

| Option | CLI | Field | Values | Default |
| --- | --- | --- | --- | --- |
| Focus | `-f`, `--focus <name>` | `focus` | the name of a `focus` block | the whole diagram |
| Detail | `-d`, `--detail <level>` | `detail` | `tables`, `pk`, `pk_fk`, `all` | `all` |
| Notation | `-n`, `--notation <name>` | `notation` | `crowsfoot`, `text` | `crowsfoot` |
| Legend | `-l`, `--legend` | `legend` | — | off |
| Dense | `-D`, `--dense` | `dense` | — | off |
| Dialect | `-d`, `--dialect <name>` | `dialect` | `auto`, `generic`, `postgres`, `mysql` | `auto` |

**Detail** is how much of an entity is drawn: `tables` its name alone, `pk` its
primary keys, `pk_fk` its keys of both kinds, `all` every column it has.

**Notation** is how the cardinalities are drawn: `crowsfoot` as symbols on the
line itself, `text` as `1` / `0..1` / `*` / `1..*` in a pill beside it.
**Legend** draws a key to the four below the diagram, in whichever notation is
in use.

**Dense** closes up the gaps between the entities and around the lines, for
fitting a large schema on one screen. The text stays the size it has to be to
read, which is why zooming out is not the same thing. There is no setting the
other way round.

**Dialect** is the only one that is about reading rather than drawing. On the
command line it belongs to `rusterd convert`, which writes ERD rather than a
diagram, and the drawing options belong to `rusterd render`; that is why both
are spelled `-d`. In the browser `sqlToSvg` reads and draws in one call, so it
takes all six together.

## Browser Usage (WASM)

```javascript
import init, { erdToSvg, erdToDataUri, sqlToErd, sqlToSvg } from 'rusterd';

await init();  // the default export: loads the wasm, once, before anything else
```

**Every function returns a `string`, and throws a `string` when it cannot.**
There is no result object to unwrap, and what is thrown is a plain string, not
an `Error` — so `catch (message)`, not `catch (e) { e.message }`.

| Function | Returns |
| --- | --- |
| `erdToSvg` | the markup, `<svg …>…</svg>` |
| `erdToDataUri` | `data:image/svg+xml,…`, ready for `<img src={…}>` |
| `sqlToErd` | ERD source, as a `.erd` file would hold it |
| `sqlToSvg` | the markup, converting on the way |

```typescript
type Detail = "tables" | "pk" | "pk_fk" | "all";
type Notation = "crowsfoot" | "text";
type Dialect = "auto" | "generic" | "postgres" | "mysql";

erdToSvg(source: string, options?: DrawOptions | null): string
erdToDataUri(source: string, options?: DrawOptions | null): string
sqlToErd(sql: string, dialect?: Dialect | null): string
sqlToSvg(sql: string, options?: ConvertOptions | null): string

interface DrawOptions {
  focus?: string | null;
  detail?: Detail | null;
  notation?: Notation | null;
  legend?: boolean | null;
  dense?: boolean | null;
}

interface ConvertOptions extends DrawOptions {
  dialect?: Dialect | null;
}
```

The package ships these, so an editor completes the values and TypeScript
refuses a misspelt one. JavaScript is told at run time instead: a value that is
not one of these throws, rather than quietly drawing the default.

```javascript
erdToSvg(source, { detail: 'pkfk' });
// Unknown detail: "pkfk" (expected "tables", "pk", "pk_fk", "all")
```

Say what you mean and leave out the rest.

```javascript
const whole = erdToSvg(source);
const part = erdToSvg(source, { focus: 'checkout' });
const keys = erdToSvg(source, { detail: 'pk_fk', notation: 'text' });
const tight = erdToSvg(source, { legend: true, dense: true });

document.querySelector('img').src = erdToDataUri(source);

const svg = sqlToSvg(dump, { dialect: 'postgres', detail: 'pk_fk' });

try {
  erdToSvg('entity {');
} catch (message) {
  console.error(message);  // "Unexpected token: LBrace, expected identifier"
}
```

`sqlToErd` skips statements it does not recognise rather than failing, so a
dump it cannot read at all comes back as `''`. Check for that before drawing:

```javascript
const erd = sqlToErd(dump, 'postgres');
if (!erd.trim()) {
  throw new Error('No tables found. Check the SQL, or name the dialect.');
}
```

## Rust Library Usage

Parse, build the graph, lay it out, render:

```rust
use rusterd::ir::{DetailLevel, GraphIR};
use rusterd::layout::LayoutEngine;
use rusterd::parser::Parser;
use rusterd::svg::SvgRenderer;

let schema = Parser::new(source)?.parse()?;
let ir = GraphIR::from_schema(&schema, None, DetailLevel::All);
let layout = LayoutEngine::default().layout(&ir);
let svg = SvgRenderer::default().render(&ir, &layout);
// or LayoutEngine::default().with_dense_spacing(true)
// or SvgRenderer::default().with_notation(Notation::Text).with_legend(true)
```

`rusterd::sql::parse_sql` plus `rusterd::serializer::serialize` cover the SQL
to ERD direction.

## Syntax Reference

Full language reference: [docs/DSL-spec.md](docs/DSL-spec.md), written to be
handed to an LLM, with GBNF grammars for constrained generation:
[docs/erd.gbnf](docs/erd.gbnf) for the DSL and [docs/sql.gbnf](docs/sql.gbnf)
for the DDL subset `convert` reads. `cargo test --test grammar` generates
documents from both and checks that the compiler accepts them.

For handing to a model rather than to a person, the documentation site serves
the same material as plain text at fixed paths: `/llms-full.txt` is the
reference and the options in one file, `/erd.gbnf` and `/sql.gbnf` are the
grammars, and `/llms.txt` says which is which.

### Entities

```erd
entity EntityName {
    column_name type [constraints]
}
```

### Relationships

```erd
rel {
    Entity1 cardinality -- cardinality Entity2 [: "label"]
}
```

Cardinalities: `1`, `*`, `0..1`, `1..*`

### Layout Hints

Placement is automatic. The hints say what matters about the diagram:

```erd
# Draw these close to one another
@hint.near = { Order, OrderItem, Payment }

# Leave these out entirely / draw only their name
@hint.omit  = { schema_migrations }
@hint.brief = { audit_logs }

# Pin an entity to a row (inside the entity body); its order and
# position across the page are still worked out
entity EntityName {
    @hint.level = 2
    column_name type
}
```

### Focus

```erd
focus focus_name {
    include Entity1, Entity2, Entity3
}
```

## Examples

See `examples/` directory for more samples including:
- Many columns layout
- Deep hierarchies
- Dense relationships
- Unicode/CJK text
- Self-referential entities
- Complex e-commerce schema

## License

MIT
