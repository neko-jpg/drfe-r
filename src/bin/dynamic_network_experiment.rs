//! Dynamic Network Experiment
//!
//! Tests PIE+TZ routing under online network changes:
//!   1. Incremental node additions
//!   2. Incremental node removals
//!   3. Edge churn (add/remove random edges)
//!
//! Measures:
//!   - Coordinate convergence speed after topology changes
//!   - Routing success rate over time
//!   - TZ table staleness effects vs rebuild
//!
//! Usage:
//!   dynamic_network_experiment [--base-size 500] [--churn-rounds 20] [--seed 42]

use drfe_r::coordinates::{NodeId, RoutingCoordinate};
use drfe_r::greedy_embedding::GreedyEmbedding;
use drfe_r::routing::{GPRouter, RoutingNode};
use drfe_r::tz_routing::{TZConfig, TZRoutingTable};
use drfe_r::PoincareDiskPoint;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::Write;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DynamicRoundResult {
    round: usize,
    event_type: String,      // "baseline", "add_nodes", "remove_nodes", "add_edges", "remove_edges"
    nodes_changed: usize,
    total_nodes: usize,
    total_edges: usize,
    // Routing without TZ rebuild
    stale_success_rate: f64,
    stale_stretch: f64,
    // Routing with TZ rebuild
    rebuild_success_rate: f64,
    rebuild_stretch: f64,
    // Timing
    re_embed_ms: u128,
    tz_rebuild_ms: u128,
    // Gravity-only routing
    gravity_success_rate: f64,
    gravity_stretch: f64,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut base_size: usize = 500;
    let mut churn_rounds: usize = 20;
    let mut seed: u64 = 42;
    let mut num_tests: usize = 200;
    let mut output_file = "paper_data/churn/dynamic_network_results.json".to_string();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--base-size" => { if i + 1 < args.len() { base_size = args[i+1].parse().unwrap_or(500); i += 1; } }
            "--churn-rounds" => { if i + 1 < args.len() { churn_rounds = args[i+1].parse().unwrap_or(20); i += 1; } }
            "--seed" => { if i + 1 < args.len() { seed = args[i+1].parse().unwrap_or(42); i += 1; } }
            "-t" | "--tests" => { if i + 1 < args.len() { num_tests = args[i+1].parse().unwrap_or(200); i += 1; } }
            "-o" | "--output" => { if i + 1 < args.len() { output_file = args[i+1].clone(); i += 1; } }
            "--help" | "-h" => {
                println!("Usage: dynamic_network_experiment [OPTIONS]");
                println!("  --base-size N     Initial network size (default: 500)");
                println!("  --churn-rounds N  Number of churn rounds (default: 20)");
                println!("  --seed N          Random seed (default: 42)");
                println!("  -t, --tests N     Routing tests per round (default: 200)");
                println!("  -o, --output F    Output file path");
                return;
            }
            _ => {}
        }
        i += 1;
    }

    std::fs::create_dir_all("paper_data/churn").ok();

    println!("Dynamic Network Experiment");
    println!("  Base size: {}", base_size);
    println!("  Churn rounds: {}", churn_rounds);
    println!("  Seed: {}", seed);
    println!("  Tests/round: {}", num_tests);
    println!();

    let mut rng = StdRng::seed_from_u64(seed);
    let mut results: Vec<DynamicRoundResult> = Vec::new();

    // Build initial BA network
    let (mut adjacency, mut node_list) = generate_initial_ba(base_size, 3, seed);
    let edges_count = count_edges(&adjacency);
    println!("Initial network: {} nodes, {} edges", node_list.len(), edges_count);

    // Build initial PIE+TZ
    let (mut router, mut tz_table) = full_rebuild(&adjacency, seed);
    println!("Initial build complete.\n");

    // Baseline measurement
    {
        let r = measure_routing(&router, &tz_table, &node_list, num_tests, &mut rng);
        let edges = count_edges(&adjacency);
        println!("Round 0 (baseline): success={:.1}%, stretch={:.3}x", r.0 * 100.0, r.1);
        results.push(DynamicRoundResult {
            round: 0,
            event_type: "baseline".to_string(),
            nodes_changed: 0,
            total_nodes: node_list.len(),
            total_edges: edges,
            stale_success_rate: r.0,
            stale_stretch: r.1,
            rebuild_success_rate: r.0,
            rebuild_stretch: r.1,
            re_embed_ms: 0,
            tz_rebuild_ms: 0,
            gravity_success_rate: r.2,
            gravity_stretch: r.3,
        });
    }

    // Churn loop
    for round in 1..=churn_rounds {
        let event_type = match round % 4 {
            1 => "add_nodes",
            2 => "remove_edges",
            3 => "add_edges",
            0 => "remove_nodes",
            _ => unreachable!(),
        };

        let change_size = match event_type {
            "add_nodes" => (base_size as f64 * 0.05).max(5.0) as usize,
            "remove_nodes" => (node_list.len() as f64 * 0.03).max(3.0) as usize,
            "add_edges" | "remove_edges" => (count_edges(&adjacency) as f64 * 0.05).max(5.0) as usize,
            _ => 5,
        };

        println!("Round {}: {} ({})", round, event_type, change_size);

        match event_type {
            "add_nodes" => {
                add_nodes(&mut adjacency, &mut node_list, change_size, 3, &mut rng);
            }
            "remove_nodes" => {
                remove_nodes(&mut adjacency, &mut node_list, change_size, &mut rng);
            }
            "add_edges" => {
                add_random_edges(&mut adjacency, change_size, &mut rng);
            }
            "remove_edges" => {
                remove_random_edges(&mut adjacency, change_size, &mut rng);
            }
            _ => {}
        }

        let total_edges = count_edges(&adjacency);
        println!("  Now: {} nodes, {} edges", node_list.len(), total_edges);

        // Test 1: Stale routing (old PIE + old TZ)
        // We need to update the router for new nodes but keep old embeddings where possible
        let stale_router = patch_router_for_changes(&router, &adjacency, &node_list);
        let stale_result = measure_routing(&stale_router, &tz_table, &node_list, num_tests, &mut rng);

        // Test 2: Full rebuild
        let rebuild_start = Instant::now();
        let (new_router, _) = full_rebuild_pie_only(&adjacency, seed);
        let re_embed_ms = rebuild_start.elapsed().as_millis();

        let tz_start = Instant::now();
        let new_tz = match TZRoutingTable::build(&adjacency, TZConfig { num_landmarks: None, seed }) {
            Ok(t) => t,
            Err(_) => {
                println!("  TZ rebuild failed, skipping round");
                continue;
            }
        };
        let tz_rebuild_ms = tz_start.elapsed().as_millis();

        let rebuild_result = measure_routing(&new_router, &new_tz, &node_list, num_tests, &mut rng);

        // Gravity-only already measured in both
        println!("  Stale:   success={:.1}%, stretch={:.3}x", stale_result.0 * 100.0, stale_result.1);
        println!("  Rebuild: success={:.1}%, stretch={:.3}x (embed={}ms, tz={}ms)",
            rebuild_result.0 * 100.0, rebuild_result.1, re_embed_ms, tz_rebuild_ms);

        results.push(DynamicRoundResult {
            round,
            event_type: event_type.to_string(),
            nodes_changed: change_size,
            total_nodes: node_list.len(),
            total_edges: total_edges,
            stale_success_rate: stale_result.0,
            stale_stretch: stale_result.1,
            rebuild_success_rate: rebuild_result.0,
            rebuild_stretch: rebuild_result.1,
            re_embed_ms,
            tz_rebuild_ms,
            gravity_success_rate: rebuild_result.2,
            gravity_stretch: rebuild_result.3,
        });

        // Accept the rebuild for next round
        router = new_router;
        tz_table = new_tz;
    }

    // Save
    let json = serde_json::to_string_pretty(&results).unwrap();
    let mut f = File::create(&output_file).unwrap();
    f.write_all(json.as_bytes()).unwrap();
    println!("\nResults saved to {}", output_file);

    // Print summary
    println!("\n{}", "=".repeat(70));
    println!("SUMMARY");
    println!("{}", "=".repeat(70));
    println!("{:<6} {:<15} {:<8} {:<14} {:<14} {:<14}", "Round", "Event", "Changed", "Stale%", "Rebuild%", "GravityOnly%");
    println!("{}", "-".repeat(70));
    for r in &results {
        println!("{:<6} {:<15} {:<8} {:<14.1} {:<14.1} {:<14.1}",
            r.round, r.event_type, r.nodes_changed,
            r.stale_success_rate * 100.0,
            r.rebuild_success_rate * 100.0,
            r.gravity_success_rate * 100.0);
    }

    // Degradation analysis
    if results.len() >= 2 {
        let baseline_sr = results[0].rebuild_success_rate;
        let stale_srs: Vec<f64> = results[1..].iter().map(|r| r.stale_success_rate).collect();
        let rebuild_srs: Vec<f64> = results[1..].iter().map(|r| r.rebuild_success_rate).collect();
        let avg_stale = stale_srs.iter().sum::<f64>() / stale_srs.len() as f64;
        let avg_rebuild = rebuild_srs.iter().sum::<f64>() / rebuild_srs.len() as f64;
        let stale_degradation = (baseline_sr - avg_stale) / baseline_sr * 100.0;
        let rebuild_degradation = (baseline_sr - avg_rebuild) / baseline_sr * 100.0;

        println!("\nDegradation from baseline ({:.1}% success):", baseline_sr * 100.0);
        println!("  Stale TZ:   {:.1}% degradation (avg {:.1}% success)", stale_degradation, avg_stale * 100.0);
        println!("  With rebuild: {:.1}% degradation (avg {:.1}% success)", rebuild_degradation, avg_rebuild * 100.0);
    }
}

// ============================================================================
// Network mutation operations
// ============================================================================

fn generate_initial_ba(n: usize, m: usize, seed: u64) -> (HashMap<NodeId, Vec<NodeId>>, Vec<NodeId>) {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut adjacency: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    let mut node_list: Vec<NodeId> = Vec::new();
    let mut degrees: Vec<usize> = Vec::new();

    for i in 0..n {
        let id = NodeId::new(format!("node_{}", i));
        adjacency.insert(id.clone(), Vec::new());
        node_list.push(id);
        degrees.push(0);
    }

    // Initial clique
    for i in 0..m.min(n) {
        for j in (i+1)..m.min(n) {
            adjacency.get_mut(&node_list[i]).unwrap().push(node_list[j].clone());
            adjacency.get_mut(&node_list[j]).unwrap().push(node_list[i].clone());
            degrees[i] += 1;
            degrees[j] += 1;
        }
    }

    for new in m..n {
        let total_deg: usize = degrees.iter().take(new).sum();
        let mut targets = HashSet::new();
        while targets.len() < m.min(new) {
            let r = rng.gen_range(0..total_deg.max(1));
            let mut cumul = 0;
            for j in 0..new {
                cumul += degrees[j];
                if cumul > r {
                    targets.insert(j);
                    break;
                }
            }
        }
        for &t in &targets {
            adjacency.get_mut(&node_list[new]).unwrap().push(node_list[t].clone());
            adjacency.get_mut(&node_list[t]).unwrap().push(node_list[new].clone());
            degrees[new] += 1;
            degrees[t] += 1;
        }
    }

    (adjacency, node_list)
}

fn add_nodes(
    adjacency: &mut HashMap<NodeId, Vec<NodeId>>,
    node_list: &mut Vec<NodeId>,
    count: usize,
    m: usize,
    rng: &mut StdRng,
) {
    let base_idx = node_list.len();
    let existing_nodes: Vec<NodeId> = node_list.clone();
    let existing_count = existing_nodes.len();

    for i in 0..count {
        let id = NodeId::new(format!("node_{}", base_idx + i));
        adjacency.insert(id.clone(), Vec::new());

        // Attach to m random existing nodes
        let mut targets = HashSet::new();
        while targets.len() < m.min(existing_count) {
            targets.insert(rng.gen_range(0..existing_count));
        }

        for &t in &targets {
            adjacency.get_mut(&id).unwrap().push(existing_nodes[t].clone());
            adjacency.get_mut(&existing_nodes[t]).unwrap().push(id.clone());
        }

        node_list.push(id);
    }
}

fn remove_nodes(
    adjacency: &mut HashMap<NodeId, Vec<NodeId>>,
    node_list: &mut Vec<NodeId>,
    count: usize,
    rng: &mut StdRng,
) {
    let count = count.min(node_list.len().saturating_sub(10)); // Keep at least 10 nodes
    let mut to_remove = HashSet::new();
    while to_remove.len() < count {
        let idx = rng.gen_range(0..node_list.len());
        to_remove.insert(node_list[idx].clone());
    }

    for node in &to_remove {
        if let Some(neighbors) = adjacency.remove(node) {
            for neighbor in &neighbors {
                if let Some(adj) = adjacency.get_mut(neighbor) {
                    adj.retain(|n| n != node);
                }
            }
        }
    }

    node_list.retain(|n| !to_remove.contains(n));
}

fn add_random_edges(adjacency: &mut HashMap<NodeId, Vec<NodeId>>, count: usize, rng: &mut StdRng) {
    let nodes: Vec<NodeId> = adjacency.keys().cloned().collect();
    let n = nodes.len();
    if n < 2 { return; }

    let mut added = 0;
    let mut attempts = 0;
    while added < count && attempts < count * 10 {
        let i = rng.gen_range(0..n);
        let j = rng.gen_range(0..n);
        if i == j { attempts += 1; continue; }

        let already = adjacency.get(&nodes[i]).map_or(false, |v| v.contains(&nodes[j]));
        if !already {
            adjacency.get_mut(&nodes[i]).unwrap().push(nodes[j].clone());
            adjacency.get_mut(&nodes[j]).unwrap().push(nodes[i].clone());
            added += 1;
        }
        attempts += 1;
    }
}

fn remove_random_edges(adjacency: &mut HashMap<NodeId, Vec<NodeId>>, count: usize, rng: &mut StdRng) {
    let nodes: Vec<NodeId> = adjacency.keys().cloned().collect();
    let mut removed = 0;
    let mut attempts = 0;

    while removed < count && attempts < count * 10 {
        let idx = rng.gen_range(0..nodes.len());
        let node = &nodes[idx];
        if let Some(neighbors) = adjacency.get(node) {
            if neighbors.len() > 1 {
                let n_idx = rng.gen_range(0..neighbors.len());
                let neighbor = neighbors[n_idx].clone();
                // Check neighbor keeps at least 1 edge too
                if adjacency.get(&neighbor).map_or(false, |v| v.len() > 1) {
                    adjacency.get_mut(node).unwrap().retain(|n| n != &neighbor);
                    adjacency.get_mut(&neighbor).unwrap().retain(|n| n != node);
                    removed += 1;
                }
            }
        }
        attempts += 1;
    }
}

fn count_edges(adjacency: &HashMap<NodeId, Vec<NodeId>>) -> usize {
    adjacency.values().map(|v| v.len()).sum::<usize>() / 2
}

// ============================================================================
// Build/rebuild helpers
// ============================================================================

fn full_rebuild(adjacency: &HashMap<NodeId, Vec<NodeId>>, seed: u64) -> (GPRouter, TZRoutingTable) {
    let router = build_pie_router(adjacency);
    let tz = TZRoutingTable::build(adjacency, TZConfig { num_landmarks: None, seed })
        .expect("TZ build failed");
    (router, tz)
}

fn full_rebuild_pie_only(adjacency: &HashMap<NodeId, Vec<NodeId>>, _seed: u64) -> (GPRouter, ()) {
    (build_pie_router(adjacency), ())
}

fn build_pie_router(adjacency: &HashMap<NodeId, Vec<NodeId>>) -> GPRouter {
    let mut router = GPRouter::new();
    let embedder = GreedyEmbedding::new();
    let result = match embedder.embed(adjacency) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Embedding failed: {}", e);
            return router;
        }
    };

    let mut tree_parent: HashMap<NodeId, Option<NodeId>> = HashMap::new();
    tree_parent.insert(result.root.clone(), None);
    for (parent, children) in &result.tree_children {
        for child in children {
            tree_parent.insert(child.clone(), Some(parent.clone()));
        }
    }

    for node_id in adjacency.keys() {
        let point = result.coordinates.get(node_id).copied()
            .unwrap_or_else(PoincareDiskPoint::origin);
        let coord = RoutingCoordinate::new(point, 0);
        let mut rnode = RoutingNode::new(node_id.clone(), coord);
        let parent = tree_parent.get(node_id).cloned().flatten();
        let children = result.tree_children.get(node_id).cloned().unwrap_or_default();
        rnode.set_tree_info(parent, children);
        router.add_node(rnode);
    }

    let nodes: Vec<NodeId> = adjacency.keys().cloned().collect();
    for node in nodes.iter() {
        if let Some(neighbors) = adjacency.get(node) {
            for neighbor in neighbors {
                // Avoid double-adding by comparing IDs
                if node.0 < neighbor.0 {
                    router.add_edge(node, neighbor);
                }
            }
        }
    }

    router
}

/// "Patch" router: keep old embeddings for existing nodes, add new nodes with random coords
fn patch_router_for_changes(
    old_router: &GPRouter,
    adjacency: &HashMap<NodeId, Vec<NodeId>>,
    node_list: &[NodeId],
) -> GPRouter {
    let mut router = GPRouter::new();

    for node_id in node_list {
        let (coord, parent, children) = if let Some(old_node) = old_router.get_node(node_id) {
            (old_node.coord.clone(), old_node.tree_parent.clone(), old_node.tree_children.clone())
        } else {
            // New node — assign origin coordinate (will hurt gravity but that's the point)
            (RoutingCoordinate::new(PoincareDiskPoint::origin(), 0), None, Vec::new())
        };
        let mut rnode = RoutingNode::new(node_id.clone(), coord);
        rnode.set_tree_info(parent, children);
        router.add_node(rnode);
    }

    for node in node_list {
        if let Some(neighbors) = adjacency.get(node) {
            for neighbor in neighbors {
                if node.0 < neighbor.0 {
                    router.add_edge(node, neighbor);
                }
            }
        }
    }

    router
}

// ============================================================================
// Routing measurement
// ============================================================================

/// Returns (pie_tz_success, pie_tz_stretch, gravity_success, gravity_stretch)
fn measure_routing(
    router: &GPRouter,
    tz_table: &TZRoutingTable,
    node_list: &[NodeId],
    num_tests: usize,
    rng: &mut StdRng,
) -> (f64, f64, f64, f64) {
    let n = node_list.len();
    if n < 2 { return (0.0, 0.0, 0.0, 0.0); }

    let max_hops = n as u32;
    let mut tz_success = 0u32;
    let mut tz_total_h = 0u64;
    let mut tz_total_opt = 0u64;
    let mut grav_success = 0u32;
    let mut grav_total_h = 0u64;
    let mut grav_total_opt = 0u64;

    for _ in 0..num_tests {
        let src_idx = rng.gen_range(0..n);
        let mut dst_idx = rng.gen_range(0..n);
        while dst_idx == src_idx { dst_idx = rng.gen_range(0..n); }

        let src = &node_list[src_idx];
        let dst = &node_list[dst_idx];

        let opt = bfs_distance(router, src, dst);

        // Gravity-only
        let (g_ok, g_hops, stuck_at) = try_gravity(router, src, dst, max_hops);
        if g_ok {
            grav_success += 1;
            if let Some(o) = opt {
                grav_total_h += g_hops as u64;
                grav_total_opt += o as u64;
            }
        }

        // PIE+TZ: gravity then fallback to TZ
        if g_ok {
            tz_success += 1;
            if let Some(o) = opt {
                tz_total_h += g_hops as u64;
                tz_total_opt += o as u64;
            }
        } else {
            // Try TZ from the stuck point
            if let Some(path) = tz_table.compute_path(&stuck_at, dst) {
                let fallback_hops = path.len().saturating_sub(1);
                tz_success += 1;
                if let Some(o) = opt {
                    tz_total_h += g_hops as u64 + fallback_hops as u64;
                    tz_total_opt += o as u64;
                }
            }
        }
    }

    let tz_sr = tz_success as f64 / num_tests as f64;
    let tz_st = if tz_total_opt > 0 { tz_total_h as f64 / tz_total_opt as f64 } else { 0.0 };
    let grav_sr = grav_success as f64 / num_tests as f64;
    let grav_st = if grav_total_opt > 0 { grav_total_h as f64 / grav_total_opt as f64 } else { 0.0 };

    (tz_sr, tz_st, grav_sr, grav_st)
}

fn try_gravity(router: &GPRouter, src: &NodeId, dst: &NodeId, max: u32) -> (bool, u32, NodeId) {
    if src == dst { return (true, 0, src.clone()); }
    let dest_coord = match router.get_node(dst) {
        Some(n) => n.coord.point,
        None => return (false, 0, src.clone()),
    };
    let mut current = src.clone();
    let mut hops = 0u32;
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
            Some(next) => { visited.insert(next.clone()); current = next; hops += 1; }
            None => break,
        }
    }
    (&current == dst, hops, current)
}

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
