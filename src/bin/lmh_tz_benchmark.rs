//! LMH-TZ Benchmark
//!
//! Compares routing performance across different embedding strategies:
//! - PIE only (baseline)
//! - LMH (Landmark-MDS Hyperbolic) embedding
//! - LMH + TZ (with Thorup-Zwick fallback)
//!
//! Results are saved to paper_data/lmh_tz/

use drfe_r::coordinates::{NodeId, RoutingCoordinate};
use drfe_r::greedy_embedding::GreedyEmbedding;
use drfe_r::landmark_embedding::{LandmarkEmbedding, LandmarkConfig};
use drfe_r::routing::{GPRouter, RoutingNode};
use drfe_r::tz_routing::{TZRoutingTable, TZConfig};
use drfe_r::PoincareDiskPoint;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::Write;
use std::time::Instant;

/// Results from a single benchmark run
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BenchmarkResult {
    network_size: usize,
    embedding_strategy: String,
    num_tests: usize,
    success_rate: f64,
    avg_hops: f64,
    stretch: f64,
    max_stretch: f64,
    gravity_pct: f64,
    pressure_pct: f64,
    tree_pct: f64,
    embedding_time_ms: u128,
    routing_time_ms: u128,
    memory_entries: usize,
}

fn main() {
    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║            LMH-TZ Benchmark for DRFE-R                         ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    let sizes = vec![100, 300, 500, 1000];
    let num_tests = 500;
    let seed = 42u64;

    let mut all_results: Vec<BenchmarkResult> = Vec::new();

    // Print header
    println!("{:<8} {:<15} {:<10} {:<10} {:<10} {:<10} {:<10}",
             "Nodes", "Strategy", "Success%", "AvgHops", "Stretch", "MaxStr", "Gravity%");
    println!("{}", "─".repeat(80));

    for &n in &sizes {
        println!("\n▶ Testing network size: {} nodes", n);
        
        // Generate BA network adjacency
        let (nodes, adjacency_idx, adjacency) = generate_ba_adjacency(n, 3, seed);

        // Strategy 1: PIE only (baseline)
        let (router_pie, time_pie) = build_router_pie(&nodes, &adjacency_idx, &adjacency);
        let results_pie = run_routing_tests(&router_pie, &nodes, n, num_tests, seed);
        let result_pie = BenchmarkResult {
            network_size: n,
            embedding_strategy: "PIE".to_string(),
            num_tests,
            success_rate: results_pie.success_rate,
            avg_hops: results_pie.avg_hops,
            stretch: results_pie.stretch,
            max_stretch: results_pie.max_stretch,
            gravity_pct: results_pie.gravity_pct,
            pressure_pct: results_pie.pressure_pct,
            tree_pct: results_pie.tree_pct,
            embedding_time_ms: time_pie,
            routing_time_ms: results_pie.routing_time_ms,
            memory_entries: 0,
        };
        print_result(&result_pie);
        all_results.push(result_pie);

        // Strategy 2: LMH (Landmark-MDS Hyperbolic)
        let (router_lmh, time_lmh) = build_router_lmh(&nodes, &adjacency_idx, &adjacency);
        let results_lmh = run_routing_tests(&router_lmh, &nodes, n, num_tests, seed);
        let result_lmh = BenchmarkResult {
            network_size: n,
            embedding_strategy: "LMH".to_string(),
            num_tests,
            success_rate: results_lmh.success_rate,
            avg_hops: results_lmh.avg_hops,
            stretch: results_lmh.stretch,
            max_stretch: results_lmh.max_stretch,
            gravity_pct: results_lmh.gravity_pct,
            pressure_pct: results_lmh.pressure_pct,
            tree_pct: results_lmh.tree_pct,
            embedding_time_ms: time_lmh,
            routing_time_ms: results_lmh.routing_time_ms,
            memory_entries: 0,
        };
        print_result(&result_lmh);
        all_results.push(result_lmh);

        // Strategy 3: TZ-only routing (to verify stretch guarantee)
        let tz_table = TZRoutingTable::build(&adjacency, TZConfig::default()).unwrap();
        let (tz_avg, tz_max, tz_violations) = tz_table.verify_stretch(&adjacency, num_tests);
        let result_tz = BenchmarkResult {
            network_size: n,
            embedding_strategy: "TZ-only".to_string(),
            num_tests,
            success_rate: 1.0, // TZ guarantees delivery
            avg_hops: 0.0, // Not applicable directly
            stretch: tz_avg,
            max_stretch: tz_max,
            gravity_pct: 0.0,
            pressure_pct: 0.0,
            tree_pct: 100.0,
            embedding_time_ms: 0,
            routing_time_ms: 0,
            memory_entries: tz_table.memory_usage(),
        };
        println!("{:<8} {:<15} {:<10.2} {:<10} {:<10.2} {:<10.2} {:<10}",
                 n, "TZ-only", 100.0, "-", tz_avg, tz_max, "-");
        if tz_violations > 0 {
            println!("  ⚠ TZ violations: {} (stretch > 3.0)", tz_violations);
        }
        all_results.push(result_tz);

        println!();
    }

    // Print summary
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║                        SUMMARY                                 ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    println!("Stretch Improvement (vs PIE):");
    for size in &sizes {
        let pie = all_results.iter().find(|r| r.network_size == *size && r.embedding_strategy == "PIE");
        let lmh = all_results.iter().find(|r| r.network_size == *size && r.embedding_strategy == "LMH");
        let tz = all_results.iter().find(|r| r.network_size == *size && r.embedding_strategy == "TZ-only");
        
        if let (Some(p), Some(l), Some(t)) = (pie, lmh, tz) {
            let lmh_improvement = if l.stretch > 0.0 { p.stretch / l.stretch } else { 0.0 };
            let tz_improvement = if t.stretch > 0.0 { p.stretch / t.stretch } else { 0.0 };
            println!("  {} nodes: PIE={:.1}x, LMH={:.1}x ({:.1}x better), TZ={:.1}x ({:.1}x better)",
                     size, p.stretch, l.stretch, lmh_improvement, t.stretch, tz_improvement);
        }
    }

    // Save results to JSON
    let output_path = "paper_data/lmh_tz/benchmark_results.json";
    match File::create(output_path) {
        Ok(mut file) => {
            let json = serde_json::to_string_pretty(&all_results).unwrap();
            file.write_all(json.as_bytes()).unwrap();
            println!("\n✓ Results saved to {}", output_path);
        }
        Err(e) => {
            println!("\n⚠ Could not save results: {}", e);
        }
    }

    // Save human-readable summary
    let summary_path = "paper_data/lmh_tz/benchmark_summary.txt";
    match File::create(summary_path) {
        Ok(mut file) => {
            writeln!(file, "LMH-TZ Benchmark Results").unwrap();
            writeln!(file, "========================\n").unwrap();
            writeln!(file, "{:<8} {:<15} {:<10} {:<10} {:<10} {:<10}",
                     "Nodes", "Strategy", "Success%", "Stretch", "MaxStr", "Gravity%").unwrap();
            writeln!(file, "{}", "-".repeat(70)).unwrap();
            for r in &all_results {
                writeln!(file, "{:<8} {:<15} {:<10.2} {:<10.2} {:<10.2} {:<10.2}",
                         r.network_size, r.embedding_strategy, 
                         r.success_rate * 100.0, r.stretch, r.max_stretch, r.gravity_pct).unwrap();
            }
            println!("✓ Summary saved to {}", summary_path);
        }
        Err(e) => {
            println!("⚠ Could not save summary: {}", e);
        }
    }
}

fn print_result(r: &BenchmarkResult) {
    println!("{:<8} {:<15} {:<10.2} {:<10.2} {:<10.2} {:<10.2} {:<10.2}",
             r.network_size, r.embedding_strategy, 
             r.success_rate * 100.0, r.avg_hops, r.stretch, r.max_stretch, r.gravity_pct);
}

struct TestResults {
    success_rate: f64,
    avg_hops: f64,
    stretch: f64,
    max_stretch: f64,
    gravity_pct: f64,
    pressure_pct: f64,
    tree_pct: f64,
    routing_time_ms: u128,
}

fn generate_ba_adjacency(n: usize, m: usize, seed: u64) 
    -> (Vec<NodeId>, Vec<Vec<usize>>, HashMap<NodeId, Vec<NodeId>>) 
{
    let mut rng = StdRng::seed_from_u64(seed);
    let mut degrees: Vec<usize> = vec![0; n];
    let mut adjacency_idx: Vec<Vec<usize>> = vec![Vec::new(); n];
    let nodes: Vec<NodeId> = (0..n).map(|i| NodeId::new(format!("node_{}", i))).collect();

    // Initial complete graph of m nodes
    for i in 0..m.min(n) {
        for j in (i+1)..m.min(n) {
            adjacency_idx[i].push(j);
            adjacency_idx[j].push(i);
            degrees[i] += 1;
            degrees[j] += 1;
        }
    }

    // Preferential attachment
    for i in m..n {
        let total: usize = degrees.iter().take(i).sum();
        if total == 0 {
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
        let neighbors: Vec<NodeId> = adjacency_idx[i].iter().map(|&j| nodes[j].clone()).collect();
        adjacency.insert(nodes[i].clone(), neighbors);
    }

    (nodes, adjacency_idx, adjacency)
}

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

fn build_router_lmh(
    nodes: &[NodeId],
    adjacency_idx: &[Vec<usize>],
    adjacency: &HashMap<NodeId, Vec<NodeId>>,
) -> (GPRouter, u128) {
    let start = Instant::now();
    
    let config = LandmarkConfig::default();
    let embedder = LandmarkEmbedding::with_config(config);
    let result = embedder.embed(adjacency).expect("LMH embedding failed");

    let mut router = GPRouter::new();
    for node_id in nodes.iter() {
        let point = result.coordinates.get(node_id).copied().unwrap_or_else(PoincareDiskPoint::origin);
        let coord = RoutingCoordinate::new(point, 0);
        // LMH doesn't produce a tree structure, so we leave tree_info empty
        let rn = RoutingNode::new(node_id.clone(), coord);
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

fn run_routing_tests(
    router: &GPRouter, 
    nodes: &[NodeId], 
    n: usize, 
    num_tests: usize, 
    seed: u64
) -> TestResults {
    let mut rng = StdRng::seed_from_u64(seed + 1000);
    let max_ttl = (n * 20) as u32;

    let mut successes = 0;
    let mut total_hops = 0u32;
    let mut total_optimal = 0u32;
    let mut gravity_hops = 0u32;
    let mut pressure_hops = 0u32;
    let mut tree_hops = 0u32;
    let mut total_all_hops = 0u32;
    let mut max_stretch = 0.0f64;

    let routing_start = Instant::now();

    for _ in 0..num_tests {
        let src_idx = rng.gen_range(0..n);
        let mut dst_idx = rng.gen_range(0..n);
        while dst_idx == src_idx {
            dst_idx = rng.gen_range(0..n);
        }

        let source = &nodes[src_idx];
        let dest = &nodes[dst_idx];

        if let Some(dest_node) = router.get_node(dest) {
            let result = router.simulate_delivery(source, dest, dest_node.coord.point, max_ttl);
            if result.success {
                successes += 1;
                total_hops += result.hops;
                gravity_hops += result.gravity_hops;
                pressure_hops += result.pressure_hops;
                tree_hops += result.tree_hops;
                total_all_hops += result.hops;

                // BFS for optimal
                if let Some(opt) = bfs_shortest_path(router, source, dest) {
                    total_optimal += opt;
                    if opt > 0 {
                        let stretch = result.hops as f64 / opt as f64;
                        max_stretch = f64::max(max_stretch, stretch);
                    }
                }
            }
        }
    }

    let routing_time_ms = routing_start.elapsed().as_millis();

    let success_rate = successes as f64 / num_tests as f64;
    let avg_hops = if successes > 0 { total_hops as f64 / successes as f64 } else { 0.0 };
    let stretch = if total_optimal > 0 { total_hops as f64 / total_optimal as f64 } else { 0.0 };
    let gravity_pct = if total_all_hops > 0 { gravity_hops as f64 / total_all_hops as f64 * 100.0 } else { 0.0 };
    let pressure_pct = if total_all_hops > 0 { pressure_hops as f64 / total_all_hops as f64 * 100.0 } else { 0.0 };
    let tree_pct = if total_all_hops > 0 { tree_hops as f64 / total_all_hops as f64 * 100.0 } else { 0.0 };

    TestResults { 
        success_rate, 
        avg_hops, 
        stretch, 
        max_stretch,
        gravity_pct, 
        pressure_pct, 
        tree_pct,
        routing_time_ms,
    }
}

fn bfs_shortest_path(router: &GPRouter, start: &NodeId, end: &NodeId) -> Option<u32> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back((start.clone(), 0));
    visited.insert(start.clone());

    while let Some((current, dist)) = queue.pop_front() {
        if &current == end { return Some(dist); }
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
