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
    pub waypoints: Vec<(f64, f64)>, // Orthogonal path points (start, turns, end)
    pub is_self_ref: bool,
    pub edge_index: usize, // Index into GraphIR.edges
}

#[derive(Debug, Clone)]
pub struct Layout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub width: f64,
    pub height: f64,
    pub channel_gap: f64,      // Gap for routing channels between levels
    pub corner_radius: f64,    // Radius for rounded corners
}

pub struct LayoutEngine {
    metrics: TextMetrics,
    node_gap_x: f64,
    node_gap_y: f64,
    channel_gap: f64,    // Space reserved for routing channels between levels
    lane_spacing: f64,   // Spacing between parallel edges in same channel
    corner_radius: f64,  // Radius for rounded corners
    entity_margin: f64,  // Minimum distance from entity edge to routing channel
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self {
            metrics: TextMetrics::default(),
            node_gap_x: 100.0,
            node_gap_y: 30.0,     // Base vertical gap between levels
            channel_gap: 50.0,    // Base space for routing channels (will expand with edge count)
            lane_spacing: 20.0,   // Spacing between parallel edges
            corner_radius: 8.0,   // Rounded corner radius
            entity_margin: 30.0,  // Minimum distance from entity edge to channel
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

        // Create node -> level lookup
        let node_level: HashMap<&str, i64> = ir
            .nodes
            .iter()
            .map(|n| (n.id.as_str(), n.level.unwrap_or(0)))
            .collect();

        // Count edges per channel (between adjacent levels)
        let mut channel_edge_count: HashMap<i64, usize> = HashMap::new();
        for edge in &ir.edges {
            if edge.from == edge.to {
                continue; // Skip self-ref
            }
            let from_level = *node_level.get(edge.from.as_str()).unwrap_or(&0);
            let to_level = *node_level.get(edge.to.as_str()).unwrap_or(&0);
            if from_level != to_level {
                let channel_level = from_level.min(to_level);
                *channel_edge_count.entry(channel_level).or_insert(0) += 1;
            }
        }

        // Calculate dynamic channel gaps based on edge count
        let mut dynamic_channel_gap: HashMap<i64, f64> = HashMap::new();
        for (i, &level) in level_keys.iter().enumerate() {
            if i < level_keys.len() - 1 {
                let edge_count = *channel_edge_count.get(&level).unwrap_or(&0);
                // Base gap + extra space for edges (centered around channel)
                let needed_space = self.entity_margin * 2.0
                    + (edge_count.saturating_sub(1) as f64) * self.lane_spacing;
                let gap = needed_space.max(self.channel_gap);
                dynamic_channel_gap.insert(level, gap);
            }
        }

        // Place nodes with dynamic channel gaps between levels
        let mut layout_nodes = Vec::new();
        let mut level_bottom_y: HashMap<i64, f64> = HashMap::new();
        let mut channel_y: HashMap<i64, f64> = HashMap::new();
        let mut y: f64 = 40.0;
        let mut max_width: f64 = 0.0;

        for (i, &level) in level_keys.iter().enumerate() {
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
            level_bottom_y.insert(level, y + max_height);

            // Add dynamic channel gap after this level (except for last level)
            if i < level_keys.len() - 1 {
                let gap = *dynamic_channel_gap.get(&level).unwrap_or(&self.channel_gap);
                // Center channel in the middle of the total space between levels
                let total_space = self.node_gap_y + gap;
                let channel_center = y + max_height + total_space / 2.0;
                channel_y.insert(level, channel_center);
                y += max_height + total_space;
            } else {
                y += max_height + self.node_gap_y;
            }
        }

        let total_height = y - self.node_gap_y + 40.0;

        // Create node position lookup
        let node_positions: HashMap<&str, &LayoutNode> = layout_nodes
            .iter()
            .map(|n| (n.id.as_str(), n))
            .collect();

        // Group edges by source node and direction (down=true, up=false)
        // to distribute exit points along the node edge
        let mut node_exits: HashMap<(&str, bool), Vec<(usize, f64)>> = HashMap::new(); // (edge_idx, target_x)

        for (idx, edge) in ir.edges.iter().enumerate() {
            if edge.from == edge.to {
                continue; // Skip self-ref
            }
            let from_node = match node_positions.get(edge.from.as_str()) {
                Some(n) => n,
                None => continue,
            };
            let to_node = match node_positions.get(edge.to.as_str()) {
                Some(n) => n,
                None => continue,
            };

            let from_level = *node_level.get(edge.from.as_str()).unwrap_or(&0);
            let to_level = *node_level.get(edge.to.as_str()).unwrap_or(&0);
            let going_down = to_level >= from_level;
            let to_cx = to_node.x + to_node.width / 2.0;

            node_exits
                .entry((edge.from.as_str(), going_down))
                .or_default()
                .push((idx, to_cx));

            // Also track entry points on destination node
            let from_cx = from_node.x + from_node.width / 2.0;
            node_exits
                .entry((edge.to.as_str(), !going_down))
                .or_default()
                .push((idx, from_cx));
        }

        // Sort edges in each group by target x position
        for edges in node_exits.values_mut() {
            edges.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        }

        // Pre-calculate lane assignments by sorting edges within each channel
        // Edges starting from left should get earlier lanes (turn higher) to avoid crossings
        let mut edge_lane_assignments: HashMap<usize, usize> = HashMap::new();
        {
            // Collect edges with their channel info and starting x position
            let mut channel_edges: HashMap<i64, Vec<(usize, f64)>> = HashMap::new(); // channel -> [(edge_idx, from_cx)]

            for (idx, edge) in ir.edges.iter().enumerate() {
                if edge.from == edge.to {
                    continue;
                }
                let from_node = match node_positions.get(edge.from.as_str()) {
                    Some(n) => n,
                    None => continue,
                };
                if node_positions.get(edge.to.as_str()).is_none() {
                    continue;
                }

                let from_level = *node_level.get(edge.from.as_str()).unwrap_or(&0);
                let to_level = *node_level.get(edge.to.as_str()).unwrap_or(&0);

                if from_level != to_level {
                    let channel_level = from_level.min(to_level);
                    let going_down = to_level >= from_level;

                    // Get the actual from_cx for this edge
                    let from_exits = node_exits.get(&(edge.from.as_str(), going_down));
                    let from_cx = if let Some(exits) = from_exits {
                        let pos = exits.iter().position(|(i, _)| *i == idx).unwrap_or(0);
                        self.distribute_anchor(from_node, pos, exits.len())
                    } else {
                        from_node.x + from_node.width / 2.0
                    };

                    channel_edges
                        .entry(channel_level)
                        .or_default()
                        .push((idx, from_cx));
                }
            }

            // Sort edges in each channel by from_cx DESCENDING (rightmost first)
            // This prevents crossings: edges starting from right turn first (upper lane),
            // so their horizontal segment doesn't block edges starting from left
            for (_channel, edges) in channel_edges.iter_mut() {
                edges.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                for (lane, (edge_idx, _)) in edges.iter().enumerate() {
                    edge_lane_assignments.insert(*edge_idx, lane);
                }
            }
        }

        // Calculate orthogonal paths for each edge
        let layout_edges: Vec<LayoutEdge> = ir
            .edges
            .iter()
            .enumerate()
            .filter_map(|(idx, edge)| {
                let from_node = node_positions.get(edge.from.as_str())?;
                let to_node = node_positions.get(edge.to.as_str())?;

                let is_self_ref = edge.from == edge.to;

                if is_self_ref {
                    // Self-referential edge: orthogonal loop on right side
                    let x = from_node.x + from_node.width;
                    let y_top = from_node.y + from_node.height * 0.3;
                    let y_bottom = from_node.y + from_node.height * 0.7;
                    let loop_offset = 25.0;

                    let waypoints = vec![
                        (x, y_top),
                        (x + loop_offset, y_top),
                        (x + loop_offset, y_bottom),
                        (x, y_bottom),
                    ];

                    Some(LayoutEdge {
                        from: edge.from.clone(),
                        to: edge.to.clone(),
                        waypoints,
                        is_self_ref: true,
                        edge_index: idx,
                    })
                } else {
                    let from_level = *node_level.get(edge.from.as_str()).unwrap_or(&0);
                    let to_level = *node_level.get(edge.to.as_str()).unwrap_or(&0);
                    let going_down = to_level >= from_level;

                    // Get distributed exit point on source node
                    let from_exits = node_exits.get(&(edge.from.as_str(), going_down))?;
                    let from_pos = from_exits.iter().position(|(i, _)| *i == idx).unwrap_or(0);
                    let from_total = from_exits.len();
                    let from_cx = self.distribute_anchor(from_node, from_pos, from_total);

                    // Get distributed entry point on target node
                    let to_exits = node_exits.get(&(edge.to.as_str(), !going_down))?;
                    let to_pos = to_exits.iter().position(|(i, _)| *i == idx).unwrap_or(0);
                    let to_total = to_exits.len();
                    let to_cx = self.distribute_anchor(to_node, to_pos, to_total);

                    // Determine lane offset using pre-calculated assignment
                    let channel_level = from_level.min(to_level);
                    let total_edges = *channel_edge_count.get(&channel_level).unwrap_or(&1);
                    let lane = *edge_lane_assignments.get(&idx).unwrap_or(&0);
                    // Center: lane 0 at -(total-1)/2 * spacing, lane n at (n - (total-1)/2) * spacing
                    let lane_offset = (lane as f64 - (total_edges - 1) as f64 / 2.0) * self.lane_spacing;

                    let waypoints = if from_level == to_level {
                        // Same level: check if entities are horizontally adjacent
                        let (left_node, right_node) = if from_node.x < to_node.x {
                            (from_node, to_node)
                        } else {
                            (to_node, from_node)
                        };

                        let gap_between = right_node.x - (left_node.x + left_node.width);

                        if gap_between <= self.node_gap_x * 1.5 {
                            // Adjacent entities: route directly between them via sides
                            let mid_x = left_node.x + left_node.width + gap_between / 2.0;
                            let from_y = from_node.y + from_node.height / 2.0;
                            let to_y = to_node.y + to_node.height / 2.0;

                            if from_node.x < to_node.x {
                                // from is left, to is right
                                vec![
                                    (from_node.x + from_node.width, from_y),
                                    (mid_x, from_y),
                                    (mid_x, to_y),
                                    (to_node.x, to_y),
                                ]
                            } else {
                                // from is right, to is left
                                vec![
                                    (from_node.x, from_y),
                                    (mid_x, from_y),
                                    (mid_x, to_y),
                                    (to_node.x + to_node.width, to_y),
                                ]
                            }
                        } else {
                            // Non-adjacent: route below the level
                            let max_bottom = from_node.y.max(to_node.y)
                                + from_node.height.max(to_node.height);
                            let ch_y = max_bottom + self.entity_margin + lane_offset.abs();

                            vec![
                                (from_cx, from_node.y + from_node.height),
                                (from_cx, ch_y),
                                (to_cx, ch_y),
                                (to_cx, to_node.y + to_node.height),
                            ]
                        }
                    } else {
                        // Different levels: route through channel between levels
                        let (upper_node, lower_node, upper_cx, lower_cx) = if from_level < to_level {
                            (from_node, to_node, from_cx, to_cx)
                        } else {
                            (to_node, from_node, to_cx, from_cx)
                        };

                        let ch_y = *channel_y.get(&from_level.min(to_level))
                            .unwrap_or(&(upper_node.y + upper_node.height + self.channel_gap / 2.0))
                            + lane_offset;

                        if from_level < to_level {
                            vec![
                                (upper_cx, upper_node.y + upper_node.height),
                                (upper_cx, ch_y),
                                (lower_cx, ch_y),
                                (lower_cx, lower_node.y),
                            ]
                        } else {
                            vec![
                                (lower_cx, lower_node.y + lower_node.height),
                                (lower_cx, ch_y),
                                (upper_cx, ch_y),
                                (upper_cx, upper_node.y),
                            ]
                        }
                    };

                    Some(LayoutEdge {
                        from: edge.from.clone(),
                        to: edge.to.clone(),
                        waypoints,
                        is_self_ref: false,
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
            channel_gap: self.channel_gap,
            corner_radius: self.corner_radius,
        }
    }

    /// Distribute anchor points along a node's horizontal edge
    fn distribute_anchor(&self, node: &LayoutNode, position: usize, total: usize) -> f64 {
        let cx = node.x + node.width / 2.0;
        if total <= 1 {
            cx
        } else {
            let spacing = 24.0; // spacing between anchors
            let offset = (position as f64 - (total - 1) as f64 / 2.0) * spacing;
            cx + offset
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
