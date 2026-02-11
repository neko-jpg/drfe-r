//! Comprehensive Benchmark for Paper Data Collection
//!
//! Tests:
//! 1. Multiple network topologies (BA, ER, WS, Grid)
//! 2. Scalability (100 - 10000 nodes)
//! 3. Memory overhead
//! 4. Latency measurements
//! 5. Ablation study
//! 6. Stress tests

use drfe_r::coordinates::{NodeId, RoutingCoordinate};
use drfe_r::greedy_embedding::GreedyEmbedding;
use drfe_r::routing::{GPRouter, RoutingNode, RoutingMode, PacketHeader, DeliveryResult};
use drfe_r::tz_routing::{TZRoutingTable, TZConfig};
use drfe_r::PoincareDiskPoint;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::Write;
use std::time::Instant;

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TopologyResult {
    topology: String,
    nodes: usize,
    edges: usize,
    avg_degree: f64,
    diameter: u32,
    strategy: String,
    success_rate: f64,
    avg_hops: f64,
    stretch: f64,
    max_stretch: f64,
    gravity_pct: f64,
    tz_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScalabilityResult {
    nodes: usize,
    edges: usize,
    strategy: String,
    success_rate: f64,
    stretch: f64,
    max_stretch: f64,
    preprocessing_time_ms: u128,
    tz_build_time_ms: u128,
    routing_time_per_pair_us: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryResult {
    nodes: usize,
    tz_table_entries: usize,
    tz_bytes_estimated: usize,
    bytes_per_node: f64,
    landmark_count: usize,
    avg_bunch_size: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LatencyResult {
    nodes: usize,
    pie_embedding_ms: u128,
    tz_build_ms: u128,
    routing_decision_us: f64,
    path_computation_us: f64,
    total_preprocessing_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AblationResult {
    nodes: usize,
    configuration: String,
    success_rate: f64,
    stretch: f64,
    max_stretch: f64,
    gravity_pct: f64,
    pressure_pct: f64,
    tree_pct: f64,
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    println!("╔══════════════════════════════════════════════════════════════════════════╗");
    println!("║          COMPREHENSIVE BENCHMARK FOR PAPER DATA                          ║");
    println!("╚══════════════════════════════════════════════════════════════════════════╝\n");

    let seed = 42u64;
    let num_tests = 500;
    let seeds: Vec<u64> = vec![42, 43, 44, 45, 46]; // Multi-seed for statistical rigor

    // Create output directory
    std::fs::create_dir_all("paper_data/comprehensive").ok();

    // 1. Topology Tests
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("                    1. TOPOLOGY TESTS                          ");
    println!("═══════════════════════════════════════════════════════════════\n");
    let topology_results = run_topology_tests(1000, num_tests, seed);
    save_json(&topology_results, "paper_data/comprehensive/topology_results.json");

    // 2. Scalability Tests (multi-seed for statistical confidence)
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("                    2. SCALABILITY TESTS (multi-seed)           ");
    println!("═══════════════════════════════════════════════════════════════\n");
    let mut all_scalability_results = Vec::new();
    for &s in &seeds {
        println!("\n--- Seed {} ---", s);
        let results = run_scalability_tests(num_tests, s);
        all_scalability_results.push(results);
    }
    // Flatten for backward compatibility (use first seed for summary report)
    let scalability_results = all_scalability_results[0].clone();
    // Save all seeds data
    save_json(&all_scalability_results, "paper_data/comprehensive/scalability_multiseed.json");
    save_json(&scalability_results, "paper_data/comprehensive/scalability_results.json");
    // Generate confidence interval summary
    generate_scalability_ci_report(&all_scalability_results);

    // 3. Memory Overhead
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("                    3. MEMORY OVERHEAD                         ");
    println!("═══════════════════════════════════════════════════════════════\n");
    let memory_results = run_memory_tests(seed);
    save_json(&memory_results, "paper_data/comprehensive/memory_results.json");

    // 4. Latency Tests
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("                    4. LATENCY TESTS                           ");
    println!("═══════════════════════════════════════════════════════════════\n");
    let latency_results = run_latency_tests(seed);
    save_json(&latency_results, "paper_data/comprehensive/latency_results.json");

    // 5. Ablation Study
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("                    5. ABLATION STUDY                          ");
    println!("═══════════════════════════════════════════════════════════════\n");
    let ablation_results = run_ablation_study(num_tests, seed);
    save_json(&ablation_results, "paper_data/comprehensive/ablation_results.json");

    // Generate Summary Report
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("                    GENERATING SUMMARY                         ");
    println!("═══════════════════════════════════════════════════════════════\n");
    generate_summary_report(
        &topology_results,
        &scalability_results,
        &memory_results,
        &latency_results,
        &ablation_results,
    );

    println!("\n✓ All results saved to paper_data/comprehensive/");
}

// ============================================================================
// 1. Topology Tests
// ============================================================================

fn run_topology_tests(n: usize, num_tests: usize, seed: u64) -> Vec<TopologyResult> {
    let mut results = Vec::new();

    let topologies = vec![
        ("Barabasi-Albert", generate_ba_network(n, 3, seed)),
        ("Erdos-Renyi", generate_er_network(n, 0.006, seed)),
        ("Watts-Strogatz", generate_ws_network(n, 6, 0.3, seed)),
        ("Grid", generate_grid_network((n as f64).sqrt() as usize)),
        ("Tree", generate_tree_network(n, 3, seed)),
    ];

    println!("{:<20} {:<10} {:<10} {:<10} {:<10} {:<10}",
             "Topology", "Nodes", "Edges", "PIE-Str", "TZ-Str", "Improve");
    println!("{}", "-".repeat(70));

    for (name, (nodes, adj_idx, adjacency)) in topologies {
        let n_actual = nodes.len();
        let edges: usize = adj_idx.iter().map(|v| v.len()).sum::<usize>() / 2;
        let avg_degree = if n_actual > 0 { edges as f64 * 2.0 / n_actual as f64 } else { 0.0 };
        let diameter = compute_diameter(&adjacency, 50);

        // PIE-DFS
        let (router_pie, _) = build_router_pie(&nodes, &adj_idx, &adjacency);
        let pie_result = run_routing_tests(&router_pie, None, &nodes, num_tests, seed);

        results.push(TopologyResult {
            topology: name.to_string(),
            nodes: n_actual,
            edges,
            avg_degree,
            diameter,
            strategy: "PIE-DFS".to_string(),
            success_rate: pie_result.success_rate,
            avg_hops: pie_result.avg_hops,
            stretch: pie_result.stretch,
            max_stretch: pie_result.max_stretch,
            gravity_pct: pie_result.gravity_pct,
            tz_pct: 0.0,
        });

        // PIE+TZ
        let tz_table = TZRoutingTable::build(&adjacency, TZConfig::default()).unwrap();
        let tz_result = run_routing_tests_with_tz(&router_pie, &tz_table, &nodes, num_tests, seed);

        results.push(TopologyResult {
            topology: name.to_string(),
            nodes: n_actual,
            edges,
            avg_degree,
            diameter,
            strategy: "PIE+TZ".to_string(),
            success_rate: tz_result.success_rate,
            avg_hops: tz_result.avg_hops,
            stretch: tz_result.stretch,
            max_stretch: tz_result.max_stretch,
            gravity_pct: tz_result.gravity_pct,
            tz_pct: tz_result.tz_pct,
        });

        let improvement = if tz_result.stretch > 0.0 { pie_result.stretch / tz_result.stretch } else { 0.0 };
        println!("{:<20} {:<10} {:<10} {:<10.2} {:<10.2} {:<10.1}x",
                 name, n_actual, edges, pie_result.stretch, tz_result.stretch, improvement);
    }

    results
}

// ============================================================================
// 2. Scalability Tests
// ============================================================================

fn run_scalability_tests(num_tests: usize, seed: u64) -> Vec<ScalabilityResult> {
    let mut results = Vec::new();
    let sizes = vec![100, 500, 1000, 2000, 3000, 5000, 7000, 10000];

    println!("{:<8} {:<10} {:<10} {:<10} {:<12} {:<12}",
             "Nodes", "PIE-Str", "TZ-Str", "MaxStr", "Preproc(ms)", "Route(μs)");
    println!("{}", "-".repeat(75));

    for &n in &sizes {
        let start_total = Instant::now();
        let (nodes, adj_idx, adjacency) = generate_ba_network(n, 3, seed);
        let edges = adj_idx.iter().map(|v| v.len()).sum::<usize>() / 2;

        // Build PIE
        let pie_start = Instant::now();
        let (router_pie, _) = build_router_pie(&nodes, &adj_idx, &adjacency);
        let pie_time = pie_start.elapsed().as_millis();

        // Build TZ
        let tz_start = Instant::now();
        let tz_table = TZRoutingTable::build(&adjacency, TZConfig::default()).unwrap();
        let tz_build_time = tz_start.elapsed().as_millis();

        let preprocess_total = start_total.elapsed().as_millis();

        // PIE-DFS test
        let pie_result = run_routing_tests(&router_pie, None, &nodes, num_tests.min(n*n/4), seed);
        results.push(ScalabilityResult {
            nodes: n,
            edges,
            strategy: "PIE-DFS".to_string(),
            success_rate: pie_result.success_rate,
            stretch: pie_result.stretch,
            max_stretch: pie_result.max_stretch,
            preprocessing_time_ms: pie_time,
            tz_build_time_ms: 0,
            routing_time_per_pair_us: pie_result.routing_time_us,
        });

        // PIE+TZ test
        let tz_result = run_routing_tests_with_tz(&router_pie, &tz_table, &nodes, num_tests.min(n*n/4), seed);
        results.push(ScalabilityResult {
            nodes: n,
            edges,
            strategy: "PIE+TZ".to_string(),
            success_rate: tz_result.success_rate,
            stretch: tz_result.stretch,
            max_stretch: tz_result.max_stretch,
            preprocessing_time_ms: pie_time,
            tz_build_time_ms: tz_build_time,
            routing_time_per_pair_us: tz_result.routing_time_us,
        });

        println!("{:<8} {:<10.2} {:<10.2} {:<10.2} {:<12} {:<12.1}",
                 n, pie_result.stretch, tz_result.stretch, tz_result.max_stretch, 
                 preprocess_total, tz_result.routing_time_us);
    }

    results
}

// ============================================================================
// 3. Memory Tests
// ============================================================================

fn run_memory_tests(seed: u64) -> Vec<MemoryResult> {
    let mut results = Vec::new();
    let sizes = vec![100, 500, 1000, 2000, 3000, 5000, 10000];

    println!("{:<8} {:<12} {:<15} {:<12} {:<10} {:<12}",
             "Nodes", "TZ Entries", "Est. Bytes", "Bytes/Node", "Landmarks", "Avg Bunch");
    println!("{}", "-".repeat(75));

    for &n in &sizes {
        let (_, _, adjacency) = generate_ba_network(n, 3, seed);
        let tz_table = TZRoutingTable::build(&adjacency, TZConfig::default()).unwrap();

        let entries = tz_table.memory_usage();
        let landmarks = tz_table.landmarks.len();
        
        // Estimate bytes: each entry ≈ 48 bytes (NodeId + u32 + NodeId for bunch)
        let bytes_estimated = entries * 48 + landmarks * 32;
        let bytes_per_node = bytes_estimated as f64 / n as f64;

        let avg_bunch: f64 = tz_table.node_info.values()
            .map(|info| info.bunch.len())
            .sum::<usize>() as f64 / n as f64;

        results.push(MemoryResult {
            nodes: n,
            tz_table_entries: entries,
            tz_bytes_estimated: bytes_estimated,
            bytes_per_node,
            landmark_count: landmarks,
            avg_bunch_size: avg_bunch,
        });

        println!("{:<8} {:<12} {:<15} {:<12.1} {:<10} {:<12.1}",
                 n, entries, format_bytes(bytes_estimated), bytes_per_node, landmarks, avg_bunch);
    }

    results
}

// ============================================================================
// 4. Latency Tests
// ============================================================================

fn run_latency_tests(seed: u64) -> Vec<LatencyResult> {
    let mut results = Vec::new();
    let sizes = vec![100, 500, 1000, 2000, 5000];
    let warmup = 100;
    let iterations = 1000;

    println!("{:<8} {:<12} {:<12} {:<15} {:<15}",
             "Nodes", "PIE(ms)", "TZ(ms)", "Route(μs)", "Path(μs)");
    println!("{}", "-".repeat(65));

    for &n in &sizes {
        let (nodes, adj_idx, adjacency) = generate_ba_network(n, 3, seed);

        // PIE embedding time
        let pie_start = Instant::now();
        let (router, _) = build_router_pie(&nodes, &adj_idx, &adjacency);
        let pie_ms = pie_start.elapsed().as_millis();

        // TZ build time
        let tz_start = Instant::now();
        let tz_table = TZRoutingTable::build(&adjacency, TZConfig::default()).unwrap();
        let tz_ms = tz_start.elapsed().as_millis();

        // Routing decision latency
        let mut rng = StdRng::seed_from_u64(seed);
        let mut total_route_ns = 0u128;
        let mut total_path_ns = 0u128;

        // Warmup
        for _ in 0..warmup {
            let src = rng.gen_range(0..n);
            let dst = rng.gen_range(0..n);
            if src != dst {
                let _ = tz_table.compute_path(&nodes[src], &nodes[dst]);
            }
        }

        // Measure
        for _ in 0..iterations {
            let src = rng.gen_range(0..n);
            let mut dst = rng.gen_range(0..n);
            while dst == src { dst = rng.gen_range(0..n); }

            // Routing decision (single hop)
            if let Some(dest_node) = router.get_node(&nodes[dst]) {
                let mut packet = PacketHeader::new(
                    nodes[src].clone(),
                    nodes[dst].clone(),
                    dest_node.coord.point,
                    100,
                );
                let route_start = Instant::now();
                let _ = router.route(&nodes[src], &mut packet);
                total_route_ns += route_start.elapsed().as_nanos();
            }

            // Path computation
            let path_start = Instant::now();
            let _ = tz_table.compute_path(&nodes[src], &nodes[dst]);
            total_path_ns += path_start.elapsed().as_nanos();
        }

        let route_us = total_route_ns as f64 / iterations as f64 / 1000.0;
        let path_us = total_path_ns as f64 / iterations as f64 / 1000.0;

        results.push(LatencyResult {
            nodes: n,
            pie_embedding_ms: pie_ms,
            tz_build_ms: tz_ms,
            routing_decision_us: route_us,
            path_computation_us: path_us,
            total_preprocessing_ms: pie_ms + tz_ms,
        });

        println!("{:<8} {:<12} {:<12} {:<15.2} {:<15.2}",
                 n, pie_ms, tz_ms, route_us, path_us);
    }

    results
}

// ============================================================================
// 5. Ablation Study
// ============================================================================

fn run_ablation_study(num_tests: usize, seed: u64) -> Vec<AblationResult> {
    let mut results = Vec::new();
    let sizes = vec![500, 1000, 2000];

    println!("{:<8} {:<20} {:<10} {:<10} {:<10} {:<10}",
             "Nodes", "Configuration", "Success%", "Stretch", "MaxStr", "Gravity%");
    println!("{}", "-".repeat(70));

    for &n in &sizes {
        let (nodes, adj_idx, adjacency) = generate_ba_network(n, 3, seed);
        let (router, _) = build_router_pie(&nodes, &adj_idx, &adjacency);
        let tz_table = TZRoutingTable::build(&adjacency, TZConfig::default()).unwrap();

        // PIE only (Gravity only, no fallback)
        let gravity_only = run_gravity_only_tests(&router, &nodes, num_tests, seed);
        results.push(AblationResult {
            nodes: n,
            configuration: "Gravity-only".to_string(),
            success_rate: gravity_only.success_rate,
            stretch: gravity_only.stretch,
            max_stretch: gravity_only.max_stretch,
            gravity_pct: 100.0,
            pressure_pct: 0.0,
            tree_pct: 0.0,
        });

        // PIE + DFS (current baseline)
        let pie_dfs = run_routing_tests(&router, None, &nodes, num_tests, seed);
        results.push(AblationResult {
            nodes: n,
            configuration: "PIE+DFS".to_string(),
            success_rate: pie_dfs.success_rate,
            stretch: pie_dfs.stretch,
            max_stretch: pie_dfs.max_stretch,
            gravity_pct: pie_dfs.gravity_pct,
            pressure_pct: pie_dfs.pressure_pct,
            tree_pct: pie_dfs.tree_pct,
        });

        // PIE + TZ (proposed)
        let pie_tz = run_routing_tests_with_tz(&router, &tz_table, &nodes, num_tests, seed);
        results.push(AblationResult {
            nodes: n,
            configuration: "PIE+TZ".to_string(),
            success_rate: pie_tz.success_rate,
            stretch: pie_tz.stretch,
            max_stretch: pie_tz.max_stretch,
            gravity_pct: pie_tz.gravity_pct,
            pressure_pct: 0.0,
            tree_pct: pie_tz.tz_pct,
        });

        // TZ only
        let (tz_avg, tz_max, _) = tz_table.verify_stretch(&adjacency, num_tests);
        results.push(AblationResult {
            nodes: n,
            configuration: "TZ-only".to_string(),
            success_rate: 1.0,
            stretch: tz_avg,
            max_stretch: tz_max,
            gravity_pct: 0.0,
            pressure_pct: 0.0,
            tree_pct: 100.0,
        });

        // Print for this size
        for r in results.iter().rev().take(4).collect::<Vec<_>>().into_iter().rev() {
            if r.nodes == n {
                println!("{:<8} {:<20} {:<10.2} {:<10.2} {:<10.2} {:<10.2}",
                         r.nodes, r.configuration, r.success_rate * 100.0,
                         r.stretch, r.max_stretch, r.gravity_pct);
            }
        }
        println!();
    }

    results
}

// ============================================================================
// Network Generators
// ============================================================================

fn generate_ba_network(n: usize, m: usize, seed: u64) 
    -> (Vec<NodeId>, Vec<Vec<usize>>, HashMap<NodeId, Vec<NodeId>>) 
{
    let mut rng = StdRng::seed_from_u64(seed);
    let mut degrees: Vec<usize> = vec![0; n];
    let mut adjacency_idx: Vec<Vec<usize>> = vec![Vec::new(); n];
    let nodes: Vec<NodeId> = (0..n).map(|i| NodeId::new(format!("node_{}", i))).collect();

    for i in 0..m.min(n) {
        for j in (i+1)..m.min(n) {
            adjacency_idx[i].push(j);
            adjacency_idx[j].push(i);
            degrees[i] += 1;
            degrees[j] += 1;
        }
    }

    for i in m..n {
        let total: usize = degrees.iter().take(i).sum();
        if total == 0 { continue; }

        let mut connected = HashSet::new();
        while connected.len() < m.min(i) {
            let r = rng.gen::<f64>() * total as f64;
            let mut cumsum = 0.0;
            for j in 0..i {
                cumsum += degrees[j] as f64;
                if cumsum >= r && !connected.contains(&j) {
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

    let mut adjacency: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for i in 0..n {
        adjacency.insert(nodes[i].clone(), 
            adjacency_idx[i].iter().map(|&j| nodes[j].clone()).collect());
    }

    (nodes, adjacency_idx, adjacency)
}

fn generate_er_network(n: usize, p: f64, seed: u64) 
    -> (Vec<NodeId>, Vec<Vec<usize>>, HashMap<NodeId, Vec<NodeId>>) 
{
    let mut rng = StdRng::seed_from_u64(seed);
    let mut adjacency_idx: Vec<Vec<usize>> = vec![Vec::new(); n];
    let nodes: Vec<NodeId> = (0..n).map(|i| NodeId::new(format!("er_{}", i))).collect();

    for i in 0..n {
        for j in (i+1)..n {
            if rng.gen::<f64>() < p {
                adjacency_idx[i].push(j);
                adjacency_idx[j].push(i);
            }
        }
    }

    // Ensure connected
    ensure_connected(&mut adjacency_idx, n, &mut rng);

    let mut adjacency: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for i in 0..n {
        adjacency.insert(nodes[i].clone(),
            adjacency_idx[i].iter().map(|&j| nodes[j].clone()).collect());
    }

    (nodes, adjacency_idx, adjacency)
}

fn generate_ws_network(n: usize, k: usize, beta: f64, seed: u64) 
    -> (Vec<NodeId>, Vec<Vec<usize>>, HashMap<NodeId, Vec<NodeId>>) 
{
    let mut rng = StdRng::seed_from_u64(seed);
    let mut adjacency_idx: Vec<Vec<usize>> = vec![Vec::new(); n];
    let nodes: Vec<NodeId> = (0..n).map(|i| NodeId::new(format!("ws_{}", i))).collect();

    // Ring lattice
    for i in 0..n {
        for j in 1..=k/2 {
            let neighbor = (i + j) % n;
            if !adjacency_idx[i].contains(&neighbor) {
                adjacency_idx[i].push(neighbor);
                adjacency_idx[neighbor].push(i);
            }
        }
    }

    // Rewire with probability beta
    for i in 0..n {
        let neighbors: Vec<usize> = adjacency_idx[i].clone();
        for &j in &neighbors {
            if j > i && rng.gen::<f64>() < beta {
                // Remove edge (i,j)
                adjacency_idx[i].retain(|&x| x != j);
                adjacency_idx[j].retain(|&x| x != i);
                
                // Add random edge
                let mut new_j = rng.gen_range(0..n);
                while new_j == i || adjacency_idx[i].contains(&new_j) {
                    new_j = rng.gen_range(0..n);
                }
                adjacency_idx[i].push(new_j);
                adjacency_idx[new_j].push(i);
            }
        }
    }

    let mut adjacency: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for i in 0..n {
        adjacency.insert(nodes[i].clone(),
            adjacency_idx[i].iter().map(|&j| nodes[j].clone()).collect());
    }

    (nodes, adjacency_idx, adjacency)
}

fn generate_grid_network(side: usize) 
    -> (Vec<NodeId>, Vec<Vec<usize>>, HashMap<NodeId, Vec<NodeId>>) 
{
    let n = side * side;
    let mut adjacency_idx: Vec<Vec<usize>> = vec![Vec::new(); n];
    let nodes: Vec<NodeId> = (0..n).map(|i| NodeId::new(format!("grid_{}", i))).collect();

    for i in 0..side {
        for j in 0..side {
            let idx = i * side + j;
            if j + 1 < side {
                adjacency_idx[idx].push(idx + 1);
                adjacency_idx[idx + 1].push(idx);
            }
            if i + 1 < side {
                adjacency_idx[idx].push(idx + side);
                adjacency_idx[idx + side].push(idx);
            }
        }
    }

    let mut adjacency: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for i in 0..n {
        adjacency.insert(nodes[i].clone(),
            adjacency_idx[i].iter().map(|&j| nodes[j].clone()).collect());
    }

    (nodes, adjacency_idx, adjacency)
}

fn generate_tree_network(n: usize, branching: usize, seed: u64) 
    -> (Vec<NodeId>, Vec<Vec<usize>>, HashMap<NodeId, Vec<NodeId>>) 
{
    let mut rng = StdRng::seed_from_u64(seed);
    let mut adjacency_idx: Vec<Vec<usize>> = vec![Vec::new(); n];
    let nodes: Vec<NodeId> = (0..n).map(|i| NodeId::new(format!("tree_{}", i))).collect();

    for i in 1..n {
        let parent = if i <= branching { 0 } else { rng.gen_range(0..i) };
        adjacency_idx[i].push(parent);
        adjacency_idx[parent].push(i);
    }

    let mut adjacency: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for i in 0..n {
        adjacency.insert(nodes[i].clone(),
            adjacency_idx[i].iter().map(|&j| nodes[j].clone()).collect());
    }

    (nodes, adjacency_idx, adjacency)
}

fn ensure_connected(adj: &mut Vec<Vec<usize>>, n: usize, rng: &mut StdRng) {
    let mut visited = vec![false; n];
    let mut queue = VecDeque::new();
    queue.push_back(0);
    visited[0] = true;
    let mut count = 1;

    while let Some(u) = queue.pop_front() {
        for &v in &adj[u] {
            if !visited[v] {
                visited[v] = true;
                count += 1;
                queue.push_back(v);
            }
        }
    }

    // Connect isolated components
    for i in 0..n {
        if !visited[i] {
            let connected = rng.gen_range(0..i);
            adj[i].push(connected);
            adj[connected].push(i);
        }
    }
}

// ============================================================================
// Router Building
// ============================================================================

fn build_router_pie(
    nodes: &[NodeId], 
    adjacency_idx: &[Vec<usize>], 
    adjacency: &HashMap<NodeId, Vec<NodeId>>
) -> (GPRouter, u128) {
    let start = Instant::now();
    let embedder = GreedyEmbedding::new();
    let result = embedder.embed(adjacency).expect("PIE embedding failed");

    let mut tree_parent: HashMap<NodeId, Option<NodeId>> = HashMap::new();
    tree_parent.insert(result.root.clone(), None);
    for (parent, children) in &result.tree_children {
        for child in children {
            tree_parent.insert(child.clone(), Some(parent.clone()));
        }
    }

    let mut router = GPRouter::new();
    for node_id in nodes.iter() {
        let point = result.coordinates.get(node_id).copied().unwrap_or_else(PoincareDiskPoint::origin);
        let coord = RoutingCoordinate::new(point, 0);
        let mut rn = RoutingNode::new(node_id.clone(), coord);
        rn.set_tree_info(
            tree_parent.get(node_id).cloned().flatten(),
            result.tree_children.get(node_id).cloned().unwrap_or_default()
        );
        router.add_node(rn);
    }

    for i in 0..nodes.len() {
        for &j in &adjacency_idx[i] {
            if i < j {
                router.add_edge(&nodes[i], &nodes[j]);
            }
        }
    }

    (router, start.elapsed().as_millis())
}

// ============================================================================
// Test Runners
// ============================================================================

struct TestResult {
    success_rate: f64,
    avg_hops: f64,
    stretch: f64,
    max_stretch: f64,
    gravity_pct: f64,
    pressure_pct: f64,
    tree_pct: f64,
    tz_pct: f64,
    routing_time_us: f64,
}

fn run_routing_tests(
    router: &GPRouter, 
    _tz: Option<&TZRoutingTable>,
    nodes: &[NodeId], 
    num_tests: usize, 
    seed: u64
) -> TestResult {
    let mut rng = StdRng::seed_from_u64(seed + 1000);
    let n = nodes.len();
    let max_ttl = (n * 20) as u32;

    let mut successes = 0u32;
    let mut total_hops = 0u32;
    let mut total_optimal = 0u32;
    let mut gravity = 0u32;
    let mut pressure = 0u32;
    let mut tree = 0u32;
    let mut all_hops = 0u32;
    let mut max_stretch = 0.0f64;

    let start = Instant::now();

    for _ in 0..num_tests {
        let src = rng.gen_range(0..n);
        let mut dst = rng.gen_range(0..n);
        while dst == src { dst = rng.gen_range(0..n); }

        if let Some(dest_node) = router.get_node(&nodes[dst]) {
            let result = router.simulate_delivery(&nodes[src], &nodes[dst], dest_node.coord.point, max_ttl);
            if result.success {
                successes += 1;
                total_hops += result.hops;
                gravity += result.gravity_hops;
                pressure += result.pressure_hops;
                tree += result.tree_hops;
                all_hops += result.hops;

                if let Some(opt) = bfs_distance(router, &nodes[src], &nodes[dst]) {
                    total_optimal += opt;
                    if opt > 0 {
                        let s = result.hops as f64 / opt as f64;
                        max_stretch = f64::max(max_stretch, s);
                    }
                }
            }
        }
    }

    let elapsed_us = start.elapsed().as_micros() as f64;

    TestResult {
        success_rate: successes as f64 / num_tests as f64,
        avg_hops: if successes > 0 { total_hops as f64 / successes as f64 } else { 0.0 },
        stretch: if total_optimal > 0 { total_hops as f64 / total_optimal as f64 } else { 0.0 },
        max_stretch,
        gravity_pct: if all_hops > 0 { gravity as f64 / all_hops as f64 * 100.0 } else { 0.0 },
        pressure_pct: if all_hops > 0 { pressure as f64 / all_hops as f64 * 100.0 } else { 0.0 },
        tree_pct: if all_hops > 0 { tree as f64 / all_hops as f64 * 100.0 } else { 0.0 },
        tz_pct: 0.0,
        routing_time_us: elapsed_us / num_tests as f64,
    }
}

fn run_routing_tests_with_tz(
    router: &GPRouter, 
    tz_table: &TZRoutingTable,
    nodes: &[NodeId], 
    num_tests: usize, 
    seed: u64
) -> TestResult {
    let mut rng = StdRng::seed_from_u64(seed + 1000);
    let n = nodes.len();
    let max_gravity = n as u32;

    let mut successes = 0u32;
    let mut total_hops = 0u32;
    let mut total_optimal = 0u32;
    let mut gravity = 0u32;
    let mut tz = 0u32;
    let mut all_hops = 0u32;
    let mut max_stretch = 0.0f64;

    let start = Instant::now();

    for _ in 0..num_tests {
        let src = rng.gen_range(0..n);
        let mut dst = rng.gen_range(0..n);
        while dst == src { dst = rng.gen_range(0..n); }

        let (success, g_hops, final_node) = try_gravity_only_limited(router, &nodes[src], &nodes[dst], max_gravity);
        
        let (ok, hops, g, t) = if success {
            (true, g_hops, g_hops, 0)
        } else {
            if let Some(path) = tz_table.compute_path(&final_node, &nodes[dst]) {
                let tz_hops = (path.len() - 1) as u32;
                (true, g_hops + tz_hops, g_hops, tz_hops)
            } else {
                (false, 0, 0, 0)
            }
        };

        if ok {
            successes += 1;
            total_hops += hops;
            gravity += g;
            tz += t;
            all_hops += hops;

            if let Some(opt) = bfs_distance(router, &nodes[src], &nodes[dst]) {
                total_optimal += opt;
                if opt > 0 {
                    let s = hops as f64 / opt as f64;
                    max_stretch = f64::max(max_stretch, s);
                }
            }
        }
    }

    let elapsed_us = start.elapsed().as_micros() as f64;

    TestResult {
        success_rate: successes as f64 / num_tests as f64,
        avg_hops: if successes > 0 { total_hops as f64 / successes as f64 } else { 0.0 },
        stretch: if total_optimal > 0 { total_hops as f64 / total_optimal as f64 } else { 0.0 },
        max_stretch,
        gravity_pct: if all_hops > 0 { gravity as f64 / all_hops as f64 * 100.0 } else { 0.0 },
        pressure_pct: 0.0,
        tree_pct: 0.0,
        tz_pct: if all_hops > 0 { tz as f64 / all_hops as f64 * 100.0 } else { 0.0 },
        routing_time_us: elapsed_us / num_tests as f64,
    }
}

fn run_gravity_only_tests(
    router: &GPRouter, 
    nodes: &[NodeId], 
    num_tests: usize, 
    seed: u64
) -> TestResult {
    let mut rng = StdRng::seed_from_u64(seed + 2000);
    let n = nodes.len();
    let max_hops = n as u32;

    let mut successes = 0u32;
    let mut total_hops = 0u32;
    let mut total_optimal = 0u32;
    let mut max_stretch = 0.0f64;

    for _ in 0..num_tests {
        let src = rng.gen_range(0..n);
        let mut dst = rng.gen_range(0..n);
        while dst == src { dst = rng.gen_range(0..n); }

        let (success, hops, _) = try_gravity_only_limited(router, &nodes[src], &nodes[dst], max_hops);
        
        if success {
            successes += 1;
            total_hops += hops;

            if let Some(opt) = bfs_distance(router, &nodes[src], &nodes[dst]) {
                total_optimal += opt;
                if opt > 0 {
                    let s = hops as f64 / opt as f64;
                    max_stretch = f64::max(max_stretch, s);
                }
            }
        }
    }

    TestResult {
        success_rate: successes as f64 / num_tests as f64,
        avg_hops: if successes > 0 { total_hops as f64 / successes as f64 } else { 0.0 },
        stretch: if total_optimal > 0 { total_hops as f64 / total_optimal as f64 } else { 0.0 },
        max_stretch,
        gravity_pct: 100.0,
        pressure_pct: 0.0,
        tree_pct: 0.0,
        tz_pct: 0.0,
        routing_time_us: 0.0,
    }
}

fn try_gravity_only_limited(router: &GPRouter, src: &NodeId, dst: &NodeId, max: u32) -> (bool, u32, NodeId) {
    if src == dst { return (true, 0, src.clone()); }

    let dest_coord = match router.get_node(dst) {
        Some(n) => n.coord.point,
        None => return (false, 0, src.clone()),
    };

    let mut current = src.clone();
    let mut hops = 0;
    let mut visited = HashSet::new();
    visited.insert(src.clone());

    for _ in 0..max {
        if &current == dst { return (true, hops, current); }

        let node = match router.get_node(&current) {
            Some(n) => n,
            None => break,
        };

        let cur_dist = node.coord.point.hyperbolic_distance(&dest_coord);
        let mut best: Option<NodeId> = None;
        let mut best_dist = cur_dist;

        for neighbor in &node.neighbors {
            if visited.contains(neighbor) { continue; }
            if let Some(n) = router.get_node(neighbor) {
                let d = n.coord.point.hyperbolic_distance(&dest_coord);
                if d < best_dist {
                    best_dist = d;
                    best = Some(neighbor.clone());
                }
            }
        }

        match best {
            Some(next) => {
                visited.insert(next.clone());
                current = next;
                hops += 1;
            }
            None => break,
        }
    }

    (&current == dst, hops, current)
}

// ============================================================================
// Utilities
// ============================================================================

fn bfs_distance(router: &GPRouter, start: &NodeId, end: &NodeId) -> Option<u32> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back((start.clone(), 0u32));
    visited.insert(start.clone());

    while let Some((cur, dist)) = queue.pop_front() {
        if &cur == end { return Some(dist); }
        if let Some(node) = router.get_node(&cur) {
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

fn compute_diameter(adj: &HashMap<NodeId, Vec<NodeId>>, samples: usize) -> u32 {
    let nodes: Vec<&NodeId> = adj.keys().collect();
    if nodes.is_empty() { return 0; }

    let mut max_dist = 0u32;
    let mut rng = StdRng::seed_from_u64(12345);

    for _ in 0..samples.min(nodes.len()) {
        let start = nodes[rng.gen_range(0..nodes.len())];
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back((start.clone(), 0u32));
        visited.insert(start.clone());

        while let Some((cur, dist)) = queue.pop_front() {
            max_dist = max_dist.max(dist);
            if let Some(neighbors) = adj.get(&cur) {
                for n in neighbors {
                    if !visited.contains(n) {
                        visited.insert(n.clone());
                        queue.push_back((n.clone(), dist + 1));
                    }
                }
            }
        }
    }

    max_dist
}

fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    }
}

/// Generate confidence interval report for multi-seed scalability results
fn generate_scalability_ci_report(all_results: &[Vec<ScalabilityResult>]) {
    let path = "paper_data/comprehensive/scalability_ci_report.md";
    let mut f = match File::create(path) {
        Ok(f) => f,
        Err(_) => return,
    };

    writeln!(f, "# Scalability Results with 95% Confidence Intervals\n").ok();
    writeln!(f, "Seeds: {} runs\n", all_results.len()).ok();
    writeln!(f, "| Nodes | PIE+TZ Stretch (mean±CI) | Max Stretch (mean±CI) | Preprocess ms (mean±CI) |").ok();
    writeln!(f, "|-------|--------------------------|----------------------|------------------------|").ok();

    // Collect sizes from the first seed's results (PIE+TZ entries)
    let sizes: Vec<usize> = all_results[0].iter()
        .filter(|r| r.strategy == "PIE+TZ")
        .map(|r| r.nodes)
        .collect();

    for &n in &sizes {
        let mut stretches = Vec::new();
        let mut max_stretches = Vec::new();
        let mut preproc_times = Vec::new();

        for seed_results in all_results {
            for r in seed_results {
                if r.nodes == n && r.strategy == "PIE+TZ" {
                    stretches.push(r.stretch);
                    max_stretches.push(r.max_stretch);
                    preproc_times.push((r.preprocessing_time_ms + r.tz_build_time_ms) as f64);
                }
            }
        }

        let (s_mean, s_ci) = mean_ci(&stretches);
        let (m_mean, m_ci) = mean_ci(&max_stretches);
        let (p_mean, p_ci) = mean_ci(&preproc_times);

        writeln!(f, "| {} | {:.3}x ± {:.3} | {:.2}x ± {:.2} | {:.0} ± {:.0} |",
                 n, s_mean, s_ci, m_mean, m_ci, p_mean, p_ci).ok();
    }

    writeln!(f, "\n*95% CI computed using t-distribution with {} seeds*", all_results.len()).ok();
    println!("  ✓ CI report saved to {}", path);
}

/// Compute mean and 95% confidence interval
fn mean_ci(values: &[f64]) -> (f64, f64) {
    let n = values.len() as f64;
    if n < 2.0 {
        return (values.first().copied().unwrap_or(0.0), 0.0);
    }
    let mean = values.iter().sum::<f64>() / n;
    let variance = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let stderr = (variance / n).sqrt();
    // t-value for 95% CI (approximate for small n)
    let t = match values.len() {
        2 => 12.706,
        3 => 4.303,
        4 => 3.182,
        5 => 2.776,
        6 => 2.571,
        7 => 2.447,
        8 => 2.365,
        9 => 2.306,
        10 => 2.262,
        _ => 1.96,
    };
    (mean, t * stderr)
}

fn save_json<T: Serialize>(data: &T, path: &str) {
    if let Ok(mut file) = File::create(path) {
        let json = serde_json::to_string_pretty(data).unwrap();
        file.write_all(json.as_bytes()).ok();
        println!("  ✓ Saved {}", path);
    }
}

fn generate_summary_report(
    topology: &[TopologyResult],
    scalability: &[ScalabilityResult],
    memory: &[MemoryResult],
    latency: &[LatencyResult],
    ablation: &[AblationResult],
) {
    let path = "paper_data/comprehensive/SUMMARY_REPORT.md";
    let mut f = File::create(path).unwrap();

    writeln!(f, "# Comprehensive Benchmark Results\n").ok();
    writeln!(f, "Generated: {}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S")).ok();

    // Topology Summary
    writeln!(f, "## 1. Topology Comparison\n").ok();
    writeln!(f, "| Topology | Nodes | PIE Stretch | TZ Stretch | Improvement |").ok();
    writeln!(f, "|----------|-------|-------------|------------|-------------|").ok();
    
    let mut i = 0;
    while i + 1 < topology.len() {
        let pie = &topology[i];
        let tz = &topology[i + 1];
        let imp = if tz.stretch > 0.0 { pie.stretch / tz.stretch } else { 0.0 };
        writeln!(f, "| {} | {} | {:.2}x | {:.2}x | {:.1}x |",
                 pie.topology, pie.nodes, pie.stretch, tz.stretch, imp).ok();
        i += 2;
    }

    // Scalability Summary
    writeln!(f, "\n## 2. Scalability\n").ok();
    writeln!(f, "| Nodes | PIE Stretch | PIE+TZ Stretch | Max Stretch | Preprocess (ms) |").ok();
    writeln!(f, "|-------|-------------|----------------|-------------|-----------------|").ok();
    
    let mut i = 0;
    while i + 1 < scalability.len() {
        let pie = &scalability[i];
        let tz = &scalability[i + 1];
        writeln!(f, "| {} | {:.2}x | {:.2}x | {:.2}x | {} |",
                 pie.nodes, pie.stretch, tz.stretch, tz.max_stretch,
                 pie.preprocessing_time_ms + tz.tz_build_time_ms).ok();
        i += 2;
    }

    // Memory Summary
    writeln!(f, "\n## 3. Memory Overhead\n").ok();
    writeln!(f, "| Nodes | TZ Entries | Memory | Bytes/Node | Landmarks |").ok();
    writeln!(f, "|-------|------------|--------|------------|-----------|").ok();
    for m in memory {
        writeln!(f, "| {} | {} | {} | {:.1} | {} |",
                 m.nodes, m.tz_table_entries, format_bytes(m.tz_bytes_estimated),
                 m.bytes_per_node, m.landmark_count).ok();
    }

    // Latency Summary
    writeln!(f, "\n## 4. Latency\n").ok();
    writeln!(f, "| Nodes | PIE Build | TZ Build | Route Decision | Path Compute |").ok();
    writeln!(f, "|-------|-----------|----------|----------------|--------------|").ok();
    for l in latency {
        writeln!(f, "| {} | {}ms | {}ms | {:.2}μs | {:.2}μs |",
                 l.nodes, l.pie_embedding_ms, l.tz_build_ms,
                 l.routing_decision_us, l.path_computation_us).ok();
    }

    // Ablation Summary
    writeln!(f, "\n## 5. Ablation Study\n").ok();
    writeln!(f, "| Nodes | Configuration | Success | Stretch | Max Stretch |").ok();
    writeln!(f, "|-------|---------------|---------|---------|-------------|").ok();
    for a in ablation {
        writeln!(f, "| {} | {} | {:.1}% | {:.2}x | {:.2}x |",
                 a.nodes, a.configuration, a.success_rate * 100.0,
                 a.stretch, a.max_stretch).ok();
    }

    writeln!(f, "\n---\n*All data saved in JSON format for further analysis.*").ok();
    println!("  ✓ Summary report saved to {}", path);
}
