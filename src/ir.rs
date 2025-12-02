use crate::ast::{Cardinality, ColumnModifier, Schema};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailLevel {
    Tables,
    Pk,
    PkFk,
    All,
}

impl DetailLevel {
    pub fn from_str(s: &str) -> Option<Self> {
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
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub columns: Vec<ColumnIR>,
    pub level: Option<i64>,
    pub group: Option<String>,
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
    pub fn from_schema(schema: &Schema, view: Option<&str>, detail: DetailLevel) -> Self {
        let included_entities: Vec<&str> = match view {
            Some(view_name) => schema
                .views
                .iter()
                .find(|v| v.name == view_name)
                .map(|v| v.includes.iter().map(|s| s.as_str()).collect())
                .unwrap_or_default(),
            None => schema.entities.iter().map(|e| e.name.as_str()).collect(),
        };

        let nodes: Vec<Node> = schema
            .entities
            .iter()
            .filter(|e| included_entities.contains(&e.name.as_str()))
            .map(|e| {
                let columns: Vec<ColumnIR> = e
                    .columns
                    .iter()
                    .filter_map(|c| {
                        let is_pk = c.modifiers.iter().any(|m| matches!(m, ColumnModifier::Pk));
                        let is_fk = c.modifiers.iter().any(|m| matches!(m, ColumnModifier::Fk { .. }));

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
                    if h.key == "hint.level" {
                        if let crate::ast::HintValue::Int(n) = h.value {
                            return Some(n);
                        }
                    }
                    None
                });

                let group = e.hints.iter().find_map(|h| {
                    if h.key == "hint.group" {
                        match &h.value {
                            crate::ast::HintValue::Str(s) => return Some(s.clone()),
                            crate::ast::HintValue::Ident(s) => return Some(s.clone()),
                            _ => {}
                        }
                    }
                    None
                });

                Node {
                    id: e.name.clone(),
                    label: e.name.clone(),
                    columns,
                    level,
                    group,
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

        GraphIR { nodes, edges }
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
    fn test_ir_with_view() {
        let input = r#"
            entity User { id int pk }
            entity Order { id int pk }
            entity Product { id int pk }

            view core {
                include User, Order
            }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();
        let ir = GraphIR::from_schema(&schema, Some("core"), DetailLevel::All);

        assert_eq!(ir.nodes.len(), 2);
    }
}
