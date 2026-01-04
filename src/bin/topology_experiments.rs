//! Topology Experiments for DRFE-R
//!
//! Comprehensive experiments across multiple topology types:
//! - Barabási-Albert (BA) scale-free networks
//! - Watts-Strogatz (WS) small-world networks
//! - Grid networks
//! - Random (Erdős-Rényi) networks
//! - Real-world inspired topologies
//!
//! Collects 1000+ routing tests per configuration and measures:
//! - Success rate
//! - Average hop count
//! - Stretch ratio (actual hops / shortest path hops)

use drfe_r::coordinates::{NodeId, RoutingCoordinate};
use drfe_r::greedy_embedding::GreedyEmbedding;
use drfe_r::landmark_routing::{LandmarkRoutingConfig, LandmarkRoutingTable};
use drfe_r::routing::{GPRouter, RoutingNode};
use drfe_r::PoincareDiskPoint;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::time::Instant;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TopologyType {
    BarabasiAlbert,
    WattsStrogatz,
    Grid,
    Random,
    RealWorld,
}

impl std::fmt::Display for TopologyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TopologyType::BarabasiAlbert => write!(f, "Barabási-Albert"),
            TopologyType::WattsStrogatz => write!(f, "Watts-Strogatz"),
            TopologyType::Grid => write!(f, "Grid"),
            TopologyType::Random => write!(f, "Random"),
            TopologyType::RealWorld => write!(f, "Real-World"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyConfig {
    pub topology_type: TopologyType,
    pub num_nodes: usize,
    pub num_tests: usize,
    pub max_ttl: u32,
    pub seed: u64,
    pub routing_algorithm: RoutingAlgorithm,
    pub landmark_config: LandmarkRoutingConfig,
    // BA parameters
    pub ba_m: usize,
    // WS parameters
    pub ws_k: usize,
    pub ws_beta: f64,
    // Random parameters
    pub random_p: f64,
    // Optional real-world edge list
    pub real_world_path: Option<String>,
}

impl Default for TopologyConfig {
    fn default() -> Self {
        Self {
            topology_type: TopologyType::BarabasiAlbert,
            num_nodes: 100,
            num_tests: 1000,
            max_ttl: 100,
            seed: 42,
            routing_algorithm: RoutingAlgorithm::Baseline,
            landmark_config: LandmarkRoutingConfig::default(),
            ba_m: 3,
            ws_k: 6,
            ws_beta: 0.1,
            random_p: 0.05,
            real_world_path: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoutingAlgorithm {
    Baseline,
    Landmark,
}

impl std::fmt::Display for RoutingAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RoutingAlgorithm::Baseline => write!(f, "baseline"),
            RoutingAlgorithm::Landmark => write!(f, "landmark"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentResult {
    pub seed: u64,
    pub topology_type: TopologyType,
    pub num_nodes: usize,
    pub num_edges: usize,
    pub num_tests: usize,
    pub successful_deliveries: usize,
    pub failed_deliveries: usize,
    pub success_rate: f64,
    pub total_hops: u32,
    pub avg_hops: f64,
    pub total_optimal_hops: u32,
    pub avg_optimal_hops: f64,
    pub stretch_ratio: f64,
    pub max_stretch: f64,
    pub p95_stretch: f64,
    pub p99_stretch: f64,
    pub gravity_hops: u32,
    pub pressure_hops: u32,
    pub tree_hops: u32,
    pub ttl_failures: usize,
    pub no_path_failures: usize,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSummary {
    pub mean: f64,
    pub ci95: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentSummary {
    pub topology_type: TopologyType,
    pub success_rate: MetricSummary,
    pub avg_hops: MetricSummary,
    pub stretch_ratio: MetricSummary,
    pub p95_stretch: MetricSummary,
    pub p99_stretch: MetricSummary,
    pub max_stretch: MetricSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryReport {
    pub nodes_per_topology: usize,
    pub tests_per_topology: usize,
    pub seeds: Vec<u64>,
    pub summaries: Vec<ExperimentSummary>,
}

impl std::fmt::Display for ExperimentResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "=== {} Topology Results ===", self.topology_type)?;
        writeln!(f, "Network:")?;
        writeln!(f, "  Nodes:              {}", self.num_nodes)?;
        writeln!(f, "  Edges:              {}", self.num_edges)?;
        writeln!(f, "Routing Performance:")?;
        writeln!(f, "  Tests:              {}", self.num_tests)?;
        writeln!(f, "  Success rate:       {:.2}%", self.success_rate * 100.0)?;
        writeln!(f, "  Successful:         {}", self.successful_deliveries)?;
        writeln!(f, "  Failed:             {}", self.failed_deliveries)?;
        writeln!(f, "Hop Metrics:")?;
        writeln!(f, "  Avg hops:           {:.2}", self.avg_hops)?;
        writeln!(f, "  Avg optimal hops:   {:.2}", self.avg_optimal_hops)?;
        writeln!(f, "  Stretch ratio:      {:.3}", self.stretch_ratio)?;
        writeln!(f, "  Stretch p95/p99:    {:.3} / {:.3}", self.p95_stretch, self.p99_stretch)?;
        writeln!(f, "  Max stretch:        {:.3}", self.max_stretch)?;
        writeln!(f, "Mode Distribution:")?;
        writeln!(f, "  Gravity hops:       {}", self.gravity_hops)?;
        writeln!(f, "  Pressure hops:      {}", self.pressure_hops)?;
        writeln!(f, "  Tree hops:          {}", self.tree_hops)?;
        writeln!(f, "Failures:")?;
        writeln!(f, "  TTL exhausted:      {}", self.ttl_failures)?;
        writeln!(f, "  No path:            {}", self.no_path_failures)?;
        writeln!(f, "Time:                 {} ms", self.elapsed_ms)?;
        Ok(())
    }
}

/// Generate Barabási-Albert scale-free network
fn generate_barabasi_albert(config: &TopologyConfig) -> GPRouter {
    let mut rng = StdRng::seed_from_u64(config.seed);
    let mut degrees: Vec<usize> = vec![0; config.num_nodes];
    let mut adjacency_idx: Vec<Vec<usize>> = vec![Vec::new(); config.num_nodes];
    let nodes: Vec<NodeId> = (0..config.num_nodes)
        .map(|i| NodeId::new(format!("node_{}", i)))
        .collect();

    let m = config.ba_m.min(config.num_nodes - 1);

    // Initial complete graph
    for i in 0..m {
        for j in (i + 1)..m {
            adjacency_idx[i].push(j);
            adjacency_idx[j].push(i);
            degrees[i] += 1;
            degrees[j] += 1;
        }
    }

    // Preferential attachment
    for i in m..config.num_nodes {
        let total_degree: usize = degrees.iter().take(i).sum();
        if total_degree == 0 {
            adjacency_idx[i].push(0);
            adjacency_idx[0].push(i);
            degrees[i] += 1;
            degrees[0] += 1;
            continue;
        }

        let mut connected = HashSet::new();
        let mut attempts = 0;
        while connected.len() < m && attempts < 1000 {
            attempts += 1;
            let r_val = rng.gen::<f64>() * total_degree as f64;
            let mut cumsum = 0.0;
            for j in 0..i {
                cumsum += degrees[j] as f64;
                if cumsum >= r_val && !connected.contains(&j) {
                    adjacency_idx[i].push(j);
                    adjacency_idx[j].push(i);
                    degrees[i] += 1;
                    degrees[j] += 1;
                    connected.insert(j);
                    break;
                }
            }
        }
    }

    build_router_from_adjacency(config, &nodes, &adjacency_idx)
}

/// Generate Watts-Strogatz small-world network
fn generate_watts_strogatz(config: &TopologyConfig) -> GPRouter {
    let mut rng = StdRng::seed_from_u64(config.seed);
    let mut adjacency: HashMap<usize, HashSet<usize>> = HashMap::new();
    
    for i in 0..config.num_nodes {
        adjacency.insert(i, HashSet::new());
    }

    let k = config.ws_k.min(config.num_nodes - 1);
    
    // Ring lattice
    for i in 0..config.num_nodes {
        for j in 1..=(k / 2) {
            let neighbor = (i + j) % config.num_nodes;
            adjacency.get_mut(&i).unwrap().insert(neighbor);
            adjacency.get_mut(&neighbor).unwrap().insert(i);
        }
    }

    // Rewire with probability beta
    for i in 0..config.num_nodes {
        for j in 1..=(k / 2) {
            let original_neighbor = (i + j) % config.num_nodes;
            
            if rng.gen::<f64>() < config.ws_beta {
                if adjacency.get(&i).unwrap().contains(&original_neighbor) {
                    adjacency.get_mut(&i).unwrap().remove(&original_neighbor);
                    adjacency.get_mut(&original_neighbor).unwrap().remove(&i);

                    let mut new_neighbor = rng.gen_range(0..config.num_nodes);
                    while new_neighbor == i || adjacency.get(&i).unwrap().contains(&new_neighbor) {
                        new_neighbor = rng.gen_range(0..config.num_nodes);
                    }

                    adjacency.get_mut(&i).unwrap().insert(new_neighbor);
                    adjacency.get_mut(&new_neighbor).unwrap().insert(i);
                }
            }
        }
    }

    let nodes: Vec<NodeId> = (0..config.num_nodes)
        .map(|i| NodeId::new(format!("node_{}", i)))
        .collect();
    
    let adjacency_idx: Vec<Vec<usize>> = (0..config.num_nodes)
        .map(|i| adjacency.get(&i).unwrap().iter().copied().collect())
        .collect();

    build_router_from_adjacency(config, &nodes, &adjacency_idx)
}

/// Generate 2D Grid network
fn generate_grid(config: &TopologyConfig) -> GPRouter {
    let width = (config.num_nodes as f64).sqrt().ceil() as usize;
    let mut adjacency_idx: Vec<Vec<usize>> = vec![Vec::new(); config.num_nodes];

    for i in 0..config.num_nodes {
        let x = i % width;

        // Right neighbor
        if x + 1 < width {
            let neighbor = i + 1;
            if neighbor < config.num_nodes {
                adjacency_idx[i].push(neighbor);
                adjacency_idx[neighbor].push(i);
            }
        }

        // Bottom neighbor
        let neighbor = i + width;
        if neighbor < config.num_nodes {
            adjacency_idx[i].push(neighbor);
            adjacency_idx[neighbor].push(i);
        }
    }

    let nodes: Vec<NodeId> = (0..config.num_nodes)
        .map(|i| NodeId::new(format!("node_{}", i)))
        .collect();

    build_router_from_adjacency(config, &nodes, &adjacency_idx)
}

/// Generate Random (Erdős-Rényi) network
fn generate_random(config: &TopologyConfig) -> GPRouter {
    let mut rng = StdRng::seed_from_u64(config.seed);
    let mut adjacency_idx: Vec<Vec<usize>> = vec![Vec::new(); config.num_nodes];

    // Generate edges with probability p
    for i in 0..config.num_nodes {
        for j in (i + 1)..config.num_nodes {
            if rng.gen::<f64>() < config.random_p {
                adjacency_idx[i].push(j);
                adjacency_idx[j].push(i);
            }
        }
    }

    // Ensure connectivity
    let mut connected: HashSet<usize> = HashSet::new();
    connected.insert(0);
    
    for i in 1..config.num_nodes {
        if adjacency_idx[i].is_empty() {
            let connected_idx = *connected.iter().nth(rng.gen_range(0..connected.len())).unwrap();
            adjacency_idx[i].push(connected_idx);
            adjacency_idx[connected_idx].push(i);
        }
        connected.insert(i);
    }

    let nodes: Vec<NodeId> = (0..config.num_nodes)
        .map(|i| NodeId::new(format!("node_{}", i)))
        .collect();

    build_router_from_adjacency(config, &nodes, &adjacency_idx)
}

/// Generate Real-World inspired topology (combination of BA + community structure)
fn generate_real_world(config: &TopologyConfig) -> GPRouter {
    if let Some(path) = &config.real_world_path {
        match load_edge_list(path) {
            Ok((nodes, adjacency_idx)) => {
                return build_router_from_adjacency(config, &nodes, &adjacency_idx);
            }
            Err(err) => {
                println!("Failed to load edge list ({}). Falling back to synthetic: {}", path, err);
            }
        }
    }

    let mut rng = StdRng::seed_from_u64(config.seed);
    let num_communities = (config.num_nodes as f64).sqrt().ceil() as usize;     
    let nodes_per_community = config.num_nodes / num_communities;
    
    let mut adjacency_idx: Vec<Vec<usize>> = vec![Vec::new(); config.num_nodes];

    // Create communities with high internal connectivity (BA-like within each community)
    for comm in 0..num_communities {
        let start = comm * nodes_per_community;
        let end = if comm == num_communities - 1 {
            config.num_nodes
        } else {
            (comm + 1) * nodes_per_community
        };

        // Dense connections within community
        for i in start..end {
            for j in (i + 1)..end {
                if rng.gen::<f64>() < 0.4 {  // Increased from 0.3
                    adjacency_idx[i].push(j);
                    adjacency_idx[j].push(i);
                }
            }
        }
    }

    // Add inter-community links (more connections for better connectivity)
    let inter_links = config.num_nodes / 5;  // Increased from /10
    for _ in 0..inter_links {
        let i = rng.gen_range(0..config.num_nodes);
        let j = rng.gen_range(0..config.num_nodes);
        if i != j && !adjacency_idx[i].contains(&j) {
            adjacency_idx[i].push(j);
            adjacency_idx[j].push(i);
        }
    }

    // Ensure every node has at least 2 connections for better connectivity
    for i in 0..config.num_nodes {
        while adjacency_idx[i].len() < 2 {
            let j = rng.gen_range(0..config.num_nodes);
            if i != j && !adjacency_idx[i].contains(&j) {
                adjacency_idx[i].push(j);
                adjacency_idx[j].push(i);
            }
        }
    }

    let nodes: Vec<NodeId> = (0..config.num_nodes)
        .map(|i| NodeId::new(format!("node_{}", i)))
        .collect();

    build_router_from_adjacency(config, &nodes, &adjacency_idx)
}

fn load_edge_list(path: &str) -> Result<(Vec<NodeId>, Vec<Vec<usize>>), String> {
    let file = File::open(path).map_err(|e| format!("open failed: {}", e))?;
    let reader = BufReader::new(file);

    let mut id_map: HashMap<String, usize> = HashMap::new();
    let mut nodes: Vec<NodeId> = Vec::new();
    let mut adjacency_sets: Vec<HashSet<usize>> = Vec::new();

    for line in reader.lines() {
        let line = line.map_err(|e| format!("read failed: {}", e))?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('%') {
            continue;
        }

        let parts: Vec<&str> = trimmed
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|p| !p.is_empty())
            .collect();
        if parts.len() < 2 {
            continue;
        }

        let u = parts[0].to_string();
        let v = parts[1].to_string();
        if u == v {
            continue;
        }

        let u_idx = *id_map.entry(u.clone()).or_insert_with(|| {
            let idx = nodes.len();
            nodes.push(NodeId::new(u));
            adjacency_sets.push(HashSet::new());
            idx
        });
        let v_idx = *id_map.entry(v.clone()).or_insert_with(|| {
            let idx = nodes.len();
            nodes.push(NodeId::new(v));
            adjacency_sets.push(HashSet::new());
            idx
        });

        adjacency_sets[u_idx].insert(v_idx);
        adjacency_sets[v_idx].insert(u_idx);
    }

    if nodes.is_empty() {
        return Err("edge list contained no nodes".to_string());
    }

    let adjacency_idx = adjacency_sets
        .into_iter()
        .map(|set| set.into_iter().collect())
        .collect();

    Ok((nodes, adjacency_idx))
}

/// Build GPRouter from adjacency list using PIE embedding
fn build_router_from_adjacency(
    config: &TopologyConfig,
    nodes: &[NodeId],
    adjacency_idx: &[Vec<usize>],
) -> GPRouter {
    let mut router = GPRouter::new();

    // Prepare for embedding
    let mut adjacency: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for (i, neighbors) in adjacency_idx.iter().enumerate() {
        let neighbor_ids: Vec<NodeId> = neighbors
            .iter()
            .map(|&j| nodes[j].clone())
            .collect();
        adjacency.insert(nodes[i].clone(), neighbor_ids);
    }

    // Embed using PIE
    let embedder = GreedyEmbedding::new();
    let embedding_result = embedder.embed(&adjacency).expect("Embedding failed");

    // Build tree parent map
    let mut tree_parent: HashMap<NodeId, Option<NodeId>> = HashMap::new();
    tree_parent.insert(embedding_result.root.clone(), None);
    for (parent_id, children) in &embedding_result.tree_children {
        for child_id in children {
            tree_parent.insert(child_id.clone(), Some(parent_id.clone()));
        }
    }

    // Create nodes
    for node_id in nodes {
        let point = embedding_result
            .coordinates
            .get(node_id)
            .copied()
            .unwrap_or_else(PoincareDiskPoint::origin);
        let coord = RoutingCoordinate::new(point, 0);
        let mut routing_node = RoutingNode::new(node_id.clone(), coord);
        
        let parent = tree_parent.get(node_id).cloned().flatten();
        let children = embedding_result
            .tree_children
            .get(node_id)
            .cloned()
            .unwrap_or_default();
        routing_node.set_tree_info(parent, children);
        
        router.add_node(routing_node);
    }

    // Add edges
    for (i, neighbors) in adjacency_idx.iter().enumerate() {
        for &j in neighbors {
            if i < j {
                router.add_edge(&nodes[i], &nodes[j]);
            }
        }
    }

    if config.routing_algorithm == RoutingAlgorithm::Landmark {
        match LandmarkRoutingTable::build(&adjacency, &config.landmark_config) {
            Ok(table) => {
                router.enable_landmark_routing(table, config.landmark_config.clone());
            }
            Err(err) => {
                println!(
                    "Failed to build landmark routing table: {}. Using baseline routing.",
                    err
                );
            }
        }
    }

    router
}

/// Calculate shortest path using BFS
fn bfs_shortest_path(router: &GPRouter, start: &NodeId, end: &NodeId) -> Option<u32> {
    let mut visited = HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back((start.clone(), 0));
    visited.insert(start.clone());

    while let Some((current, dist)) = queue.pop_front() {
        if &current == end {
            return Some(dist);
        }

        if let Some(node) = router.get_node(&current) {
            for neighbor in &node.neighbors {
                if !visited.contains(neighbor) {
                    visited.insert(neighbor.clone());
                    queue.push_back((neighbor.clone(), dist + 1));
                }
            }
        }
    }
    None
}

/// Run routing experiments on a given topology
fn run_experiment(config: &TopologyConfig) -> ExperimentResult {
    println!("Generating {} topology with {} nodes...", config.topology_type, config.num_nodes);
    let gen_start = Instant::now();
    
    let router = match config.topology_type {
        TopologyType::BarabasiAlbert => generate_barabasi_albert(config),
        TopologyType::WattsStrogatz => generate_watts_strogatz(config),
        TopologyType::Grid => generate_grid(config),
        TopologyType::Random => generate_random(config),
        TopologyType::RealWorld => generate_real_world(config),
    };
    
    let gen_time = gen_start.elapsed().as_millis();
    println!("  Generated in {} ms", gen_time);
    println!("  Nodes: {}, Edges: {}", router.node_count(), router.edge_count());

    let node_ids = router.node_ids();
    let actual_nodes = node_ids.len();

    println!("Running {} routing tests...", config.num_tests);
    let test_start = Instant::now();

    let mut rng = StdRng::seed_from_u64(config.seed + 1000);

    let mut successful = 0;
    let mut failed = 0;
    let mut total_hops = 0u32;
    let mut gravity_hops = 0u32;
    let mut pressure_hops = 0u32;
    let mut tree_hops = 0u32;
    let mut total_optimal_hops = 0u32;
    let mut ttl_failures = 0;
    let mut no_path_failures = 0;
    let mut stretch_samples: Vec<f64> = Vec::with_capacity(config.num_tests);
    let mut max_stretch = 0.0f64;

    for _ in 0..config.num_tests {
        let src_idx = rng.gen_range(0..actual_nodes);
        let mut dst_idx = rng.gen_range(0..actual_nodes);
        while dst_idx == src_idx {
            dst_idx = rng.gen_range(0..actual_nodes);
        }

        let source = &node_ids[src_idx];
        let dest = &node_ids[dst_idx];

        if let Some(dest_node) = router.get_node(dest) {
            let result = router.simulate_delivery(
                source,
                dest,
                dest_node.coord.point,
                config.max_ttl,
            );

            if result.success {
                successful += 1;
                total_hops += result.hops;
                gravity_hops += result.gravity_hops;
                pressure_hops += result.pressure_hops;
                tree_hops += result.tree_hops;

                if let Some(opt) = bfs_shortest_path(&router, source, dest) {
                    total_optimal_hops += opt;
                    if opt > 0 {
                        let stretch = result.hops as f64 / opt as f64;
                        stretch_samples.push(stretch);
                        if stretch > max_stretch {
                            max_stretch = stretch;
                        }
                    }
                }
            } else {
                failed += 1;
                if let Some(ref reason) = result.failure_reason {
                    if reason.contains("TTL") {
                        ttl_failures += 1;
                    } else {
                        no_path_failures += 1;
                    }
                }
            }
        } else {
            failed += 1;
            no_path_failures += 1;
        }
    }

    let elapsed = test_start.elapsed().as_millis();
    
    let avg_hops = if successful > 0 {
        total_hops as f64 / successful as f64
    } else {
        0.0
    };

    let avg_optimal_hops = if successful > 0 {
        total_optimal_hops as f64 / successful as f64
    } else {
        0.0
    };

    let stretch_ratio = if total_optimal_hops > 0 {
        total_hops as f64 / total_optimal_hops as f64
    } else {
        0.0
    };
    let (p95_stretch, p99_stretch) = stretch_percentiles(&stretch_samples);

    ExperimentResult {
        seed: config.seed,
        topology_type: config.topology_type,
        num_nodes: actual_nodes,
        num_edges: router.edge_count(),
        num_tests: config.num_tests,
        successful_deliveries: successful,
        failed_deliveries: failed,
        success_rate: successful as f64 / config.num_tests as f64,
        total_hops,
        avg_hops,
        total_optimal_hops,
        avg_optimal_hops,
        stretch_ratio,
        max_stretch,
        p95_stretch,
        p99_stretch,
        gravity_hops,
        pressure_hops,
        tree_hops,
        ttl_failures,
        no_path_failures,
        elapsed_ms: elapsed,
    }
}

fn stretch_percentiles(samples: &[f64]) -> (f64, f64) {
    if samples.is_empty() {
        return (0.0, 0.0);
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p95 = percentile_sorted(&sorted, 0.95);
    let p99 = percentile_sorted(&sorted, 0.99);
    (p95, p99)
}

fn percentile_sorted(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn mean_std(values: &[f64]) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0);
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    if values.len() < 2 {
        return (mean, 0.0);
    }
    let var = values
        .iter()
        .map(|v| {
            let diff = v - mean;
            diff * diff
        })
        .sum::<f64>()
        / (values.len() as f64 - 1.0);
    (mean, var.sqrt())
}

fn t_critical_95(df: usize) -> f64 {
    match df {
        1 => 12.706,
        2 => 4.303,
        3 => 3.182,
        4 => 2.776,
        5 => 2.571,
        6 => 2.447,
        7 => 2.365,
        8 => 2.306,
        9 => 2.262,
        10 => 2.228,
        11 => 2.201,
        12 => 2.179,
        13 => 2.160,
        14 => 2.145,
        15 => 2.131,
        16 => 2.120,
        17 => 2.110,
        18 => 2.101,
        19 => 2.093,
        20 => 2.086,
        21 => 2.080,
        22 => 2.074,
        23 => 2.069,
        24 => 2.064,
        25 => 2.060,
        26 => 2.056,
        27 => 2.052,
        28 => 2.048,
        29 => 2.045,
        30 => 2.042,
        _ => 1.96,
    }
}

fn metric_summary(values: &[f64]) -> MetricSummary {
    let (mean, std_dev) = mean_std(values);
    if values.len() < 2 {
        return MetricSummary { mean, ci95: 0.0 };
    }
    let t = t_critical_95(values.len() - 1);
    let ci95 = t * std_dev / (values.len() as f64).sqrt();
    MetricSummary { mean, ci95 }
}

fn summarize_results(results: &[ExperimentResult]) -> Vec<ExperimentSummary> {
    let mut by_key: HashMap<String, Vec<&ExperimentResult>> = HashMap::new();
    for result in results {
        let key = format!("{}", result.topology_type);
        by_key.entry(key).or_default().push(result);
    }

    let mut keys: Vec<String> = by_key.keys().cloned().collect();
    keys.sort();

    let mut summaries = Vec::new();
    for key in keys {
        let items = &by_key[&key];
        if items.is_empty() {
            continue;
        }
        let first = items[0];
        let values = |f: fn(&ExperimentResult) -> f64| {
            items.iter().map(|r| f(r)).collect::<Vec<f64>>()
        };

        summaries.push(ExperimentSummary {
            topology_type: first.topology_type,
            success_rate: metric_summary(&values(|r| r.success_rate)),
            avg_hops: metric_summary(&values(|r| r.avg_hops)),
            stretch_ratio: metric_summary(&values(|r| r.stretch_ratio)),
            p95_stretch: metric_summary(&values(|r| r.p95_stretch)),
            p99_stretch: metric_summary(&values(|r| r.p99_stretch)),
            max_stretch: metric_summary(&values(|r| r.max_stretch)),
        });
    }

    summaries
}

fn derive_summary_path(output_file: &str) -> String {
    if let Some(stripped) = output_file.strip_suffix(".json") {
        format!("{}_summary.json", stripped)
    } else {
        format!("{}_summary.json", output_file)
    }
}

fn parse_seed_list(input: &str) -> Vec<u64> {
    input
        .split(',')
        .filter_map(|s| s.trim().parse::<u64>().ok())
        .collect()
}

fn main() {
    println!("DRFE-R Topology Experiments");
    println!("===========================\n");

    let args: Vec<String> = std::env::args().collect();
    
    let mut num_nodes = 100;
    let mut num_tests = 1000;
    let mut output_file = "topology_experiments.json".to_string();
    let mut seed = 42u64;
    let mut seeds_override: Option<Vec<u64>> = None;
    let mut summary_output: Option<String> = None;
    let mut edge_list_path: Option<String> = None;
    let mut routing_algorithm = RoutingAlgorithm::Baseline;
    let mut landmark_config = LandmarkRoutingConfig::default();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--nodes" | "-n" => {
                if i + 1 < args.len() {
                    num_nodes = args[i + 1].parse().unwrap_or(100);
                    i += 1;
                }
            }
            "--tests" | "-t" => {
                if i + 1 < args.len() {
                    num_tests = args[i + 1].parse().unwrap_or(1000);
                    i += 1;
                }
            }
            "--output" | "-o" => {
                if i + 1 < args.len() {
                    output_file = args[i + 1].clone();
                    i += 1;
                }
            }
            "--seed" => {
                if i + 1 < args.len() {
                    seed = args[i + 1].parse().unwrap_or(seed);
                    i += 1;
                }
            }
            "--seeds" => {
                if i + 1 < args.len() {
                    let parsed = parse_seed_list(&args[i + 1]);
                    if !parsed.is_empty() {
                        seeds_override = Some(parsed);
                    }
                    i += 1;
                }
            }
            "--summary-output" => {
                if i + 1 < args.len() {
                    summary_output = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--edge-list" => {
                if i + 1 < args.len() {
                    edge_list_path = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--routing" => {
                if i + 1 < args.len() {
                    routing_algorithm = match args[i + 1].as_str() {
                        "baseline" => RoutingAlgorithm::Baseline,
                        "landmark" | "real" => RoutingAlgorithm::Landmark,
                        _ => routing_algorithm,
                    };
                    i += 1;
                }
            }
            "--landmarks" => {
                if i + 1 < args.len() {
                    if let Ok(count) = args[i + 1].parse::<usize>() {
                        if count > 0 {
                            landmark_config.num_landmarks = Some(count);
                        }
                    }
                    i += 1;
                }
            }
            "--lookahead-depth" => {
                if i + 1 < args.len() {
                    landmark_config.lookahead_depth =
                        args[i + 1].parse().unwrap_or(landmark_config.lookahead_depth);
                    i += 1;
                }
            }
            "--lookahead-max" => {
                if i + 1 < args.len() {
                    landmark_config.lookahead_max_nodes =
                        args[i + 1].parse().unwrap_or(landmark_config.lookahead_max_nodes);
                    i += 1;
                }
            }
            "--landmark-weight" => {
                if i + 1 < args.len() {
                    landmark_config.landmark_weight =
                        args[i + 1].parse().unwrap_or(landmark_config.landmark_weight);
                    i += 1;
                }
            }
            "--hyper-weight" => {
                if i + 1 < args.len() {
                    landmark_config.hyperbolic_weight =
                        args[i + 1].parse().unwrap_or(landmark_config.hyperbolic_weight);
                    i += 1;
                }
            }
            "--help" | "-h" => {
                println!("Usage: topology_experiments [OPTIONS]");
                println!();
                println!("Options:");
                println!("  -n, --nodes NUM    Number of nodes (default: 100)");
                println!("  -t, --tests NUM    Number of routing tests per topology (default: 1000)");
                println!("  -o, --output FILE  Output JSON file (default: topology_experiments.json)");
                println!("      --seed NUM     Random seed (default: 42)");
                println!("      --seeds LIST   Comma-separated seeds (overrides --seed)");
                println!("      --summary-output FILE  Summary JSON output (optional)");
                println!("      --edge-list FILE  Real-world edge list file for Real-World topology");
                println!("      --routing MODE  Routing algorithm: baseline, landmark");
                println!("      --landmarks NUM  Number of landmarks (default: auto)");
                println!("      --lookahead-depth NUM  Lookahead BFS depth (default: 3)");
                println!("      --lookahead-max NUM  Lookahead BFS node cap (default: 5000)");
                println!("      --landmark-weight NUM  Landmark distance weight (default: 1.0)");
                println!("      --hyper-weight NUM  Hyperbolic distance weight (default: 0.15)");
                println!("  -h, --help         Show this help");
                return;
            }
            _ => {}
        }
        i += 1;
    }

    println!("Configuration:");
    println!("  Nodes per topology: {}", num_nodes);
    println!("  Tests per topology: {}", num_tests);
    println!("  Output file:        {}", output_file);
    if let Some(path) = &edge_list_path {
        println!("  Real-world edge list: {}", path);
    }
    println!("  Routing algorithm:  {}", routing_algorithm);
    if routing_algorithm == RoutingAlgorithm::Landmark {
        let landmark_count = landmark_config
            .num_landmarks
            .map(|v| v.to_string())
            .unwrap_or_else(|| "auto".to_string());
        println!("  Landmarks:          {}", landmark_count);
        println!("  Lookahead depth:    {}", landmark_config.lookahead_depth);
        println!("  Lookahead max:      {}", landmark_config.lookahead_max_nodes);
        println!("  Landmark weight:    {}", landmark_config.landmark_weight);
        println!("  Hyper weight:       {}", landmark_config.hyperbolic_weight);
    }
    let seeds = seeds_override.unwrap_or_else(|| vec![seed]);
    println!("  Seeds:              {:?}", seeds);
    if let Some(path) = &summary_output {
        println!("  Summary output:     {}", path);
    }
    println!();

    let topologies = vec![
        TopologyType::BarabasiAlbert,
        TopologyType::WattsStrogatz,
        TopologyType::Grid,
        TopologyType::Random,
        TopologyType::RealWorld,
    ];

    let mut all_results = Vec::new();

    for current_seed in &seeds {
        if seeds.len() > 1 {
            println!("\nSeed {}", current_seed);
        }

        for topology_type in &topologies {
            println!("\n{}", "=".repeat(60));
            println!("Experiment: {} Topology", topology_type);
            println!("{}\n", "=".repeat(60));

            let config = TopologyConfig {
                topology_type: *topology_type,
                num_nodes,
                num_tests,
                max_ttl: 100,
                seed: *current_seed,
                routing_algorithm,
                landmark_config: landmark_config.clone(),
                ba_m: 3,
                ws_k: 6,
                ws_beta: 0.1,
                random_p: 0.05,
                real_world_path: edge_list_path.clone(),
            };

            let result = run_experiment(&config);
            println!("\n{}", result);

            all_results.push(result);
        }
    }

    // Save results to JSON
    println!("\n{}", "=".repeat(60));
    println!("Saving results to {}...", output_file);
    
    let json = serde_json::to_string_pretty(&all_results).expect("Failed to serialize results");
    let mut file = File::create(&output_file).expect("Failed to create output file");
    file.write_all(json.as_bytes()).expect("Failed to write results");

    println!("Results saved successfully!");

    let summary_path = summary_output
        .or_else(|| if seeds.len() > 1 { Some(derive_summary_path(&output_file)) } else { None });
    if let Some(path) = summary_path {
        let summaries = summarize_results(&all_results);
        let report = SummaryReport {
            nodes_per_topology: num_nodes,
            tests_per_topology: num_tests,
            seeds,
            summaries,
        };
        let summary_json = serde_json::to_string_pretty(&report).expect("Failed to serialize summary");
        let mut summary_file = File::create(&path).expect("Failed to create summary file");
        summary_file
            .write_all(summary_json.as_bytes())
            .expect("Failed to write summary");
        println!("Summary saved to {}", path);
    }

    // Summary
    println!("\n{}", "=".repeat(60));
    println!("Summary");
    println!("{}\n", "=".repeat(60));
    
    for result in &all_results {
        println!("{:20} | Success: {:6.2}% | Avg Hops: {:5.2} | Stretch: {:.3}",
            format!("{}", result.topology_type),
            result.success_rate * 100.0,
            result.avg_hops,
            result.stretch_ratio
        );
    }
}
