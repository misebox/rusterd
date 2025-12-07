//! Serializer for converting AST to ERD notation string.

use crate::ast::{
    Cardinality, Column, ColumnModifier, Constraint, Entity, Relationship, Schema,
};
use std::collections::{HashMap, HashSet};

/// Serialize a Schema to ERD notation string.
pub fn serialize(schema: &Schema) -> String {
    let mut output = String::new();

    // Serialize entities
    for (i, entity) in schema.entities.iter().enumerate() {
        if i > 0 {
            output.push('\n');
        }
        serialize_entity(&mut output, entity);
    }

    // Serialize relationships
    if !schema.relationships.is_empty() {
        output.push_str("\nrel {\n");
        for rel in &schema.relationships {
            serialize_relationship(&mut output, rel);
        }
        output.push_str("}\n");
    }

    // Generate arrangement based on FK dependencies
    let arrangement = generate_arrangement(schema);
    if !arrangement.is_empty() {
        output.push_str("\n@hint.arrangement = {\n");
        for row in &arrangement {
            output.push_str("    ");
            output.push_str(&row.join(" "));
            output.push_str("\n");
        }
        output.push_str("}\n");
    }

    output
}

/// Generate arrangement based on FK dependencies.
/// Parent tables (referenced by FK) go to upper levels.
fn generate_arrangement(schema: &Schema) -> Vec<Vec<String>> {
    if schema.entities.is_empty() {
        return vec![];
    }

    let entity_names: HashSet<&str> = schema.entities.iter().map(|e| e.name.as_str()).collect();

    // Build dependency graph: child -> parents (FK targets)
    let mut parents: HashMap<&str, HashSet<&str>> = HashMap::new();
    for entity in &schema.entities {
        parents.insert(entity.name.as_str(), HashSet::new());
    }

    for rel in &schema.relationships {
        // rel.left is parent (1 side), rel.right is child (* side)
        if entity_names.contains(rel.left.as_str()) && entity_names.contains(rel.right.as_str()) {
            if let Some(deps) = parents.get_mut(rel.right.as_str()) {
                deps.insert(rel.left.as_str());
            }
        }
    }

    // Calculate levels using topological sort
    // Level 0 = no dependencies (root tables)
    let mut levels: HashMap<&str, usize> = HashMap::new();
    let mut changed = true;

    // Initialize: tables with no parents get level 0
    for (entity, deps) in &parents {
        if deps.is_empty() {
            levels.insert(entity, 0);
        }
    }

    // Propagate levels
    while changed {
        changed = false;
        for (entity, deps) in &parents {
            if levels.contains_key(entity) {
                continue;
            }
            // Check if all parents have levels assigned
            let parent_levels: Vec<usize> = deps
                .iter()
                .filter_map(|p| levels.get(p).copied())
                .collect();

            if parent_levels.len() == deps.len() {
                // All parents have levels, this entity's level = max(parent_levels) + 1
                let level = parent_levels.iter().max().copied().unwrap_or(0) + 1;
                levels.insert(entity, level);
                changed = true;
            }
        }
    }

    // Handle circular dependencies: assign remaining to max_level + 1
    let max_level = levels.values().copied().max().unwrap_or(0);
    for entity in entity_names.iter() {
        if !levels.contains_key(entity) {
            levels.insert(entity, max_level + 1);
        }
    }

    // Group by level
    let final_max_level = levels.values().copied().max().unwrap_or(0);
    let mut rows: Vec<Vec<String>> = vec![vec![]; final_max_level + 1];

    for (entity, level) in &levels {
        rows[*level].push(entity.to_string());
    }

    // Sort entities within each row alphabetically for consistency
    for row in &mut rows {
        row.sort();
    }

    // Remove empty rows
    rows.into_iter().filter(|r| !r.is_empty()).collect()
}

fn serialize_entity(output: &mut String, entity: &Entity) {
    output.push_str(&format!("entity {} {{\n", entity.name));

    // Collect PKs from constraints for composite key handling
    let composite_pk_columns: Vec<&str> = entity
        .constraints
        .iter()
        .filter_map(|c| match c {
            Constraint::PrimaryKey(cols) if cols.len() > 1 => Some(cols.iter().map(|s| s.as_str()).collect::<Vec<_>>()),
            _ => None,
        })
        .flatten()
        .collect();

    for column in &entity.columns {
        serialize_column(output, column, &composite_pk_columns);
    }

    // Serialize block-level constraints
    for constraint in &entity.constraints {
        serialize_constraint(output, constraint);
    }

    output.push_str("}\n");
}

fn serialize_column(output: &mut String, column: &Column, composite_pk_columns: &[&str]) {
    output.push_str(&format!("    {} {}", column.name, column.typ));

    // Check if this column is part of a composite PK (don't add pk modifier)
    let is_composite_pk_member = composite_pk_columns.contains(&column.name.as_str());

    // Serialize modifiers in order: pk, unique, not null, fk, default
    let has_pk = column.modifiers.iter().any(|m| matches!(m, ColumnModifier::Pk));
    let has_unique = column.modifiers.iter().any(|m| matches!(m, ColumnModifier::Unique));
    let has_not_null = column.modifiers.iter().any(|m| matches!(m, ColumnModifier::NotNull));

    if has_pk && !is_composite_pk_member {
        output.push_str(" pk");
    }
    if has_unique {
        output.push_str(" unique");
    }
    if has_not_null {
        output.push_str(" not null");
    }

    // FK modifier
    for modifier in &column.modifiers {
        if let ColumnModifier::Fk { target, column: col } = modifier {
            output.push_str(&format!(" fk -> {}.{}", target, col));
        }
    }

    // Default value
    for modifier in &column.modifiers {
        if let ColumnModifier::Default(val) = modifier {
            // Check if it's a function call (e.g., NOW())
            let is_function_call = val.contains('(') && val.ends_with(')');
            // Quote the value if it contains special characters but is not a function call
            let needs_quote = !is_function_call
                && (val.contains(' ') || val.starts_with('\''));
            if needs_quote {
                output.push_str(&format!(" default \"{}\"", val));
            } else {
                output.push_str(&format!(" default {}", val));
            }
        }
    }

    output.push('\n');
}

fn serialize_constraint(output: &mut String, constraint: &Constraint) {
    match constraint {
        Constraint::PrimaryKey(cols) if cols.len() > 1 => {
            output.push_str(&format!("    primary_key({})\n", cols.join(", ")));
        }
        Constraint::PrimaryKey(_) => {
            // Single-column PK is handled at column level
        }
        Constraint::ForeignKey {
            columns,
            target,
            target_columns,
            on_delete,
            on_update,
        } => {
            output.push_str(&format!(
                "    foreign_key({}) references {}({})",
                columns.join(", "),
                target,
                target_columns.join(", ")
            ));
            if let Some(action) = on_delete {
                output.push_str(&format!(" on delete {}", action));
            }
            if let Some(action) = on_update {
                output.push_str(&format!(" on update {}", action));
            }
            output.push('\n');
        }
        Constraint::Index { columns, name } => {
            output.push_str(&format!("    index({})", columns.join(", ")));
            if let Some(n) = name {
                output.push_str(&format!(" name = {}", n));
            }
            output.push('\n');
        }
    }
}

fn serialize_relationship(output: &mut String, rel: &Relationship) {
    let left_card = serialize_cardinality(&rel.left_cardinality);
    let right_card = serialize_cardinality(&rel.right_cardinality);

    output.push_str(&format!(
        "    {} {} -- {} {}",
        rel.left, left_card, right_card, rel.right
    ));

    if let Some(label) = &rel.label {
        output.push_str(&format!(" : \"{}\"", label));
    }

    if let Some(role) = &rel.role {
        output.push_str(&format!(" as {}", role));
    }

    output.push('\n');
}

fn serialize_cardinality(card: &Cardinality) -> &'static str {
    match card {
        Cardinality::One => "1",
        Cardinality::ZeroOrOne => "0..1",
        Cardinality::Many => "*",
        Cardinality::OneOrMore => "1..*",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Cardinality, Column, ColumnModifier, Entity, Relationship, Schema};

    #[test]
    fn test_serialize_simple_entity() {
        let schema = Schema {
            entities: vec![Entity {
                name: "User".to_string(),
                columns: vec![
                    Column {
                        name: "id".to_string(),
                        typ: "int".to_string(),
                        modifiers: vec![ColumnModifier::Pk],
                    },
                    Column {
                        name: "email".to_string(),
                        typ: "string".to_string(),
                        modifiers: vec![ColumnModifier::NotNull, ColumnModifier::Unique],
                    },
                ],
                constraints: vec![],
                hints: vec![],
            }],
            relationships: vec![],
            views: vec![],
            arrangement: None,
        };

        let result = serialize(&schema);
        assert!(result.contains("entity User {"));
        assert!(result.contains("id int pk"));
        assert!(result.contains("email string unique not null"));
    }

    #[test]
    fn test_serialize_with_fk() {
        let schema = Schema {
            entities: vec![Entity {
                name: "Order".to_string(),
                columns: vec![
                    Column {
                        name: "id".to_string(),
                        typ: "int".to_string(),
                        modifiers: vec![ColumnModifier::Pk],
                    },
                    Column {
                        name: "user_id".to_string(),
                        typ: "int".to_string(),
                        modifiers: vec![
                            ColumnModifier::NotNull,
                            ColumnModifier::Fk {
                                target: "User".to_string(),
                                column: "id".to_string(),
                            },
                        ],
                    },
                ],
                constraints: vec![],
                hints: vec![],
            }],
            relationships: vec![],
            views: vec![],
            arrangement: None,
        };

        let result = serialize(&schema);
        assert!(result.contains("user_id int not null fk -> User.id"));
    }

    #[test]
    fn test_serialize_relationship() {
        let schema = Schema {
            entities: vec![],
            relationships: vec![Relationship {
                left: "User".to_string(),
                left_cardinality: Cardinality::One,
                right: "Order".to_string(),
                right_cardinality: Cardinality::Many,
                label: Some("places".to_string()),
                role: None,
            }],
            views: vec![],
            arrangement: None,
        };

        let result = serialize(&schema);
        assert!(result.contains("rel {"));
        assert!(result.contains("User 1 -- * Order : \"places\""));
    }
}
