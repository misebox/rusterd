# rusterd

ER diagram DSL compiler that renders to SVG. Written in Rust, compiles to WASM for browser use.

## Features

- **Entities**: Define tables with typed columns
- **Column types**: `int`, `string`, `decimal`, `timestamp`, `boolean`, `text`
- **Constraints**: `pk`, `fk -> Entity.column`, `not null`, `unique`
- **Relationships**: Support all cardinalities (`1`, `*`, `0..1`, `1..*`)
- **Self-references**: Entities can reference themselves
- **Layout hints**: Grid-based positioning with `@hint.arrangement`
- **Views**: Filter diagrams with `view` blocks
- **Detail levels**: Control what's shown (tables only, pk, pk+fk, all columns)

## Example

```erd
@hint.arrangement = {
    User;
    Order OrderItem;
    Product
}

entity User {
    id int pk
    email string unique not null
    name string
}

entity Order {
    id int pk
    user_id int fk -> User.id
    total decimal
}

entity Product {
    id int pk
    name string not null
    price decimal
}

entity OrderItem {
    order_id int pk fk -> Order.id
    product_id int pk fk -> Product.id
    quantity int not null
}

rel {
    User 1 -- * Order
    Order 1 -- * OrderItem
    Product 1 -- * OrderItem
}

view simple {
    include User, Order
}
```

## CLI Usage

```bash
# Build
cargo build --release

# Render to file
rusterd input.erd -o output.svg

# Render specific view
rusterd input.erd -v simple -o output.svg

# Control detail level
rusterd input.erd -d pk_fk -o output.svg
```

**Detail levels:**
- `tables` - Entity names only
- `pk` - Primary keys
- `pk_fk` - Primary and foreign keys
- `all` - All columns (default)

## WASM Usage

```bash
# Build
wasm-pack build --target web

# Run demo
cd demo
bun install
bun run dev
```

```javascript
import init, { erdToSvg } from 'rusterd';

await init();
const svg = erdToSvg(erdSource);           // Full diagram
const svg = erdToSvg(erdSource, 'simple'); // Specific view
const svg = erdToSvg(erdSource, null, 'pk_fk'); // Detail level
```

## Syntax Reference

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

```erd
# Grid-based arrangement (semicolons separate rows)
@hint.arrangement = {
    Entity1 Entity2;
    Entity3 Entity4
}

# Entity-specific level hint
entity EntityName @hint.level = 2 {
    ...
}
```

### Views

```erd
view view_name {
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
