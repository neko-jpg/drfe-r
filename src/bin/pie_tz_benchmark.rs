//! PIE + TZ Benchmark
//!
//! Tests the combination of PIE embedding with TZ fallback routing.
//! This should achieve the best of both worlds:
//! - PIE provides tree-based greedy routing (Gravity mode)
//! - TZ provides stretch ≤ 3 guarantee when Gravity fails
//!
//! Results are saved to paper_data/lmh_tz/

use drfe_r::coordinates::{NodeId, RoutingCoordinate};
use drfe_r::greedy_embedding::GreedyEmbedding;
use drfe_r::routing::{GPRouter, RoutingNode, RoutingMode};
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
    strategy: String,
    num_tests: usize,
    success_rate: f64,
    avg_hops: f64,
    stretch: f64,
    max_stretch: f64,
    gravity_pct: f64,
    tz_pct: f64,
    embedding_time_ms: u128,
    tz_build_time_ms: u128,
    tz_memory_entries: usize,
}

fn main() {
    println!("╔════════════════════════════════════════════════════════════════════════╗");
    println!("║               PIE + TZ Combination Benchmark                           ║");
    println!("╚════════════════════════════════════════════════════════════════════════╝\n");

    let sizes = vec![100, 300, 500, 1000, 2000, 3000];
    let num_tests = 500;
    let seed = 42u64;

    let mut all_results: Vec<BenchmarkResult> = Vec::new();

    println!("{:<6} {:<12} {:<10} {:<8} {:<10} {:<10} {:<10} {:<10}",
             "Nodes", "Strategy", "Success%", "Hops", "Stretch", "MaxStr", "Gravity%", "TZ%");
    println!("{}", "═".repeat(90));

    for &n in &sizes {
        println!("\n▶ Network size: {} nodes", n);
        
        // Generate BA network
        let (nodes, adjacency_idx, adjacency) = generate_ba_adjacency(n, 3, seed);

        // === Strategy 1: PIE only (current baseline) ===
        let (router_pie, time_pie) = build_router_pie(&nodes, &adjacency_idx, &adjacency);
        let results_pie = run_routing_tests(&router_pie, None, &nodes, n, num_tests, seed);
        
        let result_pie = BenchmarkResult {
            network_size: n,
            strategy: "PIE-DFS".to_string(),
            num_tests,
            success_rate: results_pie.success_rate,
            avg_hops: results_pie.avg_hops,
            stretch: results_pie.stretch,
            max_stretch: results_pie.max_stretch,
            gravity_pct: results_pie.gravity_pct,
            tz_pct: 0.0,
            embedding_time_ms: time_pie,
            tz_build_time_ms: 0,
            tz_memory_entries: 0,
        };
        print_result(&result_pie);
        all_results.push(result_pie);

        // === Strategy 2: PIE + TZ fallback ===
        let tz_start = Instant::now();
        let tz_table = TZRoutingTable::build(&adjacency, TZConfig::default()).unwrap();
        let tz_build_time = tz_start.elapsed().as_millis();
        let tz_memory = tz_table.memory_usage();

        let results_pie_tz = run_routing_tests_with_tz(&router_pie, &tz_table, &nodes, n, num_tests, seed);
        
        let result_pie_tz = BenchmarkResult {
            network_size: n,
            strategy: "PIE+TZ".to_string(),
            num_tests,
            success_rate: results_pie_tz.success_rate,
            avg_hops: results_pie_tz.avg_hops,
            stretch: results_pie_tz.stretch,
            max_stretch: results_pie_tz.max_stretch,
            gravity_pct: results_pie_tz.gravity_pct,
            tz_pct: results_pie_tz.tz_pct,
            embedding_time_ms: time_pie,
            tz_build_time_ms: tz_build_time,
            tz_memory_entries: tz_memory,
        };
        print_result(&result_pie_tz);
        all_results.push(result_pie_tz);

        // Calculate improvement
        let improvement = if results_pie_tz.stretch > 0.0 && results_pie.stretch > 0.0 {
            results_pie.stretch / results_pie_tz.stretch
        } else { 0.0 };
        
        if results_pie_tz.max_stretch <= 3.0 {
            println!("  ✓ Stretch guarantee achieved! Max stretch: {:.2}x (≤3.0)", results_pie_tz.max_stretch);
        }
        println!("  → Stretch improvement: {:.1}x better ({:.1}x → {:.1}x)", 
                 improvement, results_pie.stretch, results_pie_tz.stretch);
    }

    // Summary
    println!("\n╔════════════════════════════════════════════════════════════════════════╗");
    println!("║                           SUMMARY                                      ║");
    println!("╚════════════════════════════════════════════════════════════════════════╝\n");

    println!("Stretch Comparison:");
    println!("{:<8} {:<15} {:<15} {:<15}", "Nodes", "PIE-DFS", "PIE+TZ", "Improvement");
    println!("{}", "-".repeat(55));
    
    for size in &sizes {
        let pie = all_results.iter().find(|r| r.network_size == *size && r.strategy == "PIE-DFS");
        let pie_tz = all_results.iter().find(|r| r.network_size == *size && r.strategy == "PIE+TZ");
        
        if let (Some(p), Some(pt)) = (pie, pie_tz) {
            let improvement = if pt.stretch > 0.0 { p.stretch / pt.stretch } else { 0.0 };
            println!("{:<8} {:<15.2} {:<15.2} {:<15.1}x", 
                     size, p.stretch, pt.stretch, improvement);
        }
    }

    println!("\nMax Stretch (should be ≤ 3.0 for PIE+TZ):");
    for size in &sizes {
        let pie_tz = all_results.iter().find(|r| r.network_size == *size && r.strategy == "PIE+TZ");
        if let Some(pt) = pie_tz {
            let status = if pt.max_stretch <= 3.0 { "✓" } else { "✗" };
            println!("  {} nodes: {:.2}x {}", size, pt.max_stretch, status);
        }
    }

    // Save results
    let output_path = "paper_data/lmh_tz/pie_tz_results.json";
    if let Ok(mut file) = File::create(output_path) {
        let json = serde_json::to_string_pretty(&all_results).unwrap();
        file.write_all(json.as_bytes()).unwrap();
        println!("\n✓ Results saved to {}", output_path);
    }

    let summary_path = "paper_data/lmh_tz/pie_tz_summary.txt";
    if let Ok(mut file) = File::create(summary_path) {
        writeln!(file, "PIE + TZ Combination Results").unwrap();
        writeln!(file, "============================\n").unwrap();
        writeln!(file, "{:<8} {:<12} {:<10} {:<10} {:<10} {:<10} {:<10}",
                 "Nodes", "Strategy", "Success%", "Stretch", "MaxStr", "Gravity%", "TZ%").unwrap();
        writeln!(file, "{}", "-".repeat(80)).unwrap();
        for r in &all_results {
            writeln!(file, "{:<8} {:<12} {:<10.2} {:<10.2} {:<10.2} {:<10.2} {:<10.2}",
                     r.network_size, r.strategy, 
                     r.success_rate * 100.0, r.stretch, r.max_stretch, 
                     r.gravity_pct, r.tz_pct).unwrap();
        }
        println!("✓ Summary saved to {}", summary_path);
    }
}

fn print_result(r: &BenchmarkResult) {
    println!("{:<6} {:<12} {:<10.2} {:<8.2} {:<10.2} {:<10.2} {:<10.2} {:<10.2}",
             r.network_size, r.strategy, 
             r.success_rate * 100.0, r.avg_hops, r.stretch, r.max_stretch, 
             r.gravity_pct, r.tz_pct);
}

struct TestResults {
    success_rate: f64,
    avg_hops: f64,
    stretch: f64,
    max_stretch: f64,
    gravity_pct: f64,
    tz_pct: f64,
}

fn generate_ba_adjacency(n: usize, m: usize, seed: u64) 
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

/// Run standard routing tests (using GPRouter's built-in Gravity-Pressure-Tree)
fn run_routing_tests(
    router: &GPRouter, 
    _tz_table: Option<&TZRoutingTable>,
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
    let mut total_all_hops = 0u32;
    let mut max_stretch = 0.0f64;

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
                total_all_hops += result.hops;

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

    let success_rate = successes as f64 / num_tests as f64;
    let avg_hops = if successes > 0 { total_hops as f64 / successes as f64 } else { 0.0 };
    let stretch = if total_optimal > 0 { total_hops as f64 / total_optimal as f64 } else { 0.0 };
    let gravity_pct = if total_all_hops > 0 { gravity_hops as f64 / total_all_hops as f64 * 100.0 } else { 0.0 };

    TestResults { 
        success_rate, 
        avg_hops, 
        stretch, 
        max_stretch,
        gravity_pct, 
        tz_pct: 0.0,
    }
}

/// Run routing tests with TZ fallback when Gravity fails
fn run_routing_tests_with_tz(
    router: &GPRouter, 
    tz_table: &TZRoutingTable,
    nodes: &[NodeId], 
    n: usize, 
    num_tests: usize, 
    seed: u64
) -> TestResults {
    let mut rng = StdRng::seed_from_u64(seed + 1000);
    let max_gravity_attempts = n as u32; // Try Gravity for limited hops

    let mut successes = 0;
    let mut total_hops = 0u32;
    let mut total_optimal = 0u32;
    let mut gravity_hops = 0u32;
    let mut tz_hops = 0u32;
    let mut total_all_hops = 0u32;
    let mut max_stretch = 0.0f64;

    for _ in 0..num_tests {
        let src_idx = rng.gen_range(0..n);
        let mut dst_idx = rng.gen_range(0..n);
        while dst_idx == src_idx {
            dst_idx = rng.gen_range(0..n);
        }

        let source = &nodes[src_idx];
        let dest = &nodes[dst_idx];

        // Try pure Gravity first (limited attempt)
        let (gravity_success, g_hops, final_node) = try_gravity_only(router, source, dest, max_gravity_attempts);
        
        let (success, hops, this_gravity_hops, this_tz_hops) = if gravity_success {
            (true, g_hops, g_hops, 0)
        } else {
            // Gravity failed at final_node, use TZ from there
            if let Some(tz_path) = tz_table.compute_path(&final_node, dest) {
                let tz_hops_count = (tz_path.len() - 1) as u32;
                (true, g_hops + tz_hops_count, g_hops, tz_hops_count)
            } else {
                // TZ also failed (shouldn't happen in connected graph)
                (false, 0, 0, 0)
            }
        };

        if success {
            successes += 1;
            total_hops += hops;
            gravity_hops += this_gravity_hops;
            tz_hops += this_tz_hops;
            total_all_hops += hops;

            if let Some(opt) = bfs_shortest_path(router, source, dest) {
                total_optimal += opt;
                if opt > 0 {
                    let stretch = hops as f64 / opt as f64;
                    max_stretch = f64::max(max_stretch, stretch);
                }
            }
        }
    }

    let success_rate = successes as f64 / num_tests as f64;
    let avg_hops = if successes > 0 { total_hops as f64 / successes as f64 } else { 0.0 };
    let stretch = if total_optimal > 0 { total_hops as f64 / total_optimal as f64 } else { 0.0 };
    let gravity_pct = if total_all_hops > 0 { gravity_hops as f64 / total_all_hops as f64 * 100.0 } else { 0.0 };
    let tz_pct = if total_all_hops > 0 { tz_hops as f64 / total_all_hops as f64 * 100.0 } else { 0.0 };

    TestResults { 
        success_rate, 
        avg_hops, 
        stretch, 
        max_stretch,
        gravity_pct, 
        tz_pct,
    }
}

/// Try gravity-only routing with limited attempts
/// Returns (success, hops_taken, final_node_reached)
fn try_gravity_only(
    router: &GPRouter, 
    source: &NodeId, 
    dest: &NodeId, 
    max_attempts: u32
) -> (bool, u32, NodeId) {
    if source == dest {
        return (true, 0, source.clone());
    }

    let dest_coord = match router.get_node(dest) {
        Some(node) => node.coord.point,
        None => return (false, 0, source.clone()),
    };

    let mut current = source.clone();
    let mut hops = 0;
    let mut visited = HashSet::new();
    visited.insert(source.clone());

    for _ in 0..max_attempts {
        if current == *dest {
            return (true, hops, current);
        }

        let current_node = match router.get_node(&current) {
            Some(node) => node,
            None => break,
        };

        let current_dist = current_node.coord.point.hyperbolic_distance(&dest_coord);

        // Find neighbor closest to destination
        let mut best_neighbor: Option<NodeId> = None;
        let mut best_dist = current_dist;

        for neighbor_id in &current_node.neighbors {
            if visited.contains(neighbor_id) {
                continue;
            }
            if let Some(neighbor) = router.get_node(neighbor_id) {
                let dist = neighbor.coord.point.hyperbolic_distance(&dest_coord);
                if dist < best_dist {
                    best_dist = dist;
                    best_neighbor = Some(neighbor_id.clone());
                }
            }
        }

        match best_neighbor {
            Some(next) => {
                visited.insert(next.clone());
                current = next;
                hops += 1;
            }
            None => {
                // Local minimum reached
                break;
            }
        }
    }

    (current == *dest, hops, current)
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
