# rusterd ERD DSL

A text format that compiles to an SVG entity-relationship diagram. This
document describes the language as the parser actually accepts it; it is meant
to be handed to a model that has to author `.erd` files.

[`erd.gbnf`](erd.gbnf) is the same language as a GBNF grammar, for constrained
decoding (`llama-cli --grammar-file docs/erd.gbnf`). It is stricter than the
parser on purpose — one canonical layout, four-space indents — so that
everything it produces parses. [`sql.gbnf`](sql.gbnf) does the same for the DDL
subset that `rusterd convert` reads.

Both grammars are covered by `cargo test --test grammar`, which generates
documents from them and runs the result through the compiler.

## File structure

A file is a sequence of these top-level items, in any order:

| Item | Repeatable |
| --- | --- |
| `entity NAME { ... }` | yes, one per entity |
| `rel { ... }` | yes, all blocks are merged |
| `view NAME { ... }` | yes, one per view |
| `@hint.arrangement = { ... }` | once (a second one replaces the first) |

Line comments start with `#` and run to the end of the line. Blank lines are
free. Anything else at the top level is an error.

## Lexical rules

- **Identifiers** start with a letter or `_` and continue with letters, digits
  or `_`. Letters may be non-ASCII, so `注文` and `顧客ID` are valid names.
- **Strings** are double-quoted and used only for relationship labels and some
  hint values.
- **Numbers** are integers.
- A **column type is a bare identifier**. `varchar(255)` is a parse error —
  write `varchar`. Length, precision and other parameters have no place in this
  language.

## Entities

```erd
entity User {
    id int pk
    email varchar unique not null
    name varchar
    created_at timestamp default now()
}
```

A column is `NAME TYPE [MODIFIER ...]` and ends at the end of the line, so
exactly one column per line.

| Modifier | Meaning | Drawn as |
| --- | --- | --- |
| `pk` | primary key | `◆` and bold |
| `fk -> Entity.column` | foreign key target | italic |
| `not null` | both words, in this order | not drawn |
| `unique` | | not drawn |
| `default VALUE` | identifier, number, string or `func(...)` | not drawn |

Table-level constraints may appear between the columns:

```erd
entity OrderItem {
    order_id int not null fk -> Order.id
    product_id int not null fk -> Product.id
    quantity int not null
    primary_key(order_id, product_id)
}
```

- `primary_key(a, b)` — marks those columns as primary keys, exactly like `pk`.
- `foreign_key(a) references Target(b) [on delete ACTION] [on update ACTION]` —
  parsed and kept in the schema, but **not drawn**.
- `index(a, b) [name=ix_name]` — parsed, **not drawn**.

## Relationships

**This is the only thing that draws a line.** A column marked `fk -> User.id`
is rendered in italics but produces no edge; every relationship you want to see
must be written in a `rel` block.

```erd
rel {
    Category 0..1 -- * Category : "parent"
    User 1 -- * Order : "places"
    Order 1 -- 1..* OrderItem : "contains"
}
```

Syntax: `LEFT CARDINALITY -- CARDINALITY RIGHT [: "label"] [as role]`

The separator is exactly `--`. The optional label is a quoted string. The
optional `as role` is parsed but not drawn.

| Cardinality | Meaning | Crow's foot |
| --- | --- | --- |
| `1` | exactly one | one tick |
| `0..1` | zero or one | tick and circle |
| `*` | many | crow's foot |
| `1..*` | one or more | crow's foot and tick |

Those four are the whole set. `0..*`, `1..1` and `2..5` are parse errors.

An entity may relate to itself (`Category 0..1 -- * Category`), which draws a
loop on its right-hand side.

## Views

A view names a subset of the entities. Relationships are kept when both of
their entities are in the subset.

```erd
view checkout {
    include User, Order
    include OrderItem
}
```

Several `include` lines are allowed and are concatenated. Views change nothing
unless the renderer is asked for one by name.

## Layout

Placement is a grid: one row per level, entities left to right within a row.
**Write nothing and it is worked out for you.** An entity goes below the ones
it references — the `1` end of a relationship is the parent — on the row that
keeps the relationships as short as possible, and the order within each row is
searched for the arrangement that crosses least.

Say where things go only when you want something other than that:

```erd
@hint.arrangement = {
    Category User
    Product Order
    OrderItem
}
```

Rows are separated by a newline or a `;`. Entities missing from the arrangement
fall to level 0.

Inside an entity, `@hint.level = 2` puts it on that level. `@hint.group =
"core"` is parsed but currently unused, as is any other `@hint.*` key.

An arrangement or a level hint anywhere in the file turns the automatic
placement off for the whole diagram, so entities you did not mention land on
level 0. Place all of them or none of them.

## Render-time options

These are not part of the file. They are chosen when rendering:

- **view**: `-v checkout` renders only that view.
- **detail**: `-d tables | pk | pk_fk | all` (default `all`) filters which
  columns are drawn.
- **notation**: `-n crowsfoot | text` (default `crowsfoot`) switches between
  crow's foot symbols and `1` / `0..1` / `*` / `1..*` written beside the line.
- **legend**: `-l` draws a key to the four cardinalities below the diagram, in
  whichever notation is in use.

## Mistakes to avoid

| Mistake | Result |
| --- | --- |
| Relying on `fk ->` to draw a relationship | no line is drawn |
| `varchar(255)`, `decimal(10, 2)` | `Parse error: Unexpected token: LParen` |
| `0..*` or `1..1` | parse error; use `*` or `1` |
| Two columns on one line | the second is read as a modifier and fails |
| Referring to an entity that is not defined | the relationship is dropped silently |
| Naming only some entities in the arrangement | the rest land on level 0 |

## Complete example

```erd
# Online shop

entity Category {
    id int pk
    parent_id int fk -> Category.id
    name varchar not null
}

entity User {
    id int pk
    email varchar unique not null
    created_at timestamp default now()
}

entity Product {
    id int pk
    category_id int fk -> Category.id
    name varchar not null
    price decimal
}

entity Order {
    id int pk
    user_id int fk -> User.id
    status varchar not null
}

entity OrderItem {
    order_id int not null fk -> Order.id
    product_id int not null fk -> Product.id
    quantity int not null
    primary_key(order_id, product_id)
}

rel {
    Category 0..1 -- * Category : "parent"
    Category 1 -- * Product
    User 1 -- * Order : "places"
    Order 1 -- 1..* OrderItem : "contains"
    Product 1 -- * OrderItem
}

@hint.arrangement = {
    Category User
    Product Order
    OrderItem
}

view checkout {
    include User, Order, OrderItem
}
```
