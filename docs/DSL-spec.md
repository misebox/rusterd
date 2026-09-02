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

This document and the render options are served as one plain-text file at
`/llms-full.txt` on the documentation site, with `/llms.txt` as the index
naming it and the grammars.

## File structure

A file is a sequence of these top-level items, in any order:

| Item | Repeatable |
| --- | --- |
| `entity NAME { ... }` | yes, one per entity |
| `rel { ... }` | yes, all blocks are merged |
| `focus NAME { ... }` | yes, one per focus |
| `@hint.near = { ... }` | yes, each is a separate set |
| `@hint.omit = { ... }` | yes, all are merged |
| `@hint.brief = { ... }` | yes, all are merged |

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

entity Post {
    id int pk
    author_id int fk -> User.id
    title varchar not null
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
[`examples/07_all_cardinalities.erd`](examples/07_all_cardinalities.erd) draws
one of each, in both notations.

An entity may relate to itself (`Category 0..1 -- * Category`), which draws a
loop on its right-hand side.

## Focus

A focus names a part of the schema worth drawing on its own. Relationships are
kept when both of their entities are in it.

```erd
focus checkout {
    include User, Order
    include OrderItem
}
```

Several `include` lines are allowed and are concatenated. A focus changes
nothing unless the renderer is asked for one by name. It is not a SQL view —
nothing about the schema is being defined, only what to draw.

## Layout

**Placement is worked out for you.** The diagram is drawn in levels — bands of
entities across the page, level 0 at the top, counting downwards. An entity
goes on a level below the ones it references, the `1` end of a relationship
being the parent; which level is the one that keeps its relationships shortest.
The order within a level is searched for the arrangement that crosses least,
and each entity is then slid across to sit under what it relates to.

There is no way to say where an entity goes. What can be said is what matters
about the diagram.

### Keeping entities together

```erd
@hint.near = { Order, OrderItem, Payment }
@hint.near = { User, UserProfile }
```

Entities in a set are drawn close to one another. Write as many sets as you
like; an entity may be in several. This does not draw anything — it is read as
if those entities were related, so the layout keeps them side by side or one
above the other.

### Leaving things out

```erd
@hint.omit  = { schema_migrations, ar_internal_metadata }
@hint.brief = { audit_logs, events }
```

`omit` drops an entity and its relationships from the diagram. `brief` keeps the
entity and its relationships but draws only its name, for a table whose columns
are noise. A `focus` is the opposite of `omit`: it names what to keep.

### Choosing the levels

```erd
entity Team {
    @hint.level = 0
    id int pk
    name varchar not null
}

entity Member {
    @hint.level = 1
    id int pk
    team_id int fk -> Team.id
}

entity ApiKey {
    @hint.level = 2
    id int pk
    member_id int fk -> Member.id
}

rel {
    Team 1 -- * Member
    Member 1 -- * ApiKey
}
```

`@hint.level = N` puts an entity on level N. Its order within that level, and
its position across the page, are still worked out.

An entity with no hint is placed around the ones that have: below what it
references, above what references it, and as near to them as that allows.
Naming the level of one table is not a decision about the rest of them, so
pinning one, some or all of them is all the same to the compiler.

Pinning a child above its parent asks for something a diagram drawn downwards
cannot have. Both hints are still obeyed; what gives is the relation between
them, which reaches upwards instead.

## Render-time options

Nothing in this document is affected by these: they are chosen when the file is
rendered rather than written in it. There are seven — focus, detail, notation,
legend, dense, aspect, and dialect for reading SQL — and the same seven are
given as flags on the command line or as fields in the browser. The
[options table](../README.md#options) says what each one takes.

## Mistakes to avoid

| Mistake | Result |
| --- | --- |
| Relying on `fk ->` to draw a relationship | no line is drawn |
| `varchar(255)`, `decimal(10, 2)` | ``line 3, column 18: expected a name, found `(` `` |
| `0..*` or `1..1` | parse error; use `*` or `1` |
| Two columns on one line | the second is read as a modifier and fails |
| Referring to an entity that is not defined | the relationship is dropped silently |
| Pinning a child above its parent | the relation between them reaches upwards |

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

@hint.near = { Order, OrderItem }

focus checkout {
    include User, Order, OrderItem
}
```
