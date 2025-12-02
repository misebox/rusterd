use crate::ir::{GraphIR, Node};
use crate::measure::TextMetrics;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct LayoutNode {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone)]
pub struct LayoutEdge {
    pub from: String,
    pub to: String,
    pub from_point: (f64, f64),
    pub to_point: (f64, f64),
}

#[derive(Debug, Clone)]
pub struct Layout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub width: f64,
    pub height: f64,
}

pub struct LayoutEngine {
    metrics: TextMetrics,
    node_gap_x: f64,
    node_gap_y: f64,
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self {
            metrics: TextMetrics::default(),
            node_gap_x: 100.0,
            node_gap_y: 80.0,
        }
    }
}

impl LayoutEngine {
    pub fn layout(&self, ir: &GraphIR) -> Layout {
        let mut node_sizes: HashMap<String, (f64, f64)> = HashMap::new();

        for node in &ir.nodes {
            let columns: Vec<(String, String)> = node
                .columns
                .iter()
                .map(|c| (c.name.clone(), c.typ.clone()))
                .collect();
            let size = self.metrics.node_size(&node.label, &columns);
            node_sizes.insert(node.id.clone(), size);
        }

        // Group nodes by level
        let mut levels: HashMap<i64, Vec<&Node>> = HashMap::new();
        for node in &ir.nodes {
            let level = node.level.unwrap_or(0);
            levels.entry(level).or_default().push(node);
        }

        let mut level_keys: Vec<i64> = levels.keys().copied().collect();
        level_keys.sort();

        let mut layout_nodes = Vec::new();
        let mut y: f64 = 40.0;
        let mut max_width: f64 = 0.0;

        for level in level_keys {
            let nodes_in_level = &levels[&level];
            let mut x: f64 = 40.0;
            let mut max_height: f64 = 0.0;

            for node in nodes_in_level {
                let (w, h) = node_sizes[&node.id];
                layout_nodes.push(LayoutNode {
                    id: node.id.clone(),
                    x,
                    y,
                    width: w,
                    height: h,
                });
                x += w + self.node_gap_x;
                max_height = max_height.max(h);
            }

            max_width = max_width.max(x - self.node_gap_x + 40.0);
            y += max_height + self.node_gap_y;
        }

        let total_height = y - self.node_gap_y + 40.0;

        // Create node position lookup
        let node_positions: HashMap<&str, &LayoutNode> = layout_nodes
            .iter()
            .map(|n| (n.id.as_str(), n))
            .collect();

        // Layout edges
        let layout_edges: Vec<LayoutEdge> = ir
            .edges
            .iter()
            .filter_map(|edge| {
                let from_node = node_positions.get(edge.from.as_str())?;
                let to_node = node_positions.get(edge.to.as_str())?;

                let from_point = self.edge_anchor(from_node, to_node);
                let to_point = self.edge_anchor(to_node, from_node);

                Some(LayoutEdge {
                    from: edge.from.clone(),
                    to: edge.to.clone(),
                    from_point,
                    to_point,
                })
            })
            .collect();

        Layout {
            nodes: layout_nodes,
            edges: layout_edges,
            width: max_width,
            height: total_height,
        }
    }

    fn edge_anchor(&self, from: &LayoutNode, to: &LayoutNode) -> (f64, f64) {
        let from_cx = from.x + from.width / 2.0;
        let from_cy = from.y + from.height / 2.0;
        let to_cx = to.x + to.width / 2.0;
        let to_cy = to.y + to.height / 2.0;

        let dx = to_cx - from_cx;
        let dy = to_cy - from_cy;

        if dx.abs() > dy.abs() {
            // Horizontal connection
            if dx > 0.0 {
                (from.x + from.width, from_cy)
            } else {
                (from.x, from_cy)
            }
        } else {
            // Vertical connection
            if dy > 0.0 {
                (from_cx, from.y + from.height)
            } else {
                (from_cx, from.y)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::DetailLevel;
    use crate::parser::Parser;

    #[test]
    fn test_basic_layout() {
        let input = r#"
            entity User { id int pk }
            entity Order { id int pk }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();
        let ir = GraphIR::from_schema(&schema, None, DetailLevel::All);
        let engine = LayoutEngine::default();
        let layout = engine.layout(&ir);

        assert_eq!(layout.nodes.len(), 2);
        assert!(layout.width > 0.0);
        assert!(layout.height > 0.0);
    }

    #[test]
    fn test_layout_with_levels() {
        let input = r#"
            entity User {
                @hint.level = 0
                id int pk
            }
            entity Order {
                @hint.level = 1
                id int pk
            }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();
        let ir = GraphIR::from_schema(&schema, None, DetailLevel::All);
        let engine = LayoutEngine::default();
        let layout = engine.layout(&ir);

        let user = layout.nodes.iter().find(|n| n.id == "User").unwrap();
        let order = layout.nodes.iter().find(|n| n.id == "Order").unwrap();
        assert!(user.y < order.y);
    }

    #[test]
    fn test_layout_edges() {
        let input = r#"
            entity User { id int pk }
            entity Order { id int pk }
            rel { User 1 -- * Order }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();
        let ir = GraphIR::from_schema(&schema, None, DetailLevel::All);
        let engine = LayoutEngine::default();
        let layout = engine.layout(&ir);

        assert_eq!(layout.edges.len(), 1);
    }
}
