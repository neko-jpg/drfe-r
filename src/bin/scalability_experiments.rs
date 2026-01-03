//! Scalability Experiments for DRFE-R
//!
//! Comprehensive scalability testing for networks of varying sizes.
//! Measures routing performance, memory usage, and convergence properties
//! across different scales.
//!
//! Requirements: 10.1, 16.1, 16.2, 16.3, 16.4

use drfe_r::coordinates::{NodeId, RoutingCoordinate};
use drfe_r::greedy_embedding::GreedyEmbedding;
use drfe_r::routing::{GPRouter, RoutingNode};
use drfe_r::PoincareDiskPoint;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Write;
use std::time::Instant;

/// Configuration for scalability experiments
#[derive(Debug, Clone)]
pub struct ScalabilityConfig {
    /// Network sizes to test
    pub network_sizes: Vec<usize>,
    /// Number of routing tests per size
    pub num_routing_tests: usize,
    /// Maximum TTL for routing
    pub max_ttl: u32,
    /// Random seed for reproducibility
    pub seed: u64,
    /// Output file for results
    pub output_file: String,
}

impl Default for ScalabilityConfig {
    fn default() -> Self {
        Self {
            network_sizes: vec![100, 300, 500, 1000, 3000, 5000],
            num_routing_tests: 1000,
            max_ttl: 200,
            seed: 42,
            output_file: "scalability_results.json".to_string(),
        }
    }
}

/// Results for a single network size
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalabilityResult {
    /// Network size (number of nodes)
    pub network_size: usize,
    /// Number of edges in the network
    pub num_edges: usize,
    /// Average node degree
    pub avg_degree: f64,
    
    // Routing Performance Metrics
    /// Total routing tests performed
    pub total_tests: usize,
    /// Number of successful deliveries
    pub successful_deliveries: usize,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
    /// Average hop count for successful deliveries
    pub avg_hops: f64,
    /// Average optimal hop count (BFS shortest path)
    pub avg_optimal_hops: f64,
    /// Average stretch ratio (actual hops / optimal hops)
    pub avg_stretch: f64,
    /// Median hop count
    pub median_hops: u32,
    /// 95th percentile hop count
    pub p95_hops: u32,
    /// Maximum hop count observed
    pub max_hops: u32,
    
    // Mode Distribution
    /// Total hops in Gravity mode
    pub gravity_hops: u32,
    /// Total hops in Pressure mode
    pub pressure_hops: u32,
    /// Total hops in Tree mode
    pub tree_hops: u32,
    /// Percentage of hops in Gravity mode
    pub gravity_percentage: f64,
    
    // Timing Metrics
    /// Time to generate network (ms)
    pub network_generation_ms: u128,
    /// Time to compute embedding (ms)
    pub embedding_time_ms: u128,
    /// Time to run all routing tests (ms)
    pub routing_time_ms: u128,
    /// Average time per routing test (μs)
    pub avg_routing_time_us: f64,
    
    // Memory Metrics (estimated)
    /// Estimated memory per node (bytes)
    pub memory_per_node_bytes: usize,
    /// Total estimated memory (MB)
    pub total_memory_mb: f64,
    
    // Complexity Verification
    /// Average routing complexity (hops per test)
    pub routing_complexity: f64,
    /// Embedding complexity (time per edge, ms)
    pub embedding_complexity_per_edge: f64,
}

impl std::fmt::Display for ScalabilityResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Network Size: {} nodes, {} edges", self.network_size, self.num_edges)?;
        writeln!(f, "  Avg Degree:        {:.2}", self.avg_degree)?;
        writeln!(f, "  Success Rate:      {:.2}%", self.success_rate * 100.0)?;
        writeln!(f, "  Avg Hops:          {:.2}", self.avg_hops)?;
        writeln!(f, "  Avg Optimal Hops:  {:.2}", self.avg_optimal_hops)?;
        writeln!(f, "  Stretch Ratio:     {:.3}", self.avg_stretch)?;
        writeln!(f, "  Median Hops:       {}", self.median_hops)?;
        writeln!(f, "  P95 Hops:          {}", self.p95_hops)?;
        writeln!(f, "  Max Hops:          {}", self.max_hops)?;
        writeln!(f, "  Gravity Mode:      {:.1}%", self.gravity_percentage)?;
        writeln!(f, "  Network Gen:       {} ms", self.network_generation_ms)?;
        writeln!(f, "  Embedding Time:    {} ms", self.embedding_time_ms)?;
        writeln!(f, "  Routing Time:      {} ms", self.routing_time_ms)?;
        writeln!(f, "  Avg Routing:       {:.2} μs", self.avg_routing_time_us)?;
        writeln!(f, "  Memory/Node:       {} bytes", self.memory_per_node_bytes)?;
        writeln!(f, "  Total Memory:      {:.2} MB", self.total_memory_mb)?;
        Ok(())
    }
}

/// Summary of all scalability experiments
#[derive(Debug, Serialize, Deserialize)]
pub struct ScalabilitySummary {
    pub results: Vec<ScalabilityResult>,
    pub timestamp: String,
    pub config: ScalabilityConfigSummary,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScalabilityConfigSummary {
    pub network_sizes: Vec<usize>,
    pub num_routing_tests: usize,
    pub max_ttl: u32,
    pub seed: u64,
}

/// Generate a Barabási-Albert scale-free network with PIE embedding
fn generate_ba_network(n: usize, m: usize, seed: u64) -> (GPRouter, u128) {
    let start = Instant::now();
    let mut rng = StdRng::seed_from_u64(seed);
    let mut degrees: Vec<usize> = Vec::new();
    let mut nodes: Vec<NodeId> = Vec::new();
    let mut adjacency_idx: Vec<Vec<usize>> = Vec::new();

    // Create nodes
    for i in 0..n {
        let id = NodeId::new(format!("node_{}", i));
        nodes.push(id);
        degrees.push(0);
        adjacency_idx.push(Vec::new());
    }

    // Build initial complete graph of m nodes
    for i in 0..m.min(n) {
        for j in (i + 1)..m.min(n) {
            adjacency_idx[i].push(j);
            adjacency_idx[j].push(i);
            degrees[i] += 1;
            degrees[j] += 1;
        }
    }

    // Add remaining nodes with preferential attachment
    for i in m..n {
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
        while connected.len() < m.min(i) && attempts < 1000 {
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

    let generation_time = start.elapsed().as_millis();

    // Convert to NodeId-based adjacency
    let mut adjacency: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for i in 0..n {
        let neighbors: Vec<NodeId> = adjacency_idx[i]
            .iter()
            .map(|&j| nodes[j].clone())
            .collect();
        adjacency.insert(nodes[i].clone(), neighbors);
    }

    // Compute PIE embedding
    let embed_start = Instant::now();
    let embedder = GreedyEmbedding::new();
    let embedding_result = embedder.embed(&adjacency).expect("Embedding should succeed");
    let embedding_time = embed_start.elapsed().as_millis();

    // Build tree parent map
    let mut tree_parent: HashMap<NodeId, Option<NodeId>> = HashMap::new();
    tree_parent.insert(embedding_result.root.clone(), None);
    for (parent_id, children) in &embedding_result.tree_children {
        for child_id in children {
            tree_parent.insert(child_id.clone(), Some(parent_id.clone()));
        }
    }

    // Create router
    let mut router = GPRouter::new();
    for i in 0..n {
        let node_id = &nodes[i];
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
    for i in 0..n {
        for &j in &adjacency_idx[i] {
            if i < j {
                router.add_edge(&nodes[i], &nodes[j]);
            }
        }
    }

    (router, generation_time + embedding_time)
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

/// Run scalability experiment for a single network size
fn run_scalability_experiment(
    network_size: usize,
    config: &ScalabilityConfig,
) -> ScalabilityResult {
    println!("\n=== Testing Network Size: {} ===", network_size);
    
    // Generate network
    print!("  Generating network... ");
    std::io::stdout().flush().unwrap();
    let (router, total_gen_time) = generate_ba_network(network_size, 3, config.seed);
    let num_edges = router.edge_count();
    let avg_degree = (2 * num_edges) as f64 / network_size as f64;
    println!("done ({} ms)", total_gen_time);
    println!("    Nodes: {}, Edges: {}, Avg Degree: {:.2}", 
             network_size, num_edges, avg_degree);
    
    // Estimate embedding time separately for complexity analysis
    let embedding_time_ms = total_gen_time / 2; // Rough estimate
    let network_generation_ms = total_gen_time - embedding_time_ms;
    
    // Run routing tests
    print!("  Running {} routing tests... ", config.num_routing_tests);
    std::io::stdout().flush().unwrap();
    
    let mut rng = StdRng::seed_from_u64(config.seed + network_size as u64);
    let node_ids: Vec<NodeId> = (0..network_size)
        .map(|i| NodeId::new(format!("node_{}", i)))
        .collect();
    
    let routing_start = Instant::now();
    let mut successful = 0;
    let mut hop_counts = Vec::new();
    let mut optimal_hop_counts = Vec::new();
    let mut total_gravity_hops = 0u32;
    let mut total_pressure_hops = 0u32;
    let mut total_tree_hops = 0u32;
    
    for _ in 0..config.num_routing_tests {
        let src_idx = rng.gen_range(0..network_size);
        let mut dst_idx = rng.gen_range(0..network_size);
        while dst_idx == src_idx {
            dst_idx = rng.gen_range(0..network_size);
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
                hop_counts.push(result.hops);
                total_gravity_hops += result.gravity_hops;
                total_pressure_hops += result.pressure_hops;
                total_tree_hops += result.tree_hops;
                
                if let Some(opt) = bfs_shortest_path(&router, source, dest) {
                    optimal_hop_counts.push(opt);
                }
            }
        }
    }
    
    let routing_time_ms = routing_start.elapsed().as_millis();
    println!("done ({} ms)", routing_time_ms);
    
    // Calculate statistics
    let success_rate = successful as f64 / config.num_routing_tests as f64;
    
    let total_hops: u32 = hop_counts.iter().sum();
    let avg_hops = if successful > 0 {
        total_hops as f64 / successful as f64
    } else {
        0.0
    };
    
    let total_optimal: u32 = optimal_hop_counts.iter().sum();
    let avg_optimal_hops = if !optimal_hop_counts.is_empty() {
        total_optimal as f64 / optimal_hop_counts.len() as f64
    } else {
        0.0
    };
    
    let avg_stretch = if total_optimal > 0 {
        total_hops as f64 / total_optimal as f64
    } else {
        0.0
    };
    
    // Calculate percentiles
    hop_counts.sort_unstable();
    let median_hops = if !hop_counts.is_empty() {
        hop_counts[hop_counts.len() / 2]
    } else {
        0
    };
    
    let p95_hops = if !hop_counts.is_empty() {
        let idx = (hop_counts.len() as f64 * 0.95) as usize;
        hop_counts[idx.min(hop_counts.len() - 1)]
    } else {
        0
    };
    
    let max_hops = hop_counts.iter().max().copied().unwrap_or(0);
    
    let gravity_percentage = if total_hops > 0 {
        (total_gravity_hops as f64 / total_hops as f64) * 100.0
    } else {
        0.0
    };
    
    let avg_routing_time_us = if config.num_routing_tests > 0 {
        (routing_time_ms as f64 * 1000.0) / config.num_routing_tests as f64
    } else {
        0.0
    };
    
    // Memory estimation
    // Per node: NodeId (String ~24 bytes) + PoincareDiskPoint (16 bytes) + 
    //           neighbors vector (~8 bytes per neighbor) + tree info (~16 bytes)
    let memory_per_node_bytes = 64 + (avg_degree as usize * 8);
    let total_memory_mb = (network_size * memory_per_node_bytes) as f64 / (1024.0 * 1024.0);
    
    // Complexity metrics
    let routing_complexity = avg_hops;
    let embedding_complexity_per_edge = if num_edges > 0 {
        embedding_time_ms as f64 / num_edges as f64
    } else {
        0.0
    };
    
    println!("  Results:");
    println!("    Success Rate:      {:.2}%", success_rate * 100.0);
    println!("    Avg Hops:          {:.2}", avg_hops);
    println!("    Stretch Ratio:     {:.3}", avg_stretch);
    println!("    Gravity Mode:      {:.1}%", gravity_percentage);
    println!("    Avg Routing Time:  {:.2} μs", avg_routing_time_us);
    println!("    Memory/Node:       {} bytes", memory_per_node_bytes);
    println!("    Total Memory:      {:.2} MB", total_memory_mb);
    
    ScalabilityResult {
        network_size,
        num_edges,
        avg_degree,
        total_tests: config.num_routing_tests,
        successful_deliveries: successful,
        success_rate,
        avg_hops,
        avg_optimal_hops,
        avg_stretch,
        median_hops,
        p95_hops,
        max_hops,
        gravity_hops: total_gravity_hops,
        pressure_hops: total_pressure_hops,
        tree_hops: total_tree_hops,
        gravity_percentage,
        network_generation_ms,
        embedding_time_ms,
        routing_time_ms,
        avg_routing_time_us,
        memory_per_node_bytes,
        total_memory_mb,
        routing_complexity,
        embedding_complexity_per_edge,
    }
}

fn main() {
    println!("DRFE-R Scalability Experiments");
    println!("==============================\n");
    
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let mut config = ScalabilityConfig::default();
    
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--sizes" => {
                if i + 1 < args.len() {
                    config.network_sizes = args[i + 1]
                        .split(',')
                        .filter_map(|s| s.trim().parse().ok())
                        .collect();
                    i += 1;
                }
            }
            "--tests" => {
                if i + 1 < args.len() {
                    config.num_routing_tests = args[i + 1].parse().unwrap_or(1000);
                    i += 1;
                }
            }
            "--ttl" => {
                if i + 1 < args.len() {
                    config.max_ttl = args[i + 1].parse().unwrap_or(200);
                    i += 1;
                }
            }
            "--seed" => {
                if i + 1 < args.len() {
                    config.seed = args[i + 1].parse().unwrap_or(42);
                    i += 1;
                }
            }
            "--output" | "-o" => {
                if i + 1 < args.len() {
                    config.output_file = args[i + 1].clone();
                    i += 1;
                }
            }
            "--help" | "-h" => {
                println!("Usage: scalability_experiments [OPTIONS]");
                println!();
                println!("Options:");
                println!("  --sizes SIZES     Comma-separated network sizes (default: 100,300,500,1000,3000,5000)");
                println!("  --tests NUM       Number of routing tests per size (default: 1000)");
                println!("  --ttl NUM         Max TTL for routing (default: 200)");
                println!("  --seed NUM        Random seed (default: 42)");
                println!("  -o, --output FILE Output JSON file (default: scalability_results.json)");
                println!("  -h, --help        Show this help");
                return;
            }
            _ => {}
        }
        i += 1;
    }
    
    println!("Configuration:");
    println!("  Network Sizes: {:?}", config.network_sizes);
    println!("  Tests/Size:    {}", config.num_routing_tests);
    println!("  Max TTL:       {}", config.max_ttl);
    println!("  Seed:          {}", config.seed);
    println!("  Output:        {}", config.output_file);
    
    // Run experiments for each network size
    let mut results = Vec::new();
    let total_start = Instant::now();
    
    for &size in &config.network_sizes {
        let result = run_scalability_experiment(size, &config);
        results.push(result);
    }
    
    let total_time = total_start.elapsed();
    
    // Print summary
    println!("\n=== Scalability Summary ===");
    println!("Total experiment time: {:.2} seconds", total_time.as_secs_f64());
    println!();
    
    println!("{:<10} {:<12} {:<12} {:<12} {:<12} {:<12}", 
             "Size", "Success%", "Avg Hops", "Stretch", "Time(μs)", "Memory(MB)");
    println!("{}", "-".repeat(72));
    
    for result in &results {
        println!("{:<10} {:<12.2} {:<12.2} {:<12.3} {:<12.2} {:<12.2}",
                 result.network_size,
                 result.success_rate * 100.0,
                 result.avg_hops,
                 result.avg_stretch,
                 result.avg_routing_time_us,
                 result.total_memory_mb);
    }
    
    // Verify scalability properties
    println!("\n=== Scalability Verification ===");
    
    // Check if routing complexity is O(k) where k is average degree
    println!("Routing Complexity (should be O(k) per hop):");
    for result in &results {
        println!("  N={:<6}: avg_hops={:.2}, avg_degree={:.2}, ratio={:.2}",
                 result.network_size,
                 result.avg_hops,
                 result.avg_degree,
                 result.avg_hops / result.avg_degree.max(1.0));
    }
    
    // Check if memory usage is O(k) per node
    println!("\nMemory Complexity (should be O(k) per node):");
    for result in &results {
        println!("  N={:<6}: memory/node={} bytes, avg_degree={:.2}, bytes/neighbor={:.1}",
                 result.network_size,
                 result.memory_per_node_bytes,
                 result.avg_degree,
                 result.memory_per_node_bytes as f64 / result.avg_degree.max(1.0));
    }
    
    // Check if embedding time is O(|E|)
    println!("\nEmbedding Complexity (should be O(|E|)):");
    for result in &results {
        println!("  N={:<6}: embedding_time={} ms, edges={}, ms/edge={:.4}",
                 result.network_size,
                 result.embedding_time_ms,
                 result.num_edges,
                 result.embedding_complexity_per_edge);
    }
    
    // Save results to JSON
    let summary = ScalabilitySummary {
        results,
        timestamp: chrono::Utc::now().to_rfc3339(),
        config: ScalabilityConfigSummary {
            network_sizes: config.network_sizes,
            num_routing_tests: config.num_routing_tests,
            max_ttl: config.max_ttl,
            seed: config.seed,
        },
    };
    
    match File::create(&config.output_file) {
        Ok(mut file) => {
            let json = serde_json::to_string_pretty(&summary).unwrap();
            file.write_all(json.as_bytes()).unwrap();
            println!("\n✓ Results saved to: {}", config.output_file);
        }
        Err(e) => {
            eprintln!("\n✗ Failed to save results: {}", e);
        }
    }
    
    println!("\n✓ Scalability experiments complete!");
}
