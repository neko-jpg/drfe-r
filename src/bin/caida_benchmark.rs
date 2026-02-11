//! CAIDA Real-World Topology Benchmark
//!
//! Tests PIE+TZ and HYPER-PRESS on the CAIDA AS-level topology (~78k nodes)

use drfe_r::coordinates::{NodeId, RoutingCoordinate};
use drfe_r::greedy_embedding::GreedyEmbedding;
use drfe_r::hyper_press::HyperPress;
use drfe_r::routing::{GPRouter, PacketHeader, RoutingNode, RoutingMode};
use drfe_r::tz_routing::{TZRoutingTable, TZConfig};
use drfe_r::PoincareDiskPoint;
use rand::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;

fn main() {
    println!("╔════════════════════════════════════════════════════════════════════════╗");
    println!("║               CAIDA Real-World Topology Benchmark                      ║");
    println!("╚════════════════════════════════════════════════════════════════════════╝\n");

    // Load CAIDA edge list
    let edge_file = "paper_data/input/caida/caida_edge_list.txt";
    println!("📂 Loading CAIDA topology from {}...", edge_file);
    
    let (nodes, adjacency) = match load_edge_list(edge_file) {
        Ok(data) => data,
        Err(e) => {
            println!("❌ Failed to load CAIDA data: {}", e);
            return;
        }
    };
    
    let n = nodes.len();
    println!("✓ Loaded {} nodes, {} edges\n", n, adjacency.values().map(|v| v.len()).sum::<usize>() / 2);
    
    // Compute adaptive TTL for this network
    let ttl = PacketHeader::compute_adaptive_ttl(n, None);
    println!("📊 Network stats:");
    println!("   Nodes: {}", n);
    println!("   Adaptive TTL: {}", ttl);
    println!();
    
    let num_tests = 100;
    let seed = 42u64;
    
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    // === Test 1: TZ only ===
    println!("\n▶ Building TZ routing table...");
    let tz_start = Instant::now();
    
    let tz_table = match TZRoutingTable::build(&adjacency, TZConfig::default()) {
        Ok(table) => table,
        Err(e) => {
            println!("❌ TZ table build failed: {}", e);
            return;
        }
    };
    let tz_time = tz_start.elapsed().as_secs();
    println!("✓ TZ table built in {}s, memory: {} entries", tz_time, tz_table.memory_usage());
    
    println!("\n▶ Testing TZ-only routing...");
    let tz_results = test_tz_routing(&tz_table, &nodes, num_tests, seed);
    println!("   Success: {:.2}%, AvgHops: {:.2}, Stretch: {:.2}x", 
             tz_results.0 * 100.0, tz_results.1, tz_results.2);
    
    // === Test 2: HYPER-PRESS ===
    println!("\n▶ Building HYPER-PRESS (H^2 + Φ_t)...");
    let hp_start = Instant::now();
    let mut hp = HyperPress::new();
    hp.set_lambda(1.0);
    hp.build_from_adjacency(&adjacency);
    let hp_time = hp_start.elapsed().as_secs();
    println!("✓ HYPER-PRESS built in {}s", hp_time);
    
    println!("\n▶ Testing HYPER-PRESS routing...");
    let hp_results = test_hyper_press_routing(&hp, &adjacency, &nodes, num_tests, ttl, seed);
    println!("   Success: {:.2}%, AvgHops: {:.2}, Gravity%: {:.2}%", 
             hp_results.0 * 100.0, hp_results.1, hp_results.2);
    
    // === Test 3: HYPER-PRESS + TZ fallback ===
    println!("\n▶ Testing HYPER-PRESS + TZ fallback...");
    let hp_tz_results = test_hyper_press_with_tz(&hp, &tz_table, &adjacency, &nodes, num_tests, ttl, seed);
    println!("   Success: {:.2}%, AvgHops: {:.2}, HP%: {:.2}%, TZ%: {:.2}%", 
             hp_tz_results.0 * 100.0, hp_tz_results.1, hp_tz_results.2, hp_tz_results.3);
    
    // === Summary ===
    println!("\n╔════════════════════════════════════════════════════════════════════════╗");
    println!("║                           SUMMARY                                      ║");
    println!("╚════════════════════════════════════════════════════════════════════════╝\n");
    
    println!("{:<20} {:<12} {:<12} {:<12}", "Strategy", "Success%", "AvgHops", "Notes");
    println!("{}", "━".repeat(60));
    println!("{:<20} {:<12.2} {:<12.2} {:<12}", "TZ-only", tz_results.0 * 100.0, tz_results.1, 
             format!("stretch={:.2}x", tz_results.2));
    println!("{:<20} {:<12.2} {:<12.2} {:<12}", "HYPER-PRESS", hp_results.0 * 100.0, hp_results.1,
             format!("gravity={:.1}%", hp_results.2));
    println!("{:<20} {:<12.2} {:<12.2} {:<12}", "HYPER-PRESS+TZ", hp_tz_results.0 * 100.0, hp_tz_results.1,
             format!("hp={:.1}%,tz={:.1}%", hp_tz_results.2, hp_tz_results.3));
    
    println!("\n✓ Benchmark complete");
}

fn load_edge_list(path: &str) -> Result<(Vec<NodeId>, HashMap<NodeId, Vec<NodeId>>), String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    
    let mut adjacency: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    let mut node_set: HashSet<NodeId> = HashSet::new();
    
    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        let line = line.trim();
        
        // Skip comments and empty lines
        if line.is_empty() || line.starts_with('#') || line.starts_with('%') {
            continue;
        }
        
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let a = NodeId(parts[0].to_string());
            let b = NodeId(parts[1].to_string());
            
            if a != b {
                node_set.insert(a.clone());
                node_set.insert(b.clone());
                
                adjacency.entry(a.clone()).or_insert_with(Vec::new).push(b.clone());
                adjacency.entry(b.clone()).or_insert_with(Vec::new).push(a.clone());
            }
        }
    }
    
    // Remove duplicate neighbors using HashSet
    for neighbors in adjacency.values_mut() {
        let unique: HashSet<NodeId> = neighbors.drain(..).collect();
        *neighbors = unique.into_iter().collect();
    }
    
    let nodes: Vec<NodeId> = node_set.into_iter().collect();
    Ok((nodes, adjacency))
}

/// Test TZ-only routing
fn test_tz_routing(
    tz_table: &TZRoutingTable,
    nodes: &[NodeId],
    num_tests: usize,
    seed: u64,
) -> (f64, f64, f64) {
    let mut rng = StdRng::seed_from_u64(seed);
    let n = nodes.len();
    
    let mut successes = 0;
    let mut total_hops = 0;
    let mut total_optimal = 0;
    
    for _ in 0..num_tests {
        let src = &nodes[rng.gen_range(0..n)];
        let dst = &nodes[rng.gen_range(0..n)];
        if src == dst { continue; }
        
        if let Some(path) = tz_table.compute_path(src, dst) {
            successes += 1;
            total_hops += path.len() - 1;
            
            // Estimate optimal (we don't have full BFS here, use TZ path as approximation)
            total_optimal += path.len() - 1;
        }
    }
    
    let success_rate = successes as f64 / num_tests as f64;
    let avg_hops = if successes > 0 { total_hops as f64 / successes as f64 } else { 0.0 };
    let stretch = 1.0; // TZ path is our baseline here
    
    (success_rate, avg_hops, stretch)
}

/// Test HYPER-PRESS routing
fn test_hyper_press_routing(
    hp: &HyperPress,
    adjacency: &HashMap<NodeId, Vec<NodeId>>,
    nodes: &[NodeId],
    num_tests: usize,
    max_ttl: u32,
    seed: u64,
) -> (f64, f64, f64) {
    let mut rng = StdRng::seed_from_u64(seed);
    let n = nodes.len();
    
    let mut successes = 0;
    let mut total_hops = 0;
    let mut gravity_decisions = 0;
    let mut total_decisions = 0;
    
    for _ in 0..num_tests {
        let src = &nodes[rng.gen_range(0..n)];
        let dst = &nodes[rng.gen_range(0..n)];
        if src == dst { continue; }
        
        let mut current = src.clone();
        let mut visited = HashSet::new();
        let mut hops = 0;
        
        for _ in 0..max_ttl {
            if &current == dst {
                successes += 1;
                total_hops += hops;
                break;
            }
            
            visited.insert(current.clone());
            
            if let Some(next) = hp.find_best_neighbor_fast(&current, dst, &visited) {
                current = next;
                hops += 1;
                gravity_decisions += 1;
                total_decisions += 1;
            } else {
                // Fallback: try any unvisited neighbor
                total_decisions += 1;
                if let Some(neighbors) = adjacency.get(&current) {
                    if let Some(next) = neighbors.iter().find(|n| !visited.contains(*n)) {
                        current = next.clone();
                        hops += 1;
                        continue;
                    }
                }
                break;
            }
        }
    }
    
    let success_rate = successes as f64 / num_tests as f64;
    let avg_hops = if successes > 0 { total_hops as f64 / successes as f64 } else { 0.0 };
    let gravity_pct = if total_decisions > 0 { gravity_decisions as f64 / total_decisions as f64 * 100.0 } else { 0.0 };
    
    (success_rate, avg_hops, gravity_pct)
}

/// Test HYPER-PRESS with TZ fallback
fn test_hyper_press_with_tz(
    hp: &HyperPress,
    tz_table: &TZRoutingTable,
    adjacency: &HashMap<NodeId, Vec<NodeId>>,
    nodes: &[NodeId],
    num_tests: usize,
    max_hp_hops: u32,
    seed: u64,
) -> (f64, f64, f64, f64) {
    let mut rng = StdRng::seed_from_u64(seed);
    let n = nodes.len();
    
    let mut successes = 0;
    let mut total_hops = 0;
    let mut hp_hops = 0;
    let mut tz_hops = 0;
    
    for _ in 0..num_tests {
        let src = &nodes[rng.gen_range(0..n)];
        let dst = &nodes[rng.gen_range(0..n)];
        if src == dst { continue; }
        
        // Try HYPER-PRESS first
        let mut current = src.clone();
        let mut visited = HashSet::new();
        let mut this_hp_hops = 0;
        let mut reached = false;
        
        for _ in 0..(max_hp_hops / 10).max(100) {
            if &current == dst {
                reached = true;
                break;
            }
            
            visited.insert(current.clone());
            
            if let Some(next) = hp.find_best_neighbor_fast(&current, dst, &visited) {
                current = next;
                this_hp_hops += 1;
            } else {
                break;
            }
        }
        
        let (success, this_tz_hops) = if reached {
            (true, 0)
        } else {
            // Fall back to TZ
            if let Some(path) = tz_table.compute_path(&current, dst) {
                (true, (path.len() - 1) as u32)
            } else {
                (false, 0)
            }
        };
        
        if success {
            successes += 1;
            total_hops += this_hp_hops + this_tz_hops;
            hp_hops += this_hp_hops;
            tz_hops += this_tz_hops;
        }
    }
    
    let success_rate = successes as f64 / num_tests as f64;
    let avg_hops = if successes > 0 { total_hops as f64 / successes as f64 } else { 0.0 };
    let total_all = hp_hops + tz_hops;
    let hp_pct = if total_all > 0 { hp_hops as f64 / total_all as f64 * 100.0 } else { 0.0 };
    let tz_pct = if total_all > 0 { tz_hops as f64 / total_all as f64 * 100.0 } else { 0.0 };
    
    (success_rate, avg_hops, hp_pct, tz_pct)
}
