use crate::ast::{Cardinality, ColumnModifier, Constraint, Schema};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailLevel {
    Tables,
    Pk,
    PkFk,
    All,
}

impl DetailLevel {
    /// The name the CLI and the options object use for each level.
    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "tables" => Some(Self::Tables),
            "pk" => Some(Self::Pk),
            "pk_fk" => Some(Self::PkFk),
            "all" => Some(Self::All),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GraphIR {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    /// Sets of entities the source asked to keep near one another
    pub near: Vec<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub columns: Vec<ColumnIR>,
    /// Row the source pinned this entity to, if it pinned one
    pub level: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ColumnIR {
    pub name: String,
    pub typ: String,
    pub is_pk: bool,
    pub is_fk: bool,
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub from_cardinality: Cardinality,
    pub to_cardinality: Cardinality,
    pub label: Option<String>,
    pub role: Option<String>,
}

impl GraphIR {
    pub fn from_schema(schema: &Schema, focus: Option<&str>, detail: DetailLevel) -> Self {
        let included_entities: Vec<&str> = match focus.and_then(|name| schema.find_focus(name)) {
            Some(focus) => focus.includes.iter().map(|s| s.as_str()).collect(),
            None => schema.entities.iter().map(|e| e.name.as_str()).collect(),
        };

        let nodes: Vec<Node> = schema
            .entities
            .iter()
            .filter(|e| included_entities.contains(&e.name.as_str()))
            .filter(|e| !schema.omit.contains(&e.name))
            .map(|e| {
                // A composite key is declared next to the columns, not on them.
                let composite_pk: Vec<&str> = e
                    .constraints
                    .iter()
                    .filter_map(|c| match c {
                        Constraint::PrimaryKey(columns) => Some(columns),
                        _ => None,
                    })
                    .flatten()
                    .map(|name| name.as_str())
                    .collect();

                // Named as brief: the entity is there, what is in it is not.
                let detail = if schema.brief.contains(&e.name) {
                    DetailLevel::Tables
                } else {
                    detail
                };

                let columns: Vec<ColumnIR> = e
                    .columns
                    .iter()
                    .filter_map(|c| {
                        let is_pk = c.modifiers.iter().any(|m| matches!(m, ColumnModifier::Pk))
                            || composite_pk.contains(&c.name.as_str());
                        let is_fk = c
                            .modifiers
                            .iter()
                            .any(|m| matches!(m, ColumnModifier::Fk { .. }));

                        let include = match detail {
                            DetailLevel::Tables => false,
                            DetailLevel::Pk => is_pk,
                            DetailLevel::PkFk => is_pk || is_fk,
                            DetailLevel::All => true,
                        };

                        if include {
                            Some(ColumnIR {
                                name: c.name.clone(),
                                typ: c.typ.clone(),
                                is_pk,
                                is_fk,
                            })
                        } else {
                            None
                        }
                    })
                    .collect();

                let level = e.hints.iter().find_map(|h| {
                    if h.key == "hint.level"
                        && let crate::ast::HintValue::Int(n) = h.value
                    {
                        return Some(n);
                    }
                    None
                });

                Node {
                    id: e.name.clone(),
                    label: e.name.clone(),
                    columns,
                    level,
                }
            })
            .collect();

        let node_ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();

        let edges: Vec<Edge> = schema
            .relationships
            .iter()
            .filter(|r| node_ids.contains(&r.left.as_str()) && node_ids.contains(&r.right.as_str()))
            .map(|r| Edge {
                from: r.left.clone(),
                to: r.right.clone(),
                from_cardinality: r.left_cardinality,
                to_cardinality: r.right_cardinality,
                label: r.label.clone(),
                role: r.role.clone(),
            })
            .collect();

        // Only what is actually drawn can be kept near anything.
        let near: Vec<Vec<String>> = schema
            .near
            .iter()
            .map(|set| {
                set.iter()
                    .filter(|name| node_ids.contains(&name.as_str()))
                    .cloned()
                    .collect::<Vec<String>>()
            })
            .filter(|set| set.len() > 1)
            .collect();

        GraphIR { nodes, edges, near }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;

    #[test]
    fn test_ir_all_detail() {
        let input = r#"
            entity User {
                id int pk
                name string
                email string
            }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();
        let ir = GraphIR::from_schema(&schema, None, DetailLevel::All);

        assert_eq!(ir.nodes.len(), 1);
        assert_eq!(ir.nodes[0].columns.len(), 3);
    }

    #[test]
    fn test_ir_pk_detail() {
        let input = r#"
            entity User {
                id int pk
                name string
                email string
            }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();
        let ir = GraphIR::from_schema(&schema, None, DetailLevel::Pk);

        assert_eq!(ir.nodes[0].columns.len(), 1);
        assert_eq!(ir.nodes[0].columns[0].name, "id");
    }

    #[test]
    fn test_ir_tables_detail() {
        let input = r#"
            entity User {
                id int pk
                name string
            }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();
        let ir = GraphIR::from_schema(&schema, None, DetailLevel::Tables);

        assert_eq!(ir.nodes[0].columns.len(), 0);
    }

    #[test]
    fn test_ir_with_focus() {
        let input = r#"
            entity User { id int pk }
            entity Order { id int pk }
            entity Product { id int pk }

            focus core {
                include User, Order
            }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();
        let ir = GraphIR::from_schema(&schema, Some("core"), DetailLevel::All);

        assert_eq!(ir.nodes.len(), 2);
    }

    #[test]
    fn leaves_out_what_the_source_asked_to_omit() {
        let input = r#"
            @hint.omit = { migrations }
            entity migrations { id int pk }
            entity User { id int pk }
            rel { User 1 -- * migrations }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();
        let ir = GraphIR::from_schema(&schema, None, DetailLevel::All);

        assert_eq!(ir.nodes.len(), 1, "the omitted entity is still drawn");
        assert!(ir.edges.is_empty(), "its relationships are still drawn");
    }

    #[test]
    fn draws_a_brief_entity_as_a_name_alone() {
        let input = r#"
            @hint.brief = { audit_logs }
            entity audit_logs {
                id int pk
                event string
            }
            entity User { id int pk }
            rel { User 1 -- * audit_logs }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();
        let ir = GraphIR::from_schema(&schema, None, DetailLevel::All);

        let brief = ir.nodes.iter().find(|n| n.id == "audit_logs").unwrap();
        let user = ir.nodes.iter().find(|n| n.id == "User").unwrap();
        assert!(brief.columns.is_empty(), "brief entity kept its columns");
        assert_eq!(user.columns.len(), 1, "every entity lost its columns");
        assert_eq!(ir.edges.len(), 1, "the relationship went with the columns");
    }

    #[test]
    fn carries_only_the_near_sets_it_can_draw() {
        let input = r#"
            @hint.near = { User, Order }
            @hint.near = { User, gone }
            entity User { id int pk }
            entity Order { id int pk }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();
        let ir = GraphIR::from_schema(&schema, None, DetailLevel::All);

        assert_eq!(ir.near, vec![vec!["User", "Order"]]);
    }
}
