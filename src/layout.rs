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
    anchor_spacing: f64, // Spacing between edge anchors on entity edge
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
            lane_spacing: 24.0,   // Spacing between parallel edges (>= 1em)
            anchor_spacing: 40.0, // Spacing for cardinality labels (~4 chars at 15px font)
            corner_radius: 8.0,   // Rounded corner radius
            entity_margin: 30.0,  // Minimum distance from entity edge to channel
        }
    }
}

impl LayoutEngine {
    pub fn layout(&self, ir: &GraphIR) -> Layout {
        // Count edges per node per direction to calculate required anchor width
        // Key: (node_id, going_down), Value: edge count
        let mut edge_count_per_node: HashMap<(&str, bool), usize> = HashMap::new();

        // Build node level lookup first (needed to determine edge direction)
        let node_level_lookup: HashMap<&str, i64> = ir
            .nodes
            .iter()
            .map(|n| (n.id.as_str(), n.level.unwrap_or(0)))
            .collect();

        for edge in &ir.edges {
            if edge.from == edge.to {
                continue; // Skip self-ref
            }
            let from_level = *node_level_lookup.get(edge.from.as_str()).unwrap_or(&0);
            let to_level = *node_level_lookup.get(edge.to.as_str()).unwrap_or(&0);
            let going_down = to_level >= from_level;

            *edge_count_per_node
                .entry((edge.from.as_str(), going_down))
                .or_insert(0) += 1;
            *edge_count_per_node
                .entry((edge.to.as_str(), !going_down))
                .or_insert(0) += 1;
        }

        let mut node_sizes: HashMap<String, (f64, f64)> = HashMap::new();

        for node in &ir.nodes {
            let columns: Vec<(String, String)> = node
                .columns
                .iter()
                .map(|c| (c.name.clone(), c.typ.clone()))
                .collect();
            let (content_w, h) = self.metrics.node_size(&node.label, &columns);

            // Calculate minimum width needed for edge anchors
            let down_edges = *edge_count_per_node
                .get(&(node.id.as_str(), true))
                .unwrap_or(&0);
            let up_edges = *edge_count_per_node
                .get(&(node.id.as_str(), false))
                .unwrap_or(&0);
            let max_edges = down_edges.max(up_edges);

            // Required width for anchors: (n-1) * spacing + margin on each side
            let anchor_width = if max_edges > 1 {
                (max_edges - 1) as f64 * self.anchor_spacing + self.anchor_spacing
            } else {
                0.0
            };

            let w = content_w.max(anchor_width);
            node_sizes.insert(node.id.clone(), (w, h));
        }

        // Group nodes by level
        let mut levels: HashMap<i64, Vec<&Node>> = HashMap::new();
        for node in &ir.nodes {
            let level = node.level.unwrap_or(0);
            levels.entry(level).or_default().push(node);
        }

        // Sort nodes within each level by their order (from arrangement)
        // Nodes without order go to the end
        for nodes in levels.values_mut() {
            nodes.sort_by_key(|n| n.order.unwrap_or(i64::MAX));
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
        // This includes ALL edges that pass through each channel, not just edges
        // that start/end at adjacent levels
        // Key: channel_level, Value: list of edge indices that pass through this channel
        let mut channel_edges_list: HashMap<i64, Vec<usize>> = HashMap::new();

        for (idx, edge) in ir.edges.iter().enumerate() {
            if edge.from == edge.to {
                continue; // Skip self-ref
            }
            let from_level = *node_level.get(edge.from.as_str()).unwrap_or(&0);
            let to_level = *node_level.get(edge.to.as_str()).unwrap_or(&0);
            if from_level == to_level {
                continue; // Same level edges don't use channels
            }

            let min_level = from_level.min(to_level);
            let max_level = from_level.max(to_level);

            // Edge passes through ALL channels from min_level to max_level-1
            for channel_level in min_level..max_level {
                channel_edges_list.entry(channel_level).or_default().push(idx);
            }
        }

        // Convert to count for gap calculation
        let channel_edge_count: HashMap<i64, usize> = channel_edges_list
            .iter()
            .map(|(&k, v)| (k, v.len()))
            .collect();

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

        // Build node order lookup: node_id -> order within level
        // This is used to determine which gap a multi-level edge passes through
        let mut node_order: HashMap<&str, usize> = HashMap::new();
        for nodes_in_level in levels.values() {
            for (idx, node) in nodes_in_level.iter().enumerate() {
                node_order.insert(node.id.as_str(), idx);
            }
        }

        // Analyze corridor requirements BEFORE node placement
        // For multi-level edges, determine which gap they pass through based on
        // the destination's order within its level
        //
        // Key: gap_index (global), Value: list of edge indices
        let mut corridor_edges: HashMap<usize, Vec<usize>> = HashMap::new();
        // Key: edge_index, Value: gap_index
        let mut edge_gap_index: HashMap<usize, usize> = HashMap::new();

        for (idx, edge) in ir.edges.iter().enumerate() {
            if edge.from == edge.to {
                continue;
            }
            let from_level = *node_level.get(edge.from.as_str()).unwrap_or(&0);
            let to_level = *node_level.get(edge.to.as_str()).unwrap_or(&0);

            // Only multi-level edges need corridor routing
            if (to_level - from_level).abs() <= 1 {
                continue;
            }

            // Determine gap index based on destination's order
            // The edge goes toward the destination, so it passes through
            // the gap to the LEFT of the destination's position
            let dest_order = node_order.get(edge.to.as_str()).copied().unwrap_or(0);
            // Gap index: if dest is at order N, the corridor is at gap N
            // (gap 0 = before first node, gap 1 = between node 0 and 1, etc.)
            // We use dest_order as the gap index (edge goes to the left of dest position)
            let gap_index = dest_order;

            edge_gap_index.insert(idx, gap_index);
            corridor_edges.entry(gap_index).or_default().push(idx);
        }

        // Calculate required extra width for each gap
        // Key: gap_index, Value: extra width needed
        let mut gap_extra_width: HashMap<usize, f64> = HashMap::new();
        for (&gap_idx, edges) in &corridor_edges {
            let extra = edges.len() as f64 * self.lane_spacing;
            gap_extra_width.insert(gap_idx, extra);
        }

        // Now place nodes with the calculated gap widths (single pass)
        let mut layout_nodes = Vec::new();
        let mut level_bottom_y: HashMap<i64, f64> = HashMap::new();
        let mut channel_y: HashMap<i64, f64> = HashMap::new();
        let mut y: f64 = 40.0;
        let mut max_width: f64 = 0.0;

        for (i, &level) in level_keys.iter().enumerate() {
            let nodes_in_level = &levels[&level];
            // Start X: margin + extra gap for gap_index 0 (left of first node)
            let gap0_extra = *gap_extra_width.get(&0).unwrap_or(&0.0);
            let mut x: f64 = 40.0 + gap0_extra;
            let mut max_height: f64 = 0.0;

            for (node_idx, node) in nodes_in_level.iter().enumerate() {
                let (w, h) = node_sizes[&node.id];
                layout_nodes.push(LayoutNode {
                    id: node.id.clone(),
                    x,
                    y,
                    width: w,
                    height: h,
                });

                // Calculate gap after this node (gap_index = node_idx + 1)
                // gap_index 0 = before first node, gap_index 1 = after first node, etc.
                let next_gap_idx = node_idx + 1;
                let extra_gap = *gap_extra_width.get(&next_gap_idx).unwrap_or(&0.0);
                let effective_gap_x = self.node_gap_x + extra_gap;

                x += w + effective_gap_x;
                max_height = max_height.max(h);
            }

            // Recalculate max_width accounting for variable gaps
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
        // Key: (channel_level, edge_idx), Value: lane number within that channel
        // Each edge gets a unique lane in EACH channel it passes through
        let mut channel_lane_assignments: HashMap<(i64, usize), usize> = HashMap::new();
        // Separate lane assignments for same-level edges (routed above)
        let mut same_level_lane_assignments: HashMap<usize, usize> = HashMap::new();
        {
            // Use the channel_edges_list we built earlier, but add X position for sorting
            // For each channel, collect (edge_idx, sort_key_x) where sort_key_x determines lane order
            let mut channel_edges_with_x: HashMap<i64, Vec<(usize, f64)>> = HashMap::new();

            for (&channel_level, edge_indices) in &channel_edges_list {
                for &idx in edge_indices {
                    let edge = &ir.edges[idx];
                    let from_node = match node_positions.get(edge.from.as_str()) {
                        Some(n) => n,
                        None => continue,
                    };

                    let from_level = *node_level.get(edge.from.as_str()).unwrap_or(&0);
                    let to_level = *node_level.get(edge.to.as_str()).unwrap_or(&0);
                    let going_down = to_level >= from_level;

                    // Get the actual from_cx for this edge
                    let from_exits = node_exits.get(&(edge.from.as_str(), going_down));
                    let from_cx = if let Some(exits) = from_exits {
                        let pos = exits.iter().position(|(i, _)| *i == idx).unwrap_or(0);
                        self.distribute_anchor(from_node, pos, exits.len())
                    } else {
                        from_node.x + from_node.width / 2.0
                    };

                    channel_edges_with_x
                        .entry(channel_level)
                        .or_default()
                        .push((idx, from_cx));
                }
            }

            // Sort and assign lanes for each channel
            for (&channel_level, edges) in channel_edges_with_x.iter_mut() {
                // Sort by from_cx (right to left) so left edges get higher lanes (turn earlier)
                edges.sort_by(|a, b| {
                    match b.1.partial_cmp(&a.1) {
                        Some(std::cmp::Ordering::Equal) | None => a.0.cmp(&b.0),
                        Some(ord) => ord,
                    }
                });
                for (lane, (edge_idx, _)) in edges.iter().enumerate() {
                    channel_lane_assignments.insert((channel_level, *edge_idx), lane);
                }
            }

            // Collect same-level edges per level
            let mut same_level_edges: HashMap<i64, Vec<(usize, f64)>> = HashMap::new();
            for (idx, edge) in ir.edges.iter().enumerate() {
                if edge.from == edge.to {
                    continue;
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

                if from_level == to_level {
                    // Same-level edge - check if non-adjacent (needs above routing)
                    let (left_node, right_node) = if from_node.x < to_node.x {
                        (from_node, to_node)
                    } else {
                        (to_node, from_node)
                    };
                    let gap_between = right_node.x - (left_node.x + left_node.width);

                    if gap_between > self.node_gap_x * 1.5 {
                        // Non-adjacent same-level edge - needs lane assignment
                        let from_cx = from_node.x + from_node.width / 2.0;
                        same_level_edges
                            .entry(from_level)
                            .or_default()
                            .push((idx, from_cx));
                    }
                }
            }

            // Sort and assign lanes for same-level edges
            for (_level, edges) in same_level_edges.iter_mut() {
                edges.sort_by(|a, b| {
                    match b.1.partial_cmp(&a.1) {
                        Some(std::cmp::Ordering::Equal) | None => a.0.cmp(&b.0),
                        Some(ord) => ord,
                    }
                });
                for (lane, (edge_idx, _)) in edges.iter().enumerate() {
                    same_level_lane_assignments.insert(*edge_idx, lane);
                }
            }
        }

        // Pre-calculate corridor lane assignments for multi-level edges
        // Key: (gap_index, edge_idx), Value: lane within that corridor
        let mut corridor_lane_assignments: HashMap<(usize, usize), usize> = HashMap::new();
        // Key: gap_index, Value: total edge count in that corridor
        let mut corridor_total_edges: HashMap<usize, usize> = HashMap::new();
        {
            // For each corridor, sort edges by their from_cx for lane assignment
            for (&gap_idx, edge_indices) in &corridor_edges {
                // Get from_cx for each edge to sort them
                let mut edges_with_x: Vec<(usize, f64)> = edge_indices
                    .iter()
                    .filter_map(|&idx| {
                        let edge = &ir.edges[idx];
                        let from_node = node_positions.get(edge.from.as_str())?;
                        let from_cx = from_node.x + from_node.width / 2.0;
                        Some((idx, from_cx))
                    })
                    .collect();

                // Sort by from_cx to avoid crossings
                edges_with_x.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

                corridor_total_edges.insert(gap_idx, edges_with_x.len());
                for (lane, (edge_idx, _)) in edges_with_x.iter().enumerate() {
                    corridor_lane_assignments.insert((gap_idx, *edge_idx), lane);
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

                    // Determine lane offset using pre-calculated channel-specific assignment
                    let channel_level = from_level.min(to_level);
                    let total_edges = *channel_edge_count.get(&channel_level).unwrap_or(&1);
                    let lane = *channel_lane_assignments.get(&(channel_level, idx)).unwrap_or(&0);
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
                            // Non-adjacent same-level: route ABOVE the level (separate from inter-level channel)
                            let same_level_lane = *same_level_lane_assignments.get(&idx).unwrap_or(&0);
                            // Lane 0 is closest to entity, higher lanes go further up
                            let same_level_lane_offset = same_level_lane as f64 * self.lane_spacing;

                            let min_top = from_node.y.min(to_node.y);
                            let ch_y = min_top - self.entity_margin - same_level_lane_offset;

                            vec![
                                (from_cx, from_node.y),
                                (from_cx, ch_y),
                                (to_cx, ch_y),
                                (to_cx, to_node.y),
                            ]
                        }
                    } else {
                        // Different levels: route through channels between levels
                        // For multi-level edges, we need to route through each intermediate channel
                        let min_level = from_level.min(to_level);
                        let max_level = from_level.max(to_level);
                        let going_down = to_level > from_level;

                        if max_level - min_level == 1 {
                            // Adjacent levels: simple routing through single channel
                            let (upper_node, lower_node, upper_cx, lower_cx) = if from_level < to_level {
                                (from_node, to_node, from_cx, to_cx)
                            } else {
                                (to_node, from_node, to_cx, from_cx)
                            };

                            let ch_y = *channel_y.get(&min_level)
                                .unwrap_or(&(upper_node.y + upper_node.height + self.channel_gap / 2.0))
                                + lane_offset;

                            if going_down {
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
                        } else {
                            // Multi-level edge: route through intermediate channels
                            // Use the pre-calculated gap index for this edge
                            let gap_index = edge_gap_index.get(&idx).copied().unwrap_or(0);

                            // Get lane assignment for this edge in this corridor
                            let corridor_lane = corridor_lane_assignments
                                .get(&(gap_index, idx))
                                .copied()
                                .unwrap_or(0);

                            // Get total edges in this corridor for centering
                            let total_in_corridor = corridor_total_edges
                                .get(&gap_index)
                                .copied()
                                .unwrap_or(1)
                                .max(1);

                            // Calculate base corridor X using the gap_index
                            // Find the center of the gap at the first intermediate level
                            let base_corridor_x = self.find_gap_center_x(
                                &layout_nodes,
                                &levels,
                                min_level + 1,
                                gap_index,
                            );

                            // Calculate lane offset within corridor
                            let corridor_lane_offset = if total_in_corridor > 1 {
                                (corridor_lane as f64 - (total_in_corridor - 1) as f64 / 2.0) * self.lane_spacing
                            } else {
                                0.0
                            };

                            let corridor_x = base_corridor_x + corridor_lane_offset;

                            // Helper to get lane offset for a specific channel
                            let get_channel_lane_offset = |ch_level: i64| -> f64 {
                                let ch_total = *channel_edge_count.get(&ch_level).unwrap_or(&1);
                                let ch_lane = *channel_lane_assignments.get(&(ch_level, idx)).unwrap_or(&0);
                                (ch_lane as f64 - (ch_total - 1) as f64 / 2.0) * self.lane_spacing
                            };

                            let mut waypoints = Vec::new();

                            if going_down {
                                // Start from source (upper)
                                waypoints.push((from_cx, from_node.y + from_node.height));

                                // First channel: move to corridor
                                let first_ch_level = from_level;
                                let first_lane_offset = get_channel_lane_offset(first_ch_level);
                                let first_ch_y = *channel_y.get(&first_ch_level)
                                    .unwrap_or(&(from_node.y + from_node.height + self.channel_gap / 2.0))
                                    + first_lane_offset;
                                waypoints.push((from_cx, first_ch_y));
                                waypoints.push((corridor_x, first_ch_y));

                                // Go down through intermediate channels
                                for level in (from_level + 1)..to_level {
                                    let ch_lane_offset = get_channel_lane_offset(level);
                                    let ch_y = *channel_y.get(&level)
                                        .unwrap_or(&(first_ch_y + self.channel_gap))
                                        + ch_lane_offset;
                                    waypoints.push((corridor_x, ch_y));
                                }

                                // Last channel: move to destination
                                let last_ch_level = to_level - 1;
                                let last_lane_offset = get_channel_lane_offset(last_ch_level);
                                let last_ch_y = *channel_y.get(&last_ch_level)
                                    .unwrap_or(&(to_node.y - self.channel_gap / 2.0))
                                    + last_lane_offset;
                                // Only add if different from last point
                                if waypoints.last().map(|(_, y)| *y) != Some(last_ch_y) {
                                    waypoints.push((corridor_x, last_ch_y));
                                }
                                waypoints.push((to_cx, last_ch_y));
                                waypoints.push((to_cx, to_node.y));
                            } else {
                                // Going up: start from source (lower)
                                waypoints.push((from_cx, from_node.y));

                                // First channel (above source)
                                let first_ch_level = from_level - 1;
                                let first_lane_offset = get_channel_lane_offset(first_ch_level);
                                let first_ch_y = *channel_y.get(&first_ch_level)
                                    .unwrap_or(&(from_node.y - self.channel_gap / 2.0))
                                    + first_lane_offset;
                                waypoints.push((from_cx, first_ch_y));
                                waypoints.push((corridor_x, first_ch_y));

                                // Go up through intermediate channels
                                for level in (to_level..(from_level - 1)).rev() {
                                    let ch_lane_offset = get_channel_lane_offset(level);
                                    let ch_y = *channel_y.get(&level)
                                        .unwrap_or(&(first_ch_y - self.channel_gap))
                                        + ch_lane_offset;
                                    waypoints.push((corridor_x, ch_y));
                                }

                                // Last channel: move to destination
                                let last_ch_level = to_level;
                                let last_lane_offset = get_channel_lane_offset(last_ch_level);
                                let last_ch_y = *channel_y.get(&last_ch_level)
                                    .unwrap_or(&(to_node.y + to_node.height + self.channel_gap / 2.0))
                                    + last_lane_offset;
                                if waypoints.last().map(|(_, y)| *y) != Some(last_ch_y) {
                                    waypoints.push((corridor_x, last_ch_y));
                                }
                                waypoints.push((to_cx, last_ch_y));
                                waypoints.push((to_cx, to_node.y + to_node.height));
                            }

                            waypoints
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

        // Debug: Detect edge crossings (orthogonal intersections)
        self.detect_crossings(&layout_edges);

        Layout {
            nodes: layout_nodes,
            edges: layout_edges,
            width: max_width,
            height: total_height,
            channel_gap: self.channel_gap,
            corner_radius: self.corner_radius,
        }
    }

    /// Detect and log edge crossings (where a horizontal segment crosses a vertical segment)
    fn detect_crossings(&self, edges: &[LayoutEdge]) {
        // Extract horizontal and vertical segments from each edge
        // Segment: ((x1, y1), (x2, y2), edge_index, segment_index)
        let mut h_segments: Vec<(f64, f64, f64, usize, &str, &str)> = Vec::new(); // (y, x_min, x_max, edge_idx, from, to)
        let mut v_segments: Vec<(f64, f64, f64, usize, &str, &str)> = Vec::new(); // (x, y_min, y_max, edge_idx, from, to)

        for edge in edges {
            if edge.is_self_ref {
                continue;
            }
            let waypoints = &edge.waypoints;
            for i in 0..waypoints.len().saturating_sub(1) {
                let (x1, y1) = waypoints[i];
                let (x2, y2) = waypoints[i + 1];

                if (y1 - y2).abs() < 0.1 {
                    // Horizontal segment
                    let x_min = x1.min(x2);
                    let x_max = x1.max(x2);
                    if x_max - x_min > 1.0 {
                        h_segments.push((y1, x_min, x_max, edge.edge_index, &edge.from, &edge.to));
                    }
                } else if (x1 - x2).abs() < 0.1 {
                    // Vertical segment
                    let y_min = y1.min(y2);
                    let y_max = y1.max(y2);
                    if y_max - y_min > 1.0 {
                        v_segments.push((x1, y_min, y_max, edge.edge_index, &edge.from, &edge.to));
                    }
                }
            }
        }

        // Check all pairs of horizontal and vertical segments for crossings
        let mut crossings: Vec<(usize, usize, &str, &str, &str, &str)> = Vec::new();

        for &(h_y, h_x_min, h_x_max, h_idx, h_from, h_to) in &h_segments {
            for &(v_x, v_y_min, v_y_max, v_idx, v_from, v_to) in &v_segments {
                // Skip if same edge
                if h_idx == v_idx {
                    continue;
                }

                // Check if they cross
                // v_x must be within h_x range (exclusive of endpoints)
                // h_y must be within v_y range (exclusive of endpoints)
                let margin = 1.0;
                if v_x > h_x_min + margin && v_x < h_x_max - margin
                    && h_y > v_y_min + margin && h_y < v_y_max - margin
                {
                    // Avoid duplicate pairs
                    if h_idx < v_idx {
                        crossings.push((h_idx, v_idx, h_from, h_to, v_from, v_to));
                    }
                }
            }
        }

        // Log crossings
        if !crossings.is_empty() {
            eprintln!("[DEBUG] Edge crossings detected: {} total", crossings.len());
            for (idx1, idx2, from1, to1, from2, to2) in &crossings {
                eprintln!("  Cross: edge[{}] ({}->{}) x edge[{}] ({}->{})",
                    idx1, from1, to1, idx2, from2, to2);
            }
        }
    }

    /// Distribute anchor points along a node's horizontal edge
    fn distribute_anchor(&self, node: &LayoutNode, position: usize, total: usize) -> f64 {
        let cx = node.x + node.width / 2.0;
        if total <= 1 {
            cx
        } else {
            let offset = (position as f64 - (total - 1) as f64 / 2.0) * self.anchor_spacing;
            cx + offset
        }
    }

    /// Find the center X coordinate of a specific gap at a given level.
    /// gap_index: 0 = before first entity, 1 = between first and second, etc.
    fn find_gap_center_x(
        &self,
        layout_nodes: &[LayoutNode],
        levels: &HashMap<i64, Vec<&Node>>,
        level: i64,
        gap_index: usize,
    ) -> f64 {
        // Get nodes at this level
        let nodes_at_level = match levels.get(&level) {
            Some(nodes) => nodes,
            None => return 100.0, // Default fallback
        };

        // Build a lookup from node id to layout position
        let node_positions: HashMap<&str, &LayoutNode> = layout_nodes
            .iter()
            .map(|n| (n.id.as_str(), n))
            .collect();

        // Build list of entity boundaries at this level
        let mut boundaries: Vec<(f64, f64)> = Vec::new(); // (left_x, right_x)

        for node in nodes_at_level {
            if let Some(layout_node) = node_positions.get(node.id.as_str()) {
                boundaries.push((layout_node.x, layout_node.x + layout_node.width));
            }
        }

        // Sort by left edge
        boundaries.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        if boundaries.is_empty() {
            return 100.0; // Default fallback
        }

        // Find the center of the specified gap
        if gap_index == 0 && !boundaries.is_empty() {
            // Gap before first entity - use midpoint between left margin and first entity
            let first_left = boundaries[0].0;
            return (40.0 + first_left) / 2.0;
        }

        if gap_index >= boundaries.len() {
            // Gap after last entity
            if let Some(&(_, last_right)) = boundaries.last() {
                return last_right + self.entity_margin + 50.0;
            }
        }

        // Gap between entities at index gap_index-1 and gap_index
        if gap_index > 0 && gap_index < boundaries.len() {
            let left_entity_right = boundaries[gap_index - 1].1;
            let right_entity_left = boundaries[gap_index].0;
            return (left_entity_right + right_entity_left) / 2.0;
        }

        // Fallback: gap between entity at gap_index and next
        if gap_index < boundaries.len().saturating_sub(1) {
            let left_entity_right = boundaries[gap_index].1;
            let right_entity_left = boundaries[gap_index + 1].0;
            return (left_entity_right + right_entity_left) / 2.0;
        }

        // Default fallback
        100.0
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
