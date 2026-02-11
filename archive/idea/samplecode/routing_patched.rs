//! Gravity-Pressure (GP) Routing Algorithm
//!
//! Implements the routing algorithm that provides theoretical delivery guarantee
//! even when greedy forwarding fails due to local minima.
//!
//! Reference: Cvetkovski窶鼎rovella (2009)

use crate::coordinates::{NodeId, RoutingCoordinate};
use crate::hyper_press::HyperPress;
use crate::landmark_routing::{LandmarkRoutingConfig, LandmarkRoutingTable};
use crate::PoincareDiskPoint;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

/// Routing mode for the GP algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoutingMode {
    /// Greedy mode: forward to neighbor closest to destination
    Gravity,
    /// Recovery mode: use pressure to escape local minimum
    Pressure,
    /// Tree mode: use spanning tree structure for guaranteed delivery (DFS fallback)
    Tree,
    /// Thorup-Zwick mode: use precomputed routing table with stretch <= 3 guarantee
    ThorupZwick,
    /// HYPER-PRESS mode: H^2 hyperbolic coordinates + Laplacian potential
    HyperPress,
}

/// Result of a routing decision
#[derive(Debug, Clone)]
pub enum RoutingDecision {
    /// Forward packet to specified next hop
    Forward { next_hop: NodeId, mode: RoutingMode },
    /// Packet has reached destination
    Delivered,
    /// Routing failed (should not happen in connected graph with sufficient TTL)
    Failed { reason: String },
}

/// Packet header for GP routing
#[derive(Debug, Clone)]
pub struct PacketHeader {
    /// Source node ID
    pub source: NodeId,
    /// Destination node ID
    pub destination: NodeId,
    /// Target coordinate (either anchor or routing coordinate)
    pub target_coord: PoincareDiskPoint,
    /// Current routing mode
    pub mode: RoutingMode,
    /// Time-to-live
    pub ttl: u32,
    /// Visited nodes (for DFS and pressure calculation)
    pub visited: HashSet<NodeId>,
    /// Pressure values accumulated during Pressure mode
    pub pressure_values: HashMap<NodeId, f64>,
    /// 繝ｪ繧ｫ繝舌Μ繧帝幕蟋九＠縺溷慍轤ｹ縺ｮ繧ｴ繝ｼ繝ｫ縺ｾ縺ｧ縺ｮ霍晞屬・郁┳蜃ｺ蛻､螳夂畑・・
    pub recovery_threshold: f64,
    /// Pressure繝｢繝ｼ繝峨・繧ｹ繝・ャ繝玲焚荳企剞・医％繧後ｒ菴ｿ縺・・縺｣縺溘ｉTree縺ｸ・・
    pub pressure_budget: u32,
    /// DFS繝舌ャ繧ｯ繝医Λ繝・け逕ｨ繧ｹ繧ｿ繝・け: 謌ｻ繧九∋縺阪ヮ繝ｼ繝峨・繝代せ
    pub dfs_stack: Vec<NodeId>,
    /// TZ routing: precomputed path to follow
    pub tz_path: Vec<NodeId>,
    /// TZ routing: current index in the path
    pub tz_path_index: usize,
}

impl PacketHeader {
    pub fn new(
        source: NodeId,
        destination: NodeId,
        target_coord: PoincareDiskPoint,
        ttl: u32,
    ) -> Self {
        Self {
            source,
            destination,
            target_coord,
            mode: RoutingMode::Gravity,
            ttl,
            visited: HashSet::new(),
            pressure_values: HashMap::new(),
            recovery_threshold: f64::INFINITY, // 蛻晄悄蛟､縺ｯ辟｡髯仙､ｧ
            pressure_budget: 0,
            dfs_stack: Vec::new(),
            tz_path: Vec::new(),
            tz_path_index: 0,
        }
    }

    /// Check if a node has been visited
    pub fn has_visited(&self, node: &NodeId) -> bool {
        self.visited.contains(node)
    }

    /// Record a visit to a node
    pub fn record_visit(&mut self, node: NodeId) {
        self.visited.insert(node);
    }

    /// Get visit count (for pressure calculation)
    pub fn visit_count(&self) -> usize {
        self.visited.len()
    }

    /// Compute adaptive TTL based on network size and estimated diameter
    /// Formula: max(α·N, β·log(N)·D, TTL_min)
    /// - α = 0.01: Linear factor for very large graphs
    /// - β = 5.0: Logarithmic factor with diameter
    /// - TTL_min = 200: Minimum TTL for small graphs
    pub fn compute_adaptive_ttl(node_count: usize, estimated_diameter: Option<usize>) -> u32 {
        const ALPHA: f64 = 0.01;
        const BETA: f64 = 5.0;
        const TTL_MIN: u32 = 200;
        const TTL_MAX: u32 = 500_000; // Cap for very large graphs
        
        let n = node_count as f64;
        
        // Linear component: α·N
        let linear_ttl = (ALPHA * n) as u32;
        
        // Logarithmic component: β·log(N)·D
        let diameter = estimated_diameter.unwrap_or_else(|| {
            // Estimate diameter as 2·log(N) for scale-free graphs
            (2.0 * n.ln()).ceil() as usize
        });
        let log_ttl = (BETA * n.ln() * (diameter as f64)) as u32;
        
        // Take maximum of all components, capped
        linear_ttl.max(log_ttl).max(TTL_MIN).min(TTL_MAX)
    }

    /// Create a new packet with adaptive TTL based on network size
    pub fn new_adaptive(
        source: NodeId,
        destination: NodeId,
        target_coord: PoincareDiskPoint,
        node_count: usize,
        estimated_diameter: Option<usize>,
    ) -> Self {
        let ttl = Self::compute_adaptive_ttl(node_count, estimated_diameter);
        Self::new(source, destination, target_coord, ttl)
    }

    /// Create a new packet with HyperPress mode and adaptive TTL
    pub fn new_hyper_press(
        source: NodeId,
        destination: NodeId,
        target_coord: PoincareDiskPoint,
        node_count: usize,
    ) -> Self {
        let ttl = Self::compute_adaptive_ttl(node_count, None);
        let mut packet = Self::new(source, destination, target_coord, ttl);
        packet.mode = RoutingMode::HyperPress;
        packet
    }
}

/// A node in the routing network
#[derive(Debug, Clone)]
pub struct RoutingNode {
    pub id: NodeId,
    pub coord: RoutingCoordinate,
    pub neighbors: Vec<NodeId>,
    /// Tree parent (from PIE embedding spanning tree)
    pub tree_parent: Option<NodeId>,
    /// Tree children (from PIE embedding spanning tree)
    pub tree_children: Vec<NodeId>,
}

impl RoutingNode {
    pub fn new(id: NodeId, coord: RoutingCoordinate) -> Self {
        Self {
            id,
            coord,
            neighbors: Vec::new(),
            tree_parent: None,
            tree_children: Vec::new(),
        }
    }

    pub fn add_neighbor(&mut self, neighbor: NodeId) {
        if !self.neighbors.contains(&neighbor) {
            self.neighbors.push(neighbor);
        }
    }

    /// Set tree structure information
    pub fn set_tree_info(&mut self, parent: Option<NodeId>, children: Vec<NodeId>) {
        self.tree_parent = parent;
        self.tree_children = children;
    }

    pub fn degree(&self) -> usize {
        self.neighbors.len()
    }
}

#[derive(Debug, Clone)]
pub struct LandmarkRoutingState {
    pub config: LandmarkRoutingConfig,
    pub table: LandmarkRoutingTable,
}

/// GP Router implementing Gravity-Pressure routing algorithm
pub struct GPRouter {
    /// All nodes in the network
    nodes: HashMap<NodeId, RoutingNode>,
    /// Pressure decay factor
    pressure_decay: f64,
    /// Pressure increment per visit
    pressure_increment: f64,
    /// Optional Thorup-Zwick routing table for guaranteed stretch <= 3
    tz_table: Option<crate::tz_routing::TZRoutingTable>,
    /// Optional landmark routing state for real-world graphs
    landmark_state: Option<LandmarkRoutingState>,
    /// Optional HYPER-PRESS router for H^2 + potential routing
    hyper_press: Option<HyperPress>,
}

impl GPRouter {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            pressure_decay: 0.95,        // Slower decay to maintain pressure longer
            pressure_increment: 5.0,     // Stronger pressure to overcome distance
            tz_table: None,
            landmark_state: None,
            hyper_press: None,
        }
    }

    /// Add a node to the network
    pub fn add_node(&mut self, node: RoutingNode) {
        self.nodes.insert(node.id.clone(), node);
    }

    /// Add a bidirectional edge between two nodes
    pub fn add_edge(&mut self, node1: &NodeId, node2: &NodeId) {
        if let Some(n1) = self.nodes.get_mut(node1) {
            n1.add_neighbor(node2.clone());
        }
        if let Some(n2) = self.nodes.get_mut(node2) {
            n2.add_neighbor(node1.clone());
        }
    }

    /// Get a node by ID
    pub fn get_node(&self, id: &NodeId) -> Option<&RoutingNode> {
        self.nodes.get(id)
    }

    /// Get mutable reference to a node
    pub fn get_node_mut(&mut self, id: &NodeId) -> Option<&mut RoutingNode> {
        self.nodes.get_mut(id)
    }

    /// Set the Thorup-Zwick routing table for guaranteed stretch 竕､ 3 fallback
    pub fn set_tz_table(&mut self, table: crate::tz_routing::TZRoutingTable) {
        self.tz_table = Some(table);
    }

    /// Check if TZ table is available
    pub fn has_tz_table(&self) -> bool {
        self.tz_table.is_some()
    }

    /// Get reference to TZ table
    pub fn get_tz_table(&self) -> Option<&crate::tz_routing::TZRoutingTable> {  
        self.tz_table.as_ref()
    }

    /// Enable landmark-guided routing heuristics
    pub fn enable_landmark_routing(
        &mut self,
        table: LandmarkRoutingTable,
        config: LandmarkRoutingConfig,
    ) {
        self.landmark_state = Some(LandmarkRoutingState { config, table });
    }

    /// Check if landmark-guided routing is enabled
    pub fn has_landmark_routing(&self) -> bool {
        self.landmark_state.is_some()
    }

    /// Enable HYPER-PRESS routing (H^2 coordinates + Laplacian potential)
    pub fn enable_hyper_press(&mut self) {
        let mut hp = HyperPress::new();
        
        // Build adjacency list from nodes
        let adjacency: HashMap<NodeId, Vec<NodeId>> = self.nodes.iter()
            .map(|(id, node)| (id.clone(), node.neighbors.clone()))
            .collect();
        
        hp.build_from_adjacency(&adjacency);
        self.hyper_press = Some(hp);
    }

    /// Enable HYPER-PRESS with custom lambda
    pub fn enable_hyper_press_with_lambda(&mut self, lambda: f64) {
        let mut hp = HyperPress::new();
        hp.set_lambda(lambda);
        
        let adjacency: HashMap<NodeId, Vec<NodeId>> = self.nodes.iter()
            .map(|(id, node)| (id.clone(), node.neighbors.clone()))
            .collect();
        
        hp.build_from_adjacency(&adjacency);
        self.hyper_press = Some(hp);
    }

    /// Check if HYPER-PRESS routing is enabled
    pub fn has_hyper_press(&self) -> bool {
        self.hyper_press.is_some()
    }

    /// Get reference to HYPER-PRESS router
    pub fn get_hyper_press(&self) -> Option<&HyperPress> {
        self.hyper_press.as_ref()
    }

    fn distance_to_target(&self, node_id: &NodeId, packet: &PacketHeader) -> f64 {
        if let Some(state) = &self.landmark_state {
            if let Some(landmark_dist) = state.table.distance(node_id, &packet.destination) {
                if state.config.hyperbolic_weight <= 0.0 {
                    return state.config.landmark_weight * landmark_dist;
                }
                let hyper = self
                    .nodes
                    .get(node_id)
                    .map(|n| n.coord.point.hyperbolic_distance(&packet.target_coord))
                    .unwrap_or(f64::INFINITY);
                return state.config.landmark_weight * landmark_dist
                    + state.config.hyperbolic_weight * hyper;
            }
        }

        self.nodes
            .get(node_id)
            .map(|n| n.coord.point.hyperbolic_distance(&packet.target_coord))
            .unwrap_or(f64::INFINITY)
    }

    fn try_lookahead_routing(
        &self,
        current: &RoutingNode,
        packet: &PacketHeader,
    ) -> Option<NodeId> {
        let state = self.landmark_state.as_ref()?;
        if state.config.lookahead_depth == 0 {
            return None;
        }

        let max_nodes = state.config.lookahead_max_nodes.max(1);
        let current_score = self.distance_to_target(&current.id, packet);

        let mut queue = VecDeque::new();
        let mut parents: HashMap<NodeId, NodeId> = HashMap::new();
        let mut depths: HashMap<NodeId, usize> = HashMap::new();
        let mut visited: HashSet<NodeId> = HashSet::new();

        queue.push_back(current.id.clone());
        depths.insert(current.id.clone(), 0);
        visited.insert(current.id.clone());

        let mut best_node: Option<NodeId> = None;
        let mut best_score = current_score;
        let mut best_depth = usize::MAX;

        while let Some(node_id) = queue.pop_front() {
            let node_depth = *depths.get(&node_id).unwrap_or(&0);
            if node_depth >= state.config.lookahead_depth {
                continue;
            }

            let node = match self.nodes.get(&node_id) {
                Some(n) => n,
                None => continue,
            };

            for neighbor_id in &node.neighbors {
                if visited.contains(neighbor_id) || packet.visited.contains(neighbor_id) {
                    continue;
                }
                visited.insert(neighbor_id.clone());
                parents.insert(neighbor_id.clone(), node_id.clone());

                let next_depth = node_depth + 1;
                depths.insert(neighbor_id.clone(), next_depth);

                let score = self.distance_to_target(neighbor_id, packet);
                if score + 1e-9 < best_score || (score <= best_score + 1e-9 && next_depth < best_depth) {
                    best_score = score;
                    best_node = Some(neighbor_id.clone());
                    best_depth = next_depth;
                }

                if visited.len() >= max_nodes {
                    break;
                }
                queue.push_back(neighbor_id.clone());
            }

            if visited.len() >= max_nodes {
                break;
            }
        }

        let target = best_node?;
        self.reconstruct_first_step(&current.id, &target, &parents)
    }

    fn reconstruct_first_step(
        &self,
        start: &NodeId,
        target: &NodeId,
        parents: &HashMap<NodeId, NodeId>,
    ) -> Option<NodeId> {
        let mut current = target.clone();
        while let Some(parent) = parents.get(&current) {
            if parent == start {
                return Some(current);
            }
            current = parent.clone();
        }
        None
    }

    /// Make routing decision for a packet at the current node
    /// 縲心ticky Recovery縲・ 繝｢繝ｼ繝峨↓蠢懊§縺溷宍譬ｼ縺ｪ蛻ｶ蠕｡繝輔Ο繝ｼ繧貞ｮ溯｣・
    pub fn route(&self, current_node: &NodeId, packet: &mut PacketHeader) -> RoutingDecision {
        // [BUG FIX] Check destination FIRST (even if TTL=0, arrival should succeed)
        if current_node == &packet.destination {
            return RoutingDecision::Delivered;
        }

        // TTL check (after destination check)
        if packet.ttl == 0 {
            return RoutingDecision::Failed {
                reason: format!("TTL expired at node {}", current_node),
            };
        }

        let current = match self.nodes.get(current_node) {
            Some(n) => n,
            None => {
                return RoutingDecision::Failed {
                    reason: format!("Node {} not found", current_node),
                }
            }
        };

        // 險ｪ蝠剰ｨ倬鹸・・ree繝｢繝ｼ繝我ｻ･螟悶〒菴ｿ逕ｨ・・
        // Tree繝｢繝ｼ繝峨〒縺ｯ蛻･騾妊FS逕ｨ縺ｮvisited繧堤ｮ｡逅・
        if packet.mode != RoutingMode::Tree {
            packet.record_visit(current_node.clone());
        }

        let current_dist = self.distance_to_target(&current.id, packet);

        // 縲蝉ｿｮ豁｣轤ｹ縲・ 繝｢繝ｼ繝峨↓蠢懊§縺溷宍譬ｼ縺ｪ蛻・ｲ・
        match packet.mode {
            RoutingMode::Gravity => {
                // Gravity mode: standard greedy forwarding
                if let Some(decision) = self.try_gravity_routing(current, packet) {
                    return decision;
                }

                if let Some(next_hop) = self.try_lookahead_routing(current, packet) {
                    return RoutingDecision::Forward {
                        next_hop,
                        mode: RoutingMode::Gravity,
                    };
                }

                // Local minimum -> Pressure mode (first-line recovery)
                packet.mode = RoutingMode::Pressure;
                packet.recovery_threshold = current_dist;
                // Budget: proportional to graph size (e.g., N/2)
                packet.pressure_budget = (self.node_count() as u32) / 2;
                packet.pressure_values.clear(); // Reset on new local minimum

                return self.pressure_routing(current, packet);
            },
            RoutingMode::Tree => {
                // Tree繝｢繝ｼ繝・(Sticky): 髢ｾ蛟､繧剃ｸ句屓繧九∪縺ｧGreedy蜴ｳ遖・(Graph DFS)
                
                // 閼ｱ蜃ｺ蛻､螳・ 迴ｾ蝨ｨ菴咲ｽｮ縺後Μ繧ｫ繝舌Μ髢句ｧ句慍轤ｹ繧医ｊ縲檎｢ｺ螳溘↓縲阪ざ繝ｼ繝ｫ縺ｫ霑代＞縺具ｼ・
                // 蜴ｳ蟇・↑荳咲ｭ牙捷 (<) 繧剃ｽｿ逕ｨ縲・
                if current_dist < packet.recovery_threshold {
                    // 閼ｱ蜃ｺ謌仙粥 -> Gravity繝｢繝ｼ繝峨∈蠕ｩ蟶ｰ
                    packet.mode = RoutingMode::Gravity;
                    packet.recovery_threshold = f64::INFINITY;
                    packet.dfs_stack.clear(); // DFS繧ｹ繧ｿ繝・け繧偵け繝ｪ繧｢
                    packet.visited.clear(); // 險ｪ蝠丞ｱ･豁ｴ繧ゅけ繝ｪ繧｢
                    
                    // Gravity繧定ｩｦ陦・
                    if let Some(decision) = self.try_gravity_routing(current, packet) {
                        return decision;
                    }
                    // Gravity縺悟､ｱ謨励＠縺溘ｉ・育ｨ縺ｪ繧ｱ繝ｼ繧ｹ縺縺鯉ｼ牙・縺ｳTree縺ｸ
                    packet.mode = RoutingMode::Tree;
                    packet.recovery_threshold = current_dist;
                    // Tree蜀埼幕譎ゅ・visited繧偵Μ繧ｻ繝・ヨ
                    packet.visited.clear();
                    packet.visited.insert(current_node.clone());
                    packet.dfs_stack.clear();
                }

                // 閼ｱ蜃ｺ譛ｪ螳御ｺ・-> 繧ｰ繝ｩ繝疋FS縺ｧ蠑ｷ蛻ｶ繝ｫ繝ｼ繝・ぅ繝ｳ繧ｰ
                if let Some(decision) = self.traverse_graph_dfs(current, packet) {
                    return decision;
                }
                
                // DFS縺君one繧定ｿ斐＠縺・= 蜈ｨ謗｢邏｢螳御ｺ・□縺悟ｮ帛・譛ｪ蛻ｰ驕・
                // 縺薙ｌ縺ｯ譛ｬ譚･縺ゅｊ縺医↑縺・ｼ磯｣邨舌げ繝ｩ繝輔↑繧会ｼ・
                // 蠢ｵ縺ｮ縺溘ａvisited繧偵Μ繧ｻ繝・ヨ縺励※蜀崎ｩｦ陦・
                if packet.visited.len() < self.node_count() {
                    // 縺ｾ縺蜈ｨ繝弱・繝峨ｒ險ｪ蝠上＠縺ｦ縺・↑縺・-> 繝舌げ縺ｮ蜿ｯ閭ｽ諤ｧ
                    // visited縺ｨ繧ｹ繧ｿ繝・け繧偵Μ繧ｻ繝・ヨ縺励※蜀埼幕
                    packet.visited.clear();
                    packet.visited.insert(current_node.clone());
                    packet.dfs_stack.clear();
                    
                    if let Some(decision) = self.traverse_graph_dfs(current, packet) {
                        return decision;
                    }
                }
                
                // 譛ｬ蠖薙↓螟ｱ謨暦ｼ医げ繝ｩ繝輔′髱樣｣邨撰ｼ・
                return RoutingDecision::Failed { reason: "Graph is disconnected".to_string() };
            },
            RoutingMode::Pressure => {
                // Pressure繝｢繝ｼ繝・ 螻謇逧・↑鄂縺九ｉ縺ｮ閼ｱ蜃ｺ
                
                // 1. 閼ｱ蜃ｺ蛻､螳・(Gravity縺ｸ縺ｮ蠕ｩ蟶ｰ)
                if current_dist < packet.recovery_threshold {
                    packet.mode = RoutingMode::Gravity;
                    packet.recovery_threshold = f64::INFINITY;
                    packet.pressure_budget = 0;
                    
                    if let Some(decision) = self.try_gravity_routing(current, packet) {
                        return decision;
                    }
                    // 縺ｾ縺輔°縺ｮ螟ｱ謨励↑繧臼ressure邯咏ｶ・
                    packet.mode = RoutingMode::Pressure;
                    packet.recovery_threshold = current_dist;
                }

                // 2. 莠育ｮ励メ繧ｧ繝・け (Tree縺ｸ縺ｮ譛邨ゅヵ繧ｩ繝ｼ繝ｫ繝舌ャ繧ｯ)
                if packet.pressure_budget == 0 {
                    // [IMPROVEMENT] Prefer TZ routing (stretch <= 3)
                    if self.has_tz_table() {
                        packet.mode = RoutingMode::ThorupZwick;
                        packet.tz_path.clear();
                        packet.tz_path_index = 0;
                        return self.route(current_node, packet);
                    }
                    
                    // Tree fallback only if TZ unavailable
                    packet.mode = RoutingMode::Tree;
                    packet.dfs_stack.clear();
                    packet.visited.clear();
                    packet.pressure_values.clear();
                    packet.visited.insert(current_node.clone());
                    
                    if let Some(decision) = self.traverse_graph_dfs(current, packet) {
                        return decision;
                    }
                    return RoutingDecision::Failed { reason: "Graph is disconnected".to_string() };
                }

                // 3. Pressure螳溯｡・
                packet.pressure_budget -= 1;
                return self.pressure_routing(current, packet);
            },
            RoutingMode::ThorupZwick => {
                // ThorupZwick mode: Follow precomputed TZ path for guaranteed stretch 竕､ 3
                
                // If TZ path is empty, compute it
                if packet.tz_path.is_empty() {
                    if let Some(tz_table) = &self.tz_table {
                        if let Some(path) = tz_table.compute_path(current_node, &packet.destination) {
                            packet.tz_path = path;
                            packet.tz_path_index = 0;
                        } else {
                            // TZ path computation failed, try DFS fallback
                            packet.mode = RoutingMode::Tree;
                            return self.route(current_node, packet);
                        }
                    } else {
                        // No TZ table available, fall back to Tree mode
                        packet.mode = RoutingMode::Tree;
                        return self.route(current_node, packet);
                    }
                }

                // Find current position in TZ path
                let current_pos = packet.tz_path.iter().position(|n| n == current_node);
                if let Some(pos) = current_pos {
                    packet.tz_path_index = pos;
                }

                // Get next hop from TZ path
                let next_index = packet.tz_path_index + 1;
                if next_index < packet.tz_path.len() {
                    let next_hop = packet.tz_path[next_index].clone();
                    packet.tz_path_index = next_index;
                    
                    // Check if next hop is neighbor
                    if current.neighbors.contains(&next_hop) {
                        return RoutingDecision::Forward {
                            next_hop,
                            mode: RoutingMode::ThorupZwick,
                        };
                    } else {
                        // Next hop is not directly reachable, find path to it
                        // Use local greedy toward next TZ waypoint
                        if let Some(decision) = self.route_toward_node(current, &next_hop) {
                            return decision;
                        }
                    }
                }

                // Path exhausted but destination not reached - unexpected
                // Fall back to Tree mode
                packet.mode = RoutingMode::Tree;
                packet.tz_path.clear();
                if let Some(decision) = self.traverse_graph_dfs(current, packet) {
                    return decision;
                }
                return RoutingDecision::Failed { reason: "TZ routing exhausted".to_string() };
            },
            RoutingMode::HyperPress => {
                // HYPER-PRESS mode: Use H^2 coordinates + Laplacian potential
                if let Some(hp) = &self.hyper_press {
                    // Use FAST local-potential version (scales to large graphs, avoids global φ collapse)
                    if let Some(next_hop) = hp.find_best_neighbor_fast(
                        current_node,
                        &packet.destination,
                        &packet.visited,
                    ) {
                        return RoutingDecision::Forward {
                            next_hop,
                            mode: RoutingMode::HyperPress,
                        };
                    }
                    
                    // If HYPER-PRESS can't find a good neighbor, try TZ fallback
                    if self.has_tz_table() {
                        packet.mode = RoutingMode::ThorupZwick;
                        packet.tz_path.clear();
                        packet.tz_path_index = 0;
                        return self.route(current_node, packet);
                    }
                    
                    // Final fallback to Tree
                    packet.mode = RoutingMode::Tree;
                    packet.dfs_stack.clear();
                    packet.visited.clear();
                    packet.visited.insert(current_node.clone());
                    if let Some(decision) = self.traverse_graph_dfs(current, packet) {
                        return decision;
                    }
                    return RoutingDecision::Failed { reason: "HYPER-PRESS exhausted, graph disconnected".to_string() };
                } else {
                    // HYPER-PRESS not enabled, fall back to Gravity
                    packet.mode = RoutingMode::Gravity;
                    return self.route(current_node, packet);
                }
            }
        }
    }

        /// Gravity Mode: Greedy forwarding to neighbor closest to target
    fn try_gravity_routing(
        &self,
        current: &RoutingNode,
        packet: &PacketHeader,
    ) -> Option<RoutingDecision> {
        let current_distance = self.distance_to_target(&current.id, packet);

        let mut best_neighbor: Option<&NodeId> = None;
        let mut best_distance = current_distance;

        for neighbor_id in &current.neighbors {
            let distance = self.distance_to_target(neighbor_id, packet);
            if distance < best_distance {
                best_distance = distance;
                best_neighbor = Some(neighbor_id);
            }
        }

        best_neighbor.map(|next_hop| RoutingDecision::Forward {
            next_hop: next_hop.clone(),
            mode: RoutingMode::Gravity,
        })
    }

    /// Route toward a specific node (used by TZ routing for waypoint navigation)
    fn route_toward_node(
        &self,
        current: &RoutingNode,
        target_node: &NodeId,
    ) -> Option<RoutingDecision> {
        // Check if target is a direct neighbor
        if current.neighbors.contains(target_node) {
            return Some(RoutingDecision::Forward {
                next_hop: target_node.clone(),
                mode: RoutingMode::ThorupZwick,
            });
        }

        // Try to find neighbor with target in its neighbors
        for neighbor_id in &current.neighbors {
            if let Some(neighbor) = self.nodes.get(neighbor_id) {
                if neighbor.neighbors.contains(target_node) {
                    return Some(RoutingDecision::Forward {
                        next_hop: neighbor_id.clone(),
                        mode: RoutingMode::ThorupZwick,
                    });
                }
            }
        }

        // Fallback: pick first available neighbor
        current.neighbors.first().map(|n| RoutingDecision::Forward {
            next_hop: n.clone(),
            mode: RoutingMode::ThorupZwick,
        })
    }

    /// Graph DFS Traversal: Explore graph systematically for guaranteed delivery
    ///
    /// This implements a proper DFS that guarantees reachability in any connected graph.
    ///
    /// Algorithm:
    /// 1. Mark current node as visited
    /// 2. Try any unvisited neighbor (explore deeper)
    /// 3. If all neighbors visited, backtrack using dfs_stack
    ///
    /// Complexity: O(|V|) hops in the worst case for a connected graph
    ///
    /// Key insight: We use dfs_stack to remember the path we came from,
    /// allowing proper backtracking without infinite loops.
    fn traverse_graph_dfs(
        &self,
        current: &RoutingNode,
        packet: &mut PacketHeader,
    ) -> Option<RoutingDecision> {
        // Mark current node as visited (for DFS)
        packet.visited.insert(current.id.clone());

        // 1. Try any unvisited neighbor (explore deeper)
        // Sort neighbors for deterministic behavior
        let mut neighbors: Vec<&NodeId> = current.neighbors.iter().collect();
        neighbors.sort_by(|a, b| a.0.cmp(&b.0));

        for neighbor_id in neighbors {
            if !packet.visited.contains(neighbor_id) {
                // Push current node to stack before moving forward
                packet.dfs_stack.push(current.id.clone());
                return Some(RoutingDecision::Forward {
                    next_hop: neighbor_id.clone(),
                    mode: RoutingMode::Tree,
                });
            }
        }

        // 2. All neighbors visited - backtrack using stack
        if let Some(prev_node) = packet.dfs_stack.pop() {
            return Some(RoutingDecision::Forward {
                next_hop: prev_node,
                mode: RoutingMode::Tree,
            });
        }

        // Stack is empty and no unvisited neighbors - we've explored everything reachable
        // This should only happen if the graph is disconnected
        None
    }

    /// Pressure Mode: Use accumulated pressure to escape local minimum
    fn pressure_routing(&self, current: &RoutingNode, packet: &mut PacketHeader) -> RoutingDecision {
        // Calculate pressure-adjusted distances for each neighbor
        let mut best_neighbor: Option<NodeId> = None;
        let mut best_score = f64::INFINITY;

        for neighbor_id in &current.neighbors {
            // Base distance (gravity component)
            let distance = self.distance_to_target(neighbor_id, packet);

            // Pressure component: penalize previously visited nodes
            let pressure = packet
                .pressure_values
                .get(neighbor_id)
                .copied()
                .unwrap_or(0.0);

            // Combined score: lower is better
            // Nodes with high pressure (many visits) get higher scores, making them less attractive
            let score = distance + pressure;

            if score < best_score {
                best_score = score;
                best_neighbor = Some(neighbor_id.clone());
            }
        }

        match best_neighbor {
            Some(next_hop) => {
                // Update pressure for the current node
                let current_pressure = packet
                    .pressure_values
                    .get(&current.id)
                    .copied()
                    .unwrap_or(0.0);
                packet
                    .pressure_values
                    .insert(current.id.clone(), current_pressure + self.pressure_increment);

                // Decay all pressures
                for pressure in packet.pressure_values.values_mut() {
                    *pressure *= self.pressure_decay;
                }

                RoutingDecision::Forward {
                    next_hop,
                    mode: RoutingMode::Pressure,
                }
            }
            None => RoutingDecision::Failed {
                reason: "No neighbors available".to_string(),
            },
        }
    }

    /// Simulate packet delivery from source to destination
    pub fn simulate_delivery(
        &self,
        source: &NodeId,
        destination: &NodeId,
        target_coord: PoincareDiskPoint,
        max_ttl: u32,
    ) -> DeliveryResult {
        let mut packet = PacketHeader::new(
            source.clone(),
            destination.clone(),
            target_coord,
            max_ttl,
        );

        let mut current = source.clone();
        let mut path = vec![current.clone()];
        let mut hops = 0;
        let mut gravity_hops = 0;
        let mut pressure_hops = 0;
        let mut tree_hops = 0;

        loop {
            let decision = self.route(&current, &mut packet);

            match decision {
                RoutingDecision::Delivered => {
                    return DeliveryResult {
                        success: true,
                        hops,
                        gravity_hops,
                        pressure_hops,
                        tree_hops,
                        path,
                        failure_reason: None,
                    };
                }
                RoutingDecision::Forward { next_hop, mode } => {
                    hops += 1;
                    packet.ttl -= 1;
                    match mode {
                        RoutingMode::Gravity => gravity_hops += 1,
                        RoutingMode::Pressure => pressure_hops += 1,
                        RoutingMode::Tree => tree_hops += 1,
                        RoutingMode::ThorupZwick => tree_hops += 1, // Count TZ hops as tree hops
                        RoutingMode::HyperPress => gravity_hops += 1, // Count HYPER-PRESS as gravity
                    }
                    path.push(next_hop.clone());
                    current = next_hop;
                }
                RoutingDecision::Failed { reason } => {
                    return DeliveryResult {
                        success: false,
                        hops,
                        gravity_hops,
                        pressure_hops,
                        tree_hops,
                        path,
                        failure_reason: Some(reason),
                    };
                }
            }
        }
    }

    /// Get total number of nodes
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get total number of edges
    pub fn edge_count(&self) -> usize {
        self.nodes.values().map(|n| n.neighbors.len()).sum::<usize>() / 2
    }

    /// Get all node IDs
    pub fn node_ids(&self) -> Vec<NodeId> {
        self.nodes.keys().cloned().collect()
    }

    /// Get all edges as (NodeId, NodeId) pairs
    pub fn get_edges(&self) -> Vec<(NodeId, NodeId)> {
        let mut edges = Vec::new();
        for node in self.nodes.values() {
            for neighbor in &node.neighbors {
                if node.id.0 < neighbor.0 {
                    edges.push((node.id.clone(), neighbor.clone()));
                }
            }
        }
        edges
    }
}

impl Default for GPRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a packet delivery simulation
#[derive(Debug, Clone)]
pub struct DeliveryResult {
    pub success: bool,
    pub hops: u32,
    pub gravity_hops: u32,
    pub pressure_hops: u32,
    pub tree_hops: u32,
    pub path: Vec<NodeId>,
    pub failure_reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_network() -> GPRouter {
        let mut router = GPRouter::new();

        // Create a simple network
        //     1
        //    / \
        //   0---2---3
        //    \ /
        //     4

        let nodes = vec![
            ("0", 0.0, 0.0),
            ("1", 0.0, 0.3),
            ("2", 0.3, 0.0),
            ("3", 0.5, 0.0),
            ("4", 0.15, -0.2),
        ];

        for (id, x, y) in &nodes {
            let coord = RoutingCoordinate::new(PoincareDiskPoint::new(*x, *y).unwrap(), 0);
            router.add_node(RoutingNode::new(NodeId::new(*id), coord));
        }

        // Add edges
        router.add_edge(&NodeId::new("0"), &NodeId::new("1"));
        router.add_edge(&NodeId::new("0"), &NodeId::new("2"));
        router.add_edge(&NodeId::new("0"), &NodeId::new("4"));
        router.add_edge(&NodeId::new("1"), &NodeId::new("2"));
        router.add_edge(&NodeId::new("2"), &NodeId::new("3"));
        router.add_edge(&NodeId::new("2"), &NodeId::new("4"));

        router
    }

    #[test]
    fn test_gravity_routing_success() {
        let router = create_test_network();
        let source = NodeId::new("0");
        let dest = NodeId::new("3");

        let dest_coord = router.get_node(&dest).unwrap().coord.point;
        let result = router.simulate_delivery(&source, &dest, dest_coord, 10);

        assert!(result.success);
        assert!(result.hops <= 3);
        assert_eq!(result.path.first().unwrap(), &source);
        assert_eq!(result.path.last().unwrap(), &dest);
    }

    #[test]
    fn test_routing_self() {
        let router = create_test_network();
        let node = NodeId::new("0");
        let coord = router.get_node(&node).unwrap().coord.point;

        let result = router.simulate_delivery(&node, &node, coord, 10);

        assert!(result.success);
        assert_eq!(result.hops, 0);
    }

    #[test]
    fn test_ttl_expiry() {
        let router = create_test_network();
        let source = NodeId::new("0");
        let dest = NodeId::new("3");
        let dest_coord = router.get_node(&dest).unwrap().coord.point;

        // TTL = 1, not enough to reach destination
        let result = router.simulate_delivery(&source, &dest, dest_coord, 1);

        assert!(!result.success);
        assert!(result.failure_reason.is_some());
    }
}








