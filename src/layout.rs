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
    pub is_self_ref: bool,
    pub control_points: Option<[(f64, f64); 2]>, // For self-referential curves
    pub edge_index: usize, // Index into GraphIR.edges
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

        // Determine which side each edge connects to for each node
        // Side: 0=top, 1=right, 2=bottom, 3=left
        let mut node_side_edges: HashMap<(&str, u8), Vec<usize>> = HashMap::new();

        for (idx, edge) in ir.edges.iter().enumerate() {
            if edge.from == edge.to {
                continue; // Skip self-ref for now
            }
            let from_node = match node_positions.get(edge.from.as_str()) {
                Some(n) => n,
                None => continue,
            };
            let to_node = match node_positions.get(edge.to.as_str()) {
                Some(n) => n,
                None => continue,
            };

            let from_side = self.get_edge_side(from_node, to_node);
            let to_side = self.get_edge_side(to_node, from_node);

            node_side_edges
                .entry((edge.from.as_str(), from_side))
                .or_default()
                .push(idx);
            node_side_edges
                .entry((edge.to.as_str(), to_side))
                .or_default()
                .push(idx);
        }

        // Sort edges on each side by direction to avoid crossings
        for ((node_id, side), edge_indices) in node_side_edges.iter_mut() {
            if node_positions.get(*node_id).is_none() {
                continue;
            }

            edge_indices.sort_by(|&a, &b| {
                let edge_a = &ir.edges[a];
                let edge_b = &ir.edges[b];

                // Get the "other" node for each edge
                let other_a = if edge_a.from == *node_id {
                    node_positions.get(edge_a.to.as_str())
                } else {
                    node_positions.get(edge_a.from.as_str())
                };
                let other_b = if edge_b.from == *node_id {
                    node_positions.get(edge_b.to.as_str())
                } else {
                    node_positions.get(edge_b.from.as_str())
                };

                let (other_a, other_b) = match (other_a, other_b) {
                    (Some(a), Some(b)) => (a, b),
                    _ => return std::cmp::Ordering::Equal,
                };

                let a_cx = other_a.x + other_a.width / 2.0;
                let a_cy = other_a.y + other_a.height / 2.0;
                let b_cx = other_b.x + other_b.width / 2.0;
                let b_cy = other_b.y + other_b.height / 2.0;

                // Sort based on side to avoid crossings
                match side {
                    0 | 2 => {
                        // Top/Bottom: sort by x (left to right)
                        a_cx.partial_cmp(&b_cx).unwrap_or(std::cmp::Ordering::Equal)
                    }
                    1 | 3 => {
                        // Right/Left: sort by y (top to bottom)
                        a_cy.partial_cmp(&b_cy).unwrap_or(std::cmp::Ordering::Equal)
                    }
                    _ => std::cmp::Ordering::Equal,
                }
            });
        }

        // Calculate anchor positions for each edge
        let anchor_spacing = 48.0; // ~4em for clear cardinality label separation

        let layout_edges: Vec<LayoutEdge> = ir
            .edges
            .iter()
            .enumerate()
            .filter_map(|(idx, edge)| {
                let from_node = node_positions.get(edge.from.as_str())?;
                let to_node = node_positions.get(edge.to.as_str())?;

                let is_self_ref = edge.from == edge.to;

                if is_self_ref {
                    // Self-referential edge: loop on right side
                    let x = from_node.x + from_node.width;
                    let y_top = from_node.y + from_node.height * 0.25;
                    let y_bottom = from_node.y + from_node.height * 0.75;
                    let loop_size = 30.0;

                    Some(LayoutEdge {
                        from: edge.from.clone(),
                        to: edge.to.clone(),
                        from_point: (x, y_top),
                        to_point: (x, y_bottom),
                        is_self_ref: true,
                        control_points: Some([
                            (x + loop_size, y_top - loop_size * 0.5),
                            (x + loop_size, y_bottom + loop_size * 0.5),
                        ]),
                        edge_index: idx,
                    })
                } else {
                    let from_side = self.get_edge_side(from_node, to_node);
                    let to_side = self.get_edge_side(to_node, from_node);

                    let from_edges = node_side_edges.get(&(edge.from.as_str(), from_side))?;
                    let to_edges = node_side_edges.get(&(edge.to.as_str(), to_side))?;

                    let from_pos = from_edges.iter().position(|&i| i == idx).unwrap_or(0);
                    let to_pos = to_edges.iter().position(|&i| i == idx).unwrap_or(0);

                    let from_point = self.anchor_on_side(
                        from_node,
                        from_side,
                        from_pos,
                        from_edges.len(),
                        anchor_spacing,
                    );
                    let to_point = self.anchor_on_side(
                        to_node,
                        to_side,
                        to_pos,
                        to_edges.len(),
                        anchor_spacing,
                    );

                    Some(LayoutEdge {
                        from: edge.from.clone(),
                        to: edge.to.clone(),
                        from_point,
                        to_point,
                        is_self_ref: false,
                        control_points: None,
                        edge_index: idx,
                    })
                }
            })
            .collect();

        Layout {
            nodes: layout_nodes,
            edges: layout_edges,
            width: max_width,
            height: total_height,
        }
    }

    /// Determine which side of 'from' node the edge should connect to reach 'to' node
    /// Returns: 0=top, 1=right, 2=bottom, 3=left
    fn get_edge_side(&self, from: &LayoutNode, to: &LayoutNode) -> u8 {
        let from_cx = from.x + from.width / 2.0;
        let from_cy = from.y + from.height / 2.0;
        let to_cx = to.x + to.width / 2.0;
        let to_cy = to.y + to.height / 2.0;

        let dx = to_cx - from_cx;
        let dy = to_cy - from_cy;

        // Favor vertical (top/bottom) when angle is close to diagonal
        // This reduces horizontal edge crossings in hierarchical layouts
        if dx.abs() > dy.abs() * 1.3 {
            if dx > 0.0 { 1 } else { 3 } // right or left
        } else {
            if dy > 0.0 { 2 } else { 0 } // bottom or top
        }
    }

    /// Calculate anchor point on a specific side of a node
    /// Distributes multiple anchors evenly along the side
    fn anchor_on_side(
        &self,
        node: &LayoutNode,
        side: u8,
        position: usize,
        total: usize,
        spacing: f64,
    ) -> (f64, f64) {
        let offset = if total > 1 {
            (position as f64 - (total - 1) as f64 / 2.0) * spacing
        } else {
            0.0
        };

        let cx = node.x + node.width / 2.0;
        let cy = node.y + node.height / 2.0;

        match side {
            0 => (cx + offset, node.y),                        // top
            1 => (node.x + node.width, cy + offset),           // right
            2 => (cx + offset, node.y + node.height),          // bottom
            3 => (node.x, cy + offset),                        // left
            _ => (cx, cy),
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
