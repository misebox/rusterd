#[derive(Debug, Clone, PartialEq)]
pub struct Schema {
    pub entities: Vec<Entity>,
    pub relationships: Vec<Relationship>,
    pub views: Vec<View>,
    /// Grid-based layout arrangement: rows of entity names
    /// Each row represents a level, columns represent horizontal order
    pub arrangement: Option<Vec<Vec<String>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    pub name: String,
    pub columns: Vec<Column>,
    pub constraints: Vec<Constraint>,
    pub hints: Vec<Hint>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub name: String,
    pub typ: String,
    pub modifiers: Vec<ColumnModifier>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ColumnModifier {
    Pk,
    NotNull,
    Unique,
    Default(String),
    Fk { target: String, column: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Constraint {
    PrimaryKey(Vec<String>),
    ForeignKey {
        columns: Vec<String>,
        target: String,
        target_columns: Vec<String>,
        on_delete: Option<String>,
        on_update: Option<String>,
    },
    Index {
        columns: Vec<String>,
        name: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hint {
    pub key: String,
    pub value: HintValue,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HintValue {
    Int(i64),
    Str(String),
    Ident(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Relationship {
    pub left: String,
    pub left_cardinality: Cardinality,
    pub right: String,
    pub right_cardinality: Cardinality,
    pub label: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cardinality {
    One,        // 1
    ZeroOrOne,  // 0..1
    Many,       // *
    OneOrMore,  // 1..*
}

#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub name: String,
    pub includes: Vec<String>,
}
