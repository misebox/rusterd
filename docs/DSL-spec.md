# Improved ER Diagram DSL Specification (Draft)

## 1. Overview

This DSL defines data entities, their attributes, constraints, and relationships.
It aims to be human-readable, layout-friendly, and structurally precise, improving on Mermaid's `erDiagram` syntax while remaining compact.

The DSL separates **entity definitions**, **relationship definitions**, and **view / layout hints**.

---

## 2. Comments

Line comments begin with `#`. Everything after `#` until end of line is ignored.

```
# This is a comment
entity User {
  id int pk  # inline comment
}
```

---

## 3. Entity Definition

```
entity EntityName {
  column_name   type      [constraint1 constraint2 ...]
  ...
  primary_key (col1, col2?)
  foreign_key (col) references Target(col) [options]
  index (col1, col2?) [name=...]
}
```

### 2.1 Columns

* Syntax: `<name> <type> [constraints...]`
* Supported constraints:

  * `pk`
  * `not null`
  * `unique`
  * `default <value>`
  * `fk -> Target.column`

### 2.2 Structural Constraints

Optional explicit constraint blocks:

* `primary_key(...)`
* `foreign_key(col) references Target(col) on delete ... on update ...`
* `index(...)`

These blocks override or complement inline constraints.

---

## 4. Relationships

Relationships are defined in a dedicated block:

```
rel {
  A 1 -- * B [:label] [as role]
}
```

### Semantics:

* Cardinalities: `1`, `0..1`, `*`, `1..*`
* Roles: optional, for distinguishing multiple edges between the same entities.

Example:

```
rel {
  User 1 -- * Order         : "places"
  User 1 -- * Order as approver
}
```

---

## 5. Layout Hints (Optional)

Entities may include layout metadata to improve diagram rendering.

```
entity User {
  @hint.level = 0
  @hint.group = "core"
  @hint.anchor = center
}
```

### Supported hints:

* `level`: integer layer (0 = top/center)
* `group`: logical cluster name
* `anchor`: `center` | `left` | `right`
* `side`: preferred region (`top`, `bottom`, `left`, `right`)
* Hints influence layout heuristics but never affect schema semantics.

---

## 6. Views (Subsets of the Schema)

For large schemas, diagrams can be generated per-view:

```
view core {
  include User, Order, OrderItem
}
```

A view defines which entities/relations appear in a rendered diagram.

---

## 7. Rendering Model (Informal)

This DSL is intended to support Graph IR generation and layout engines.

Key principles:

* Entities become nodes with measured dimensions (text-based width/height).
* Relationships become edges with cardinality markers and optional labels/roles.
* Layout engine uses:

  * Hierarchical layering (from `level` or FK direction)
  * Cluster grouping (from `group`)
  * Crossing minimization heuristics
  * Optional orthogonal or polyline routing

---

## 8. Global Detail Level Specification

### 1. Overview

The rendering engine supports configurable **detail levels** that determine how much information is shown for each entity in a diagram.
This setting is **global per rendering invocation** and applies uniformly to all entities and relationships in the output.

The DSL itself contains **no per-view or per-entity detail configuration**.

---

### 2. Detail Levels

The renderer SHALL support the following predefined detail levels:

#### **`tables`**

* Render entity boxes using **names only**
* Omit all attributes
* Show relationships between entities

#### **`pk`**

* Show **primary key columns only**
* Hide non-PK attributes
* Show relationships normally

#### **`pk_fk`**

* Show **primary key** and **foreign key** columns
* Hide all other columns
* Display foreign-key relationships normally

#### **`all`**

* Show **all attributes** of each entity
* Full-detail ER rendering

These four levels provide predictable, consistent diagram abstraction while keeping the DSL simple.

---

### 3. Global Rendering Parameter

The detail level is selected **at render time**, not in the DSL:

```
er render --view core --detail pk_fk
```

or programmatically:

```ts
render(schema, {
  view: "core",
  detail: "pk_fk"
})
```

### 4. Resolution Rules

* The global detail level applies to **all included entities and relationships**.
* Views **may not** override detail levels.
* Entities **may not** specify their own detail levels.
* The renderer must treat the detail level as a pure visualization filter;
  schema semantics (PK, FK, constraints) remain unchanged internally.

---

### 5. Rationale

* Keeps the DSL clean and stable
* Allows the same schema or view to be visualized in multiple abstraction levels
* Avoids configuration explosion (per-entity or per-view overrides)
* Keeps rendering deterministic and easy to reason about


## 9. Goals

* Stronger structural representation than Mermaid’s `erDiagram`
* Clean separation between schema and visualization
* Human-friendly syntax, minimal punctuation
* Future-proof for DDL generation, schema diffs, and automated documentation


