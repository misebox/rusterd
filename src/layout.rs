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
            corner_radius: 32.0,  // Rounded corner radius
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
        // the positions of both source and destination nodes
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

            // Determine gap index based on both source and destination positions
            // Choose the corridor that is closest to both endpoints
            let from_order = node_order.get(edge.from.as_str()).copied().unwrap_or(0);
            let to_order = node_order.get(edge.to.as_str()).copied().unwrap_or(0);

            // Gap index options:
            // - Left of from: from_order
            // - Right of from: from_order + 1
            // - Left of to: to_order
            // - Right of to: to_order + 1
            //
            // Choose the gap that minimizes horizontal distance
            // If from is to the left of to, use the gap between them (min of right-of-from and left-of-to)
            // If from is to the right of to, use the gap between them (min of left-of-from and right-of-to)
            let gap_index = if from_order <= to_order {
                // from is left of or same as to
                // Use the gap that is right of from OR left of to, whichever is smaller gap index
                // This means we go through the corridor closest to the from node
                (from_order + 1).min(to_order)
            } else {
                // from is right of to
                // Use the gap left of from OR right of to, whichever is larger gap index
                from_order.max(to_order + 1)
            };

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
        // For multi-level edges, use corridor X position for sorting to reduce crossings
        let mut node_exits: HashMap<(&str, bool), Vec<(usize, f64)>> = HashMap::new(); // (edge_idx, sort_key_x)

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

            // For multi-level edges, use corridor position as sort key
            // For adjacent/same-level edges, use target center X
            let is_multi_level = (to_level - from_level).abs() > 1;
            let sort_key_x = if is_multi_level {
                // Use corridor X position (calculated from gap_index)
                if let Some(&gap_idx) = edge_gap_index.get(&idx) {
                    self.find_gap_center_x(&layout_nodes, &levels, from_level + 1, gap_idx)
                } else {
                    to_node.x + to_node.width / 2.0
                }
            } else {
                to_node.x + to_node.width / 2.0
            };

            node_exits
                .entry((edge.from.as_str(), going_down))
                .or_default()
                .push((idx, sort_key_x));

            // Also track entry points on destination node
            // For multi-level edges, use corridor position; otherwise use source center X
            let entry_sort_key_x = if is_multi_level {
                if let Some(&gap_idx) = edge_gap_index.get(&idx) {
                    self.find_gap_center_x(&layout_nodes, &levels, to_level - 1, gap_idx)
                } else {
                    from_node.x + from_node.width / 2.0
                }
            } else {
                from_node.x + from_node.width / 2.0
            };

            node_exits
                .entry((edge.to.as_str(), !going_down))
                .or_default()
                .push((idx, entry_sort_key_x));
        }

        // Sort edges in each group by sort key X position
        for edges in node_exits.values_mut() {
            edges.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        }

        // Optimize anchor order by swapping pairs where anchor order differs from destination order
        // This reduces edge crossings near the node
        {
            // For exits: compare anchor order with destination X order
            for ((_node_id, going_down), edges) in node_exits.iter_mut() {
                if edges.len() < 2 || !going_down {
                    continue;
                }

                // Get destination X for each edge
                let dest_positions: HashMap<usize, f64> = edges
                    .iter()
                    .filter_map(|(idx, _)| {
                        let edge = &ir.edges[*idx];
                        node_positions.get(edge.to.as_str()).map(|n| (*idx, n.x + n.width / 2.0))
                    })
                    .collect();

                // Re-sort by destination X to minimize crossings
                edges.sort_by(|a, b| {
                    let dest_a = dest_positions.get(&a.0).copied().unwrap_or(a.1);
                    let dest_b = dest_positions.get(&b.0).copied().unwrap_or(b.1);
                    dest_a.partial_cmp(&dest_b).unwrap_or(std::cmp::Ordering::Equal)
                });
            }

            // For entries: compare anchor order with source X order
            for ((_node_id, going_down), edges) in node_exits.iter_mut() {
                if edges.len() < 2 || *going_down {
                    continue;
                }

                // Get source X for each edge
                let source_positions: HashMap<usize, f64> = edges
                    .iter()
                    .filter_map(|(idx, _)| {
                        let edge = &ir.edges[*idx];
                        node_positions.get(edge.from.as_str()).map(|n| (*idx, n.x + n.width / 2.0))
                    })
                    .collect();

                // Re-sort by source X to minimize crossings
                edges.sort_by(|a, b| {
                    let src_a = source_positions.get(&a.0).copied().unwrap_or(a.1);
                    let src_b = source_positions.get(&b.0).copied().unwrap_or(b.1);
                    src_a.partial_cmp(&src_b).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
        }

        // Pre-calculate lane assignments by sorting edges within each channel
        // Key: (channel_level, edge_idx), Value: lane number within that channel
        // Each edge gets a unique lane in EACH channel it passes through
        // Edges going UP use lower lanes (closer to upper entity)
        // Edges going DOWN use higher lanes (closer to lower entity)
        let mut channel_lane_assignments: HashMap<(i64, usize), usize> = HashMap::new();
        // Separate lane assignments for same-level edges (routed above)
        let mut same_level_lane_assignments: HashMap<usize, usize> = HashMap::new();
        {
            // For each channel, collect edges with direction info
            // (edge_idx, from_cx, is_going_up relative to this channel)
            let mut channel_edges_with_info: HashMap<i64, Vec<(usize, f64, bool)>> = HashMap::new();


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

                    // Determine if this edge is going UP relative to this channel
                    // Channel N is between level N and level N+1
                    // If edge goes from level > channel_level to level <= channel_level, it's going UP through this channel
                    // If edge goes from level <= channel_level to level > channel_level, it's going DOWN through this channel
                    let is_going_up = to_level <= channel_level;

                    // Get the actual from_cx for this edge
                    let from_exits = node_exits.get(&(edge.from.as_str(), going_down));
                    let from_cx = if let Some(exits) = from_exits {
                        let pos = exits.iter().position(|(i, _)| *i == idx).unwrap_or(0);
                        self.distribute_anchor(from_node, pos, exits.len())
                    } else {
                        from_node.x + from_node.width / 2.0
                    };

                    channel_edges_with_info
                        .entry(channel_level)
                        .or_default()
                        .push((idx, from_cx, is_going_up));
                }
            }

            // Sort and assign lanes for each channel
            // Edges going to closer levels use inner lanes (closer to that level's entities)
            // Within same level, edges to outer (left/right) nodes use inner lanes
            for (&channel_level, edges) in channel_edges_with_info.iter_mut() {
                edges.sort_by(|a, b| {
                    let edge_a = &ir.edges[a.0];
                    let edge_b = &ir.edges[b.0];
                    let from_level_a = *node_level.get(edge_a.from.as_str()).unwrap_or(&0);
                    let from_level_b = *node_level.get(edge_b.from.as_str()).unwrap_or(&0);
                    let to_level_a = *node_level.get(edge_a.to.as_str()).unwrap_or(&0);
                    let to_level_b = *node_level.get(edge_b.to.as_str()).unwrap_or(&0);
                    let is_down_a = to_level_a > channel_level;
                    let is_down_b = to_level_b > channel_level;

                    // For multi-level edges, get the corridor X position
                    let get_corridor_x = |edge_idx: usize| -> f64 {
                        if let Some(&gap_idx) = edge_gap_index.get(&edge_idx) {
                            // Find the gap center X at the intermediate level
                            let intermediate_level = channel_level + 1;
                            self.find_gap_center_x(&layout_nodes, &levels, intermediate_level, gap_idx)
                        } else {
                            // Single-level edge: use from_cx
                            let edge = &ir.edges[edge_idx];
                            node_positions
                                .get(edge.from.as_str())
                                .map(|n| n.x + n.width / 2.0)
                                .unwrap_or(0.0)
                        }
                    };

                    // Get destination X position
                    let get_to_x = |edge: &crate::ir::Edge| -> f64 {
                        node_positions
                            .get(edge.to.as_str())
                            .map(|n| n.x + n.width / 2.0)
                            .unwrap_or(0.0)
                    };

                    // Primary: group by direction (down-going first, then up-going)
                    match is_down_b.cmp(&is_down_a) {
                        std::cmp::Ordering::Equal => {
                            // Check if either edge spans multiple levels (uses corridor)
                            let a_multi = (to_level_a - from_level_a).abs() > 1;
                            let b_multi = (to_level_b - from_level_b).abs() > 1;

                            if a_multi || b_multi {
                                // At least one multi-level edge: sort by corridor X first
                                let corridor_x_a = get_corridor_x(a.0);
                                let corridor_x_b = get_corridor_x(b.0);

                                // If corridors are different, sort by corridor X
                                let corridor_diff = (corridor_x_a - corridor_x_b).abs();
                                if corridor_diff > 1.0 {
                                    if is_down_a {
                                        corridor_x_a.partial_cmp(&corridor_x_b).unwrap_or(std::cmp::Ordering::Equal)
                                    } else {
                                        corridor_x_b.partial_cmp(&corridor_x_a).unwrap_or(std::cmp::Ordering::Equal)
                                    }
                                } else {
                                    // Same corridor: sort by destination X
                                    // For edges going down-right: left dest = inner lane
                                    // For edges going down-left: right dest = inner lane (avoid crossing)
                                    let to_x_a = get_to_x(edge_a);
                                    let to_x_b = get_to_x(edge_b);

                                    // If corridor is left of destinations, sort ascending (left first)
                                    // If corridor is right of destinations, sort descending (right first)
                                    let avg_to_x = (to_x_a + to_x_b) / 2.0;
                                    if corridor_x_a < avg_to_x {
                                        // Corridor is left of destinations: sort ascending by dest X
                                        to_x_a.partial_cmp(&to_x_b).unwrap_or(std::cmp::Ordering::Equal)
                                    } else {
                                        // Corridor is right of destinations: sort descending by dest X
                                        to_x_b.partial_cmp(&to_x_a).unwrap_or(std::cmp::Ordering::Equal)
                                    }
                                }
                            } else {
                                // Both single-level: sort by distance to target level, then by X
                                let dist_cmp = if is_down_a {
                                    to_level_a.cmp(&to_level_b)
                                } else {
                                    to_level_b.cmp(&to_level_a)
                                };
                                match dist_cmp {
                                    std::cmp::Ordering::Equal => {
                                        // Sort by destination X relative to source position
                                        // Edges going to nodes on the same side as source should use outer lanes
                                        let to_x_a = get_to_x(edge_a);
                                        let to_x_b = get_to_x(edge_b);
                                        let from_x_a = node_positions
                                            .get(edge_a.from.as_str())
                                            .map(|n| n.x + n.width / 2.0)
                                            .unwrap_or(0.0);

                                        // If source is on the right side, sort descending (right dest = outer lane = higher number)
                                        // If source is on the left side, sort ascending (left dest = outer lane = higher number)
                                        let avg_to_x = (to_x_a + to_x_b) / 2.0;
                                        if from_x_a > avg_to_x {
                                            // Source is right of destinations: right dest should be outer (higher lane)
                                            to_x_b.partial_cmp(&to_x_a).unwrap_or(std::cmp::Ordering::Equal)
                                        } else {
                                            // Source is left of destinations: left dest should be outer (higher lane)
                                            to_x_a.partial_cmp(&to_x_b).unwrap_or(std::cmp::Ordering::Equal)
                                        }
                                    }
                                    ord => ord,
                                }
                            }
                        }
                        ord => ord,
                    }
                });
                for (lane, (edge_idx, _, _)) in edges.iter().enumerate() {
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

        // Optimize lane assignments to reduce crossings
        // Two edges cross twice if they share a channel AND a corridor,
        // AND their relative order is OPPOSITE in the channel vs corridor.
        // (If same order in both, they don't cross; if opposite order, they cross twice)
        // To fix: swap their lanes in ONE of the two (channel or corridor)
        {
            // Build reverse lookup: edge_idx -> list of channels it passes through
            let mut edge_channels: HashMap<usize, Vec<i64>> = HashMap::new();
            for (&(channel, edge_idx), _) in &channel_lane_assignments {
                edge_channels.entry(edge_idx).or_default().push(channel);
            }

            // Build reverse lookup: edge_idx -> corridor gap_index (if multi-level)
            let mut edge_corridor: HashMap<usize, usize> = HashMap::new();
            for (&(gap_idx, edge_idx), _) in &corridor_lane_assignments {
                edge_corridor.insert(edge_idx, gap_idx);
            }

            // Find pairs that share both channel(s) and corridor with OPPOSITE relative order
            let mut swap_candidates: Vec<(usize, usize, i64)> = Vec::new(); // (edge1, edge2, shared_channel)

            let edge_indices: Vec<usize> = edge_channels.keys().copied().collect();
            for i in 0..edge_indices.len() {
                for j in (i + 1)..edge_indices.len() {
                    let e1 = edge_indices[i];
                    let e2 = edge_indices[j];

                    // Check if both are in the same corridor
                    let gap1 = edge_corridor.get(&e1);
                    let gap2 = edge_corridor.get(&e2);
                    if gap1.is_none() || gap2.is_none() || gap1 != gap2 {
                        continue;
                    }
                    let gap_idx = *gap1.unwrap();

                    // Get corridor lanes
                    let corridor_lane1 = corridor_lane_assignments.get(&(gap_idx, e1)).copied();
                    let corridor_lane2 = corridor_lane_assignments.get(&(gap_idx, e2)).copied();
                    if corridor_lane1.is_none() || corridor_lane2.is_none() {
                        continue;
                    }
                    let cl1 = corridor_lane1.unwrap();
                    let cl2 = corridor_lane2.unwrap();
                    // Corridor order: e1 < e2 in corridor if cl1 < cl2
                    let e1_before_e2_in_corridor = cl1 < cl2;

                    // Check if they share any channel with OPPOSITE relative order
                    let channels1 = edge_channels.get(&e1).map(|v| v.as_slice()).unwrap_or(&[]);
                    let channels2 = edge_channels.get(&e2).map(|v| v.as_slice()).unwrap_or(&[]);

                    for &ch in channels1 {
                        if channels2.contains(&ch) {
                            // Get channel lanes
                            let ch_lane1 = channel_lane_assignments.get(&(ch, e1)).copied();
                            let ch_lane2 = channel_lane_assignments.get(&(ch, e2)).copied();
                            if let (Some(chl1), Some(chl2)) = (ch_lane1, ch_lane2) {
                                let e1_before_e2_in_channel = chl1 < chl2;

                                // If orders are OPPOSITE, they will cross twice
                                if e1_before_e2_in_corridor != e1_before_e2_in_channel {
                                    swap_candidates.push((e1, e2, ch));
                                    break; // One shared channel is enough
                                }
                            }
                        }
                    }
                }
            }

            // For each candidate pair, swap their channel lanes
            // Track which edges have been swapped to avoid undoing swaps
            let mut swapped_edges: std::collections::HashSet<usize> = std::collections::HashSet::new();

            for &(e1, e2, channel) in &swap_candidates {
                // Skip if either edge has already been involved in a swap
                if swapped_edges.contains(&e1) || swapped_edges.contains(&e2) {
                    continue;
                }

                let lane1 = channel_lane_assignments.get(&(channel, e1)).copied();
                let lane2 = channel_lane_assignments.get(&(channel, e2)).copied();

                if let (Some(l1), Some(l2)) = (lane1, lane2) {
                    // Swap lanes in the channel to match corridor order
                    channel_lane_assignments.insert((channel, e1), l2);
                    channel_lane_assignments.insert((channel, e2), l1);
                    swapped_edges.insert(e1);
                    swapped_edges.insert(e2);
                }
            }
        }

        // Pre-calculate safe corridor assignments for multi-level edges
        // Key: edge_idx, Value: (corridor_x, lane, total_lanes)
        let mut multi_level_corridor_x: HashMap<usize, f64> = HashMap::new();
        {
            // Group multi-level edges by their safe corridor
            // Key: (min_level, max_level, corridor_index), Value: list of edge indices
            let mut corridor_groups: HashMap<(i64, i64, usize), Vec<usize>> = HashMap::new();

            for (idx, edge) in ir.edges.iter().enumerate() {
                if edge.from == edge.to {
                    continue;
                }
                let from_level = *node_level.get(edge.from.as_str()).unwrap_or(&0);
                let to_level = *node_level.get(edge.to.as_str()).unwrap_or(&0);

                // Only multi-level edges (spanning more than 1 level)
                if (to_level - from_level).abs() <= 1 {
                    continue;
                }

                let min_level = from_level.min(to_level);
                let max_level = from_level.max(to_level);

                // Find safe corridors for this edge
                let safe_corridors = self.find_safe_corridor_x(&layout_nodes, &levels, min_level, max_level);

                // Choose the best corridor (closest to midpoint of from and to)
                let from_node = match node_positions.get(edge.from.as_str()) {
                    Some(n) => n,
                    None => continue,
                };
                let to_node = match node_positions.get(edge.to.as_str()) {
                    Some(n) => n,
                    None => continue,
                };
                let target_x = (from_node.x + from_node.width / 2.0 + to_node.x + to_node.width / 2.0) / 2.0;

                let best_corridor_idx = safe_corridors
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| {
                        let center_a = (a.0 + a.1.min(5000.0)) / 2.0;
                        let center_b = (b.0 + b.1.min(5000.0)) / 2.0;
                        let dist_a = (center_a - target_x).abs();
                        let dist_b = (center_b - target_x).abs();
                        dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(i, _)| i)
                    .unwrap_or(0);

                corridor_groups
                    .entry((min_level, max_level, best_corridor_idx))
                    .or_default()
                    .push(idx);
            }

            // Assign lanes within each corridor group
            for ((min_level, max_level, corridor_idx), edge_indices) in &corridor_groups {
                let safe_corridors = self.find_safe_corridor_x(&layout_nodes, &levels, *min_level, *max_level);
                let (corridor_left, corridor_right) = safe_corridors
                    .get(*corridor_idx)
                    .copied()
                    .unwrap_or((40.0, 200.0));

                let total_lanes = edge_indices.len();
                let corridor_center = (corridor_left + corridor_right) / 2.0;

                // Sort edges by from_cx for consistent lane assignment
                let mut edges_sorted: Vec<(usize, f64)> = edge_indices
                    .iter()
                    .filter_map(|&idx| {
                        let edge = &ir.edges[idx];
                        let from_node = node_positions.get(edge.from.as_str())?;
                        Some((idx, from_node.x + from_node.width / 2.0))
                    })
                    .collect();
                edges_sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

                // Assign corridor X for each edge
                for (lane, (edge_idx, _)) in edges_sorted.iter().enumerate() {
                    let lane_offset = if total_lanes > 1 {
                        (lane as f64 - (total_lanes - 1) as f64 / 2.0) * self.lane_spacing
                    } else {
                        0.0
                    };
                    let corridor_x = corridor_center + lane_offset;
                    multi_level_corridor_x.insert(*edge_idx, corridor_x);
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
                            // Non-adjacent same-level: need to route around intermediate entities
                            // Use a corridor between from and to nodes
                            let same_level_lane = *same_level_lane_assignments.get(&idx).unwrap_or(&0);
                            let same_level_lane_offset = same_level_lane as f64 * self.lane_spacing;

                            // Find a corridor between from and to
                            let from_order = node_order.get(edge.from.as_str()).copied().unwrap_or(0);
                            let to_order = node_order.get(edge.to.as_str()).copied().unwrap_or(0);
                            let corridor_gap = if from_order < to_order {
                                from_order + 1  // Right of from
                            } else {
                                to_order + 1    // Right of to
                            };

                            // Get corridor X from the actual gap position at this level
                            let corridor_x = self.find_gap_center_x(&layout_nodes, &levels, from_level, corridor_gap)
                                + same_level_lane_offset;

                            // Use the channel below this level
                            let ch_y = *channel_y.get(&from_level)
                                .unwrap_or(&(from_node.y + from_node.height + self.channel_gap / 2.0));

                            vec![
                                (from_cx, from_node.y + from_node.height),
                                (from_cx, ch_y),
                                (corridor_x, ch_y),
                                (corridor_x, to_node.y + to_node.height),
                                (to_cx, to_node.y + to_node.height),
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
                            // If from_cx and to_cx are close enough, route directly (no horizontal segment)
                            let direct_threshold = 1.0; // Threshold for "close enough" to go straight

                            if (from_cx - to_cx).abs() < direct_threshold {
                                // Direct vertical connection
                                if going_down {
                                    vec![
                                        (from_cx, from_node.y + from_node.height),
                                        (to_cx, to_node.y),
                                    ]
                                } else {
                                    vec![
                                        (from_cx, from_node.y),
                                        (to_cx, to_node.y + to_node.height),
                                    ]
                                }
                            } else {
                                // Need horizontal routing through channel
                                let upper_node = if from_level < to_level {
                                    from_node
                                } else {
                                    to_node
                                };

                                let ch_y = *channel_y.get(&min_level)
                                    .unwrap_or(&(upper_node.y + upper_node.height + self.channel_gap / 2.0))
                                    + lane_offset;

                                // Waypoints always go from -> to
                                if going_down {
                                    // from is upper, to is lower
                                    vec![
                                        (from_cx, from_node.y + from_node.height),
                                        (from_cx, ch_y),
                                        (to_cx, ch_y),
                                        (to_cx, to_node.y),
                                    ]
                                } else {
                                    // from is lower, to is upper
                                    vec![
                                        (from_cx, from_node.y),
                                        (from_cx, ch_y),
                                        (to_cx, ch_y),
                                        (to_cx, to_node.y + to_node.height),
                                    ]
                                }
                            }
                        } else {
                            // Multi-level edge: use pre-calculated corridor X
                            let corridor_x = multi_level_corridor_x
                                .get(&idx)
                                .copied()
                                .unwrap_or_else(|| {
                                    // Fallback: calculate on the fly
                                    let safe_corridors = self.find_safe_corridor_x(
                                        &layout_nodes,
                                        &levels,
                                        min_level,
                                        max_level,
                                    );
                                    safe_corridors
                                        .first()
                                        .map(|(l, r)| (l + r) / 2.0)
                                        .unwrap_or(100.0)
                                });

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

        Layout {
            nodes: layout_nodes,
            edges: layout_edges,
            width: max_width,
            height: total_height,
            channel_gap: self.channel_gap,
            corner_radius: self.corner_radius,
        }
    }

    /// Detect edge crossings and return pairs that cross an even number of times
    /// Returns: Vec<(edge_idx1, edge_idx2)> - pairs that cross an even number of times
    #[allow(dead_code)]
    fn detect_crossings(&self, edges: &[LayoutEdge]) -> Vec<(usize, usize)> {
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
        // Count crossings per edge pair: ((edge1, edge2), count)
        let mut crossing_counts: HashMap<(usize, usize), usize> = HashMap::new();

        for &(h_y, h_x_min, h_x_max, h_idx, _, _) in &h_segments {
            for &(v_x, v_y_min, v_y_max, v_idx, _, _) in &v_segments {
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
                    // Normalize pair order for consistent counting
                    let pair = if h_idx < v_idx {
                        (h_idx, v_idx)
                    } else {
                        (v_idx, h_idx)
                    };
                    *crossing_counts.entry(pair).or_insert(0) += 1;
                }
            }
        }

        // Return pairs that cross an even number of times (candidates for lane swap optimization)
        crossing_counts
            .into_iter()
            .filter(|(_, count)| *count % 2 == 0)
            .map(|((idx1, idx2), _)| (idx1, idx2))
            .collect()
    }

    /// Detect anchor swap candidates - pairs of edges from/to the same node
    /// where swapping their anchor positions would reduce crossings.
    ///
    /// Returns: Vec<(node_id, edge_idx1, edge_idx2, is_exit)>
    /// - node_id: the node where both edges connect
    /// - edge_idx1, edge_idx2: the two edges that should swap anchors
    /// - is_exit: true if these are exit anchors, false if entry anchors
    #[allow(dead_code)]
    fn detect_anchor_swap_candidates<'a>(
        &self,
        edges: &'a [LayoutEdge],
        _ir_edges: &[crate::ir::Edge],
    ) -> Vec<(&'a str, usize, usize, bool)> {
        let mut candidates: Vec<(&str, usize, usize, bool)> = Vec::new();

        // Group edges by their source node (exits) and target node (entries)
        let mut exits_by_node: HashMap<&str, Vec<&LayoutEdge>> = HashMap::new();
        let mut entries_by_node: HashMap<&str, Vec<&LayoutEdge>> = HashMap::new();

        for edge in edges {
            if edge.is_self_ref {
                continue;
            }
            exits_by_node.entry(&edge.from).or_default().push(edge);
            entries_by_node.entry(&edge.to).or_default().push(edge);
        }

        // Check exits: for each node, find pairs where anchor order differs from route direction
        for (node_id, node_edges) in &exits_by_node {
            if node_edges.len() < 2 {
                continue;
            }

            // Get anchor X positions and destination X position
            // We compare anchor order with destination order to detect crossings
            let mut edge_info: Vec<(usize, f64, f64, &str)> = Vec::new(); // (edge_idx, anchor_x, dest_x, to)
            for edge in node_edges {
                if edge.waypoints.len() >= 2 {
                    let anchor_x = edge.waypoints[0].0;
                    // Use final destination X position
                    let dest_x = edge.waypoints.last().unwrap().0;
                    edge_info.push((edge.edge_index, anchor_x, dest_x, &edge.to));
                }
            }

            // Check all pairs
            for i in 0..edge_info.len() {
                for j in (i + 1)..edge_info.len() {
                    let (idx1, anchor1, route1, _) = edge_info[i];
                    let (idx2, anchor2, route2, _) = edge_info[j];

                    // Skip if anchors are at same position (can't compare order)
                    if (anchor1 - anchor2).abs() < 1.0 {
                        continue;
                    }
                    // Skip if routes are at same position
                    if (route1 - route2).abs() < 1.0 {
                        continue;
                    }

                    // Check if anchor order differs from route order
                    let anchor_order = anchor1 < anchor2; // true if edge1 anchor is left of edge2
                    let route_order = route1 < route2; // true if edge1 routes left of edge2

                    if anchor_order != route_order {
                        // Swapping would help reduce crossing
                        candidates.push((node_id, idx1, idx2, true));
                    }
                }
            }
        }

        // Check entries: similar logic for entry anchors
        for (node_id, node_edges) in &entries_by_node {
            if node_edges.len() < 2 {
                continue;
            }

            // Get anchor X positions and source X position
            let mut edge_info: Vec<(usize, f64, f64)> = Vec::new();
            for edge in node_edges {
                if edge.waypoints.len() >= 2 {
                    let anchor_x = edge.waypoints.last().unwrap().0;
                    // Use source (first waypoint) X position
                    let source_x = edge.waypoints[0].0;
                    edge_info.push((edge.edge_index, anchor_x, source_x));
                }
            }

            for i in 0..edge_info.len() {
                for j in (i + 1)..edge_info.len() {
                    let (idx1, anchor1, source1) = edge_info[i];
                    let (idx2, anchor2, source2) = edge_info[j];

                    if (anchor1 - anchor2).abs() < 1.0 {
                        continue;
                    }
                    if (source1 - source2).abs() < 1.0 {
                        continue;
                    }

                    let anchor_order = anchor1 < anchor2;
                    let source_order = source1 < source2;

                    if anchor_order != source_order {
                        candidates.push((node_id, idx1, idx2, false));
                    }
                }
            }
        }

        candidates
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

    /// Find a corridor X position that doesn't intersect any entity across all levels from min_level to max_level.
    /// Returns the center X of the safe corridor.
    fn find_safe_corridor_x(
        &self,
        layout_nodes: &[LayoutNode],
        levels: &HashMap<i64, Vec<&Node>>,
        min_level: i64,
        max_level: i64,
    ) -> Vec<(f64, f64)> {
        // Build a lookup from node id to layout position
        let node_positions: HashMap<&str, &LayoutNode> = layout_nodes
            .iter()
            .map(|n| (n.id.as_str(), n))
            .collect();

        // Collect all entity boundaries across all levels between min_level and max_level (exclusive)
        // We only need to avoid entities at intermediate levels
        let mut all_boundaries: Vec<(f64, f64)> = Vec::new();

        for level in (min_level + 1)..max_level {
            if let Some(nodes_at_level) = levels.get(&level) {
                for node in nodes_at_level {
                    if let Some(layout_node) = node_positions.get(node.id.as_str()) {
                        all_boundaries.push((layout_node.x - self.entity_margin, layout_node.x + layout_node.width + self.entity_margin));
                    }
                }
            }
        }

        // Sort boundaries by left edge
        all_boundaries.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        // Merge overlapping boundaries
        let mut merged: Vec<(f64, f64)> = Vec::new();
        for (left, right) in all_boundaries {
            if let Some(last) = merged.last_mut() {
                if left <= last.1 {
                    last.1 = last.1.max(right);
                } else {
                    merged.push((left, right));
                }
            } else {
                merged.push((left, right));
            }
        }

        // Find gaps between merged boundaries
        let mut gaps: Vec<(f64, f64)> = Vec::new();

        // Gap before first entity
        if let Some(&(first_left, _)) = merged.first() {
            if first_left > 40.0 {
                gaps.push((40.0, first_left));
            }
        } else {
            // No entities at intermediate levels - entire width is available
            gaps.push((40.0, 10000.0));
        }

        // Gaps between entities
        for i in 0..merged.len().saturating_sub(1) {
            let gap_left = merged[i].1;
            let gap_right = merged[i + 1].0;
            if gap_right > gap_left {
                gaps.push((gap_left, gap_right));
            }
        }

        // Gap after last entity
        if let Some(&(_, last_right)) = merged.last() {
            gaps.push((last_right, 10000.0));
        }

        gaps
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
