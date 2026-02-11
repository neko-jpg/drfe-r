//! Large-Scale Topology Benchmark
//!
//! Tests PIE+TZ routing on larger networks (1000-10000 nodes) across multiple
//! topology types including synthetic community-structure graphs that mimic
//! real-world AS-level topologies.
//!
//! Usage:
//!   large_topology_benchmark [--sizes 1000,2000,5000] [--seeds 42,43,44] [--edge-list FILE]

use drfe_r::coordinates::{NodeId, RoutingCoordinate};
use drfe_r::greedy_embedding::GreedyEmbedding;
use drfe_r::routing::{GPRouter, RoutingNode};
use drfe_r::tz_routing::{TZConfig, TZRoutingTable};
use drfe_r::PoincareDiskPoint;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LargeTopoResult {
    topology: String,
    nodes: usize,
    edges: usize,
    avg_degree: f64,
    seed: u64,
    routing: String,
    success_rate: f64,
    avg_hops: f64,
    stretch: f64,
    max_stretch: f64,
    p95_stretch: f64,
    p99_stretch: f64,
    tz_pct: f64,
    preprocess_ms: u128,
    tz_build_ms: u128,
    tz_landmarks: usize,
    tz_memory_entries: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LargeTopoSummary {
    topology: String,
    nodes: usize,
    num_seeds: usize,
    success_rate_mean: f64,
    success_rate_ci95: f64,
    stretch_mean: f64,
    stretch_ci95: f64,
    max_stretch_mean: f64,
    max_stretch_ci95: f64,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut sizes = vec![1000, 2000, 5000];
    let mut seeds: Vec<u64> = vec![42, 43, 44, 45, 46];
    let mut num_tests = 500;
    let mut edge_list_path: Option<String> = None;
    let mut output_file = "paper_data/real_world/large_topology_results.json".to_string();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--sizes" => {
                if i + 1 < args.len() {
                    sizes = args[i + 1].split(',')
                        .filter_map(|s| s.trim().parse().ok())
                        .collect();
                    i += 1;
                }
            }
            "--seeds" => {
                if i + 1 < args.len() {
                    seeds = args[i + 1].split(',')
                        .filter_map(|s| s.trim().parse().ok())
                        .collect();
                    i += 1;
                }
            }
            "-t" | "--tests" => {
                if i + 1 < args.len() {
                    num_tests = args[i + 1].parse().unwrap_or(500);
                    i += 1;
                }
            }
            "--edge-list" => {
                if i + 1 < args.len() {
                    edge_list_path = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "-o" | "--output" => {
                if i + 1 < args.len() {
                    output_file = args[i + 1].clone();
                    i += 1;
                }
            }
            "--help" | "-h" => {
                println!("Usage: large_topology_benchmark [OPTIONS]");
                println!("  --sizes LIST     Comma-separated node counts (default: 1000,2000,5000)");
                println!("  --seeds LIST     Comma-separated seeds (default: 42,43,44,45,46)");
                println!("  -t, --tests N    Tests per config (default: 500)");
                println!("  --edge-list FILE Real-world edge list file");
                println!("  -o, --output FILE Output file");
                return;
            }
            _ => {}
        }
        i += 1;
    }

    std::fs::create_dir_all("paper_data/real_world").ok();

    println!("Large-Scale Topology Benchmark");
    println!("  Sizes: {:?}", sizes);
    println!("  Seeds: {:?}", seeds);
    println!("  Tests: {}", num_tests);
    println!();

    let topologies = vec!["ba", "ws", "community", "powerlaw_cluster"];
    let mut all_results = Vec::new();

    for &n in &sizes {
        for topo in &topologies {
            for &seed in &seeds {
                println!("\n=== {} n={} seed={} ===", topo, n, seed);
                let start = Instant::now();

                let (nodes, adj_idx, adjacency) = match *topo {
                    "ba" => generate_ba_network(n, 3, seed),
                    "ws" => generate_ws_network(n, 6, 0.3, seed),
                    "community" => generate_community_network(n, seed),
                    "powerlaw_cluster" => generate_powerlaw_cluster(n, seed),
                    _ => continue,
                };

                let edges = adj_idx.iter().map(|v| v.len()).sum::<usize>() / 2;
                let avg_degree = if n > 0 { edges as f64 * 2.0 / n as f64 } else { 0.0 };
                println!("  Nodes: {}, Edges: {}, AvgDeg: {:.1}", nodes.len(), edges, avg_degree);

                // Build PIE
                let pie_start = Instant::now();
                let router = build_router_pie(&nodes, &adj_idx, &adjacency);
                let pie_time = pie_start.elapsed().as_millis();

                // Build TZ
                let tz_start = Instant::now();
                let tz_table = match TZRoutingTable::build(&adjacency, TZConfig { num_landmarks: None, seed }) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("  TZ build failed: {}", e);
                        continue;
                    }
                };
                let tz_build_time = tz_start.elapsed().as_millis();
                let preprocess_time = start.elapsed().as_millis();

                println!("  PIE: {}ms, TZ: {}ms ({} landmarks, {} entries)",
                    pie_time, tz_build_time, tz_table.landmarks.len(), tz_table.memory_usage());

                // Run PIE+TZ routing tests
                let result = run_pie_tz_tests(&router, &tz_table, &nodes, &adjacency, num_tests, seed);

                println!("  Success: {:.1}%, Stretch: {:.3}x, MaxStretch: {:.2}x",
                    result.0 * 100.0, result.1, result.2);

                all_results.push(LargeTopoResult {
                    topology: topo.to_string(),
                    nodes: nodes.len(),
                    edges,
                    avg_degree,
                    seed,
                    routing: "PIE+TZ".to_string(),
                    success_rate: result.0,
                    avg_hops: result.3,
                    stretch: result.1,
                    max_stretch: result.2,
                    p95_stretch: result.4,
                    p99_stretch: result.5,
                    tz_pct: result.6,
                    preprocess_ms: preprocess_time,
                    tz_build_ms: tz_build_time,
                    tz_landmarks: tz_table.landmarks.len(),
                    tz_memory_entries: tz_table.memory_usage(),
                });
            }
        }
    }

    // Also test edge list if provided
    if let Some(path) = &edge_list_path {
        println!("\n=== Real-world edge list: {} ===", path);
        match load_edge_list(path) {
            Ok((nodes, adj_idx)) => {
                let mut adjacency = HashMap::new();
                for (i, neighbors) in adj_idx.iter().enumerate() {
                    adjacency.insert(nodes[i].clone(),
                        neighbors.iter().map(|&j| nodes[j].clone()).collect());
                }
                let n = nodes.len();
                let edges = adj_idx.iter().map(|v| v.len()).sum::<usize>() / 2;
                println!("  Nodes: {}, Edges: {}", n, edges);

                let router = build_router_pie(&nodes, &adj_idx, &adjacency);
                let tz_table = TZRoutingTable::build(&adjacency, TZConfig::default()).unwrap();
                let result = run_pie_tz_tests(&router, &tz_table, &nodes, &adjacency, num_tests, 42);

                all_results.push(LargeTopoResult {
                    topology: "real_world_edgelist".to_string(),
                    nodes: n,
                    edges,
                    avg_degree: edges as f64 * 2.0 / n as f64,
                    seed: 42,
                    routing: "PIE+TZ".to_string(),
                    success_rate: result.0,
                    avg_hops: result.3,
                    stretch: result.1,
                    max_stretch: result.2,
                    p95_stretch: result.4,
                    p99_stretch: result.5,
                    tz_pct: result.6,
                    preprocess_ms: 0,
                    tz_build_ms: 0,
                    tz_landmarks: tz_table.landmarks.len(),
                    tz_memory_entries: tz_table.memory_usage(),
                });
            }
            Err(e) => eprintln!("Failed to load edge list: {}", e),
        }
    }

    // Save results
    let json = serde_json::to_string_pretty(&all_results).unwrap();
    let mut f = File::create(&output_file).unwrap();
    f.write_all(json.as_bytes()).unwrap();
    println!("\nResults saved to {}", output_file);

    // Generate summary with CIs
    generate_summary(&all_results, &seeds);
}

fn run_pie_tz_tests(
    router: &GPRouter,
    tz_table: &TZRoutingTable,
    nodes: &[NodeId],
    _adjacency: &HashMap<NodeId, Vec<NodeId>>,
    num_tests: usize,
    seed: u64,
) -> (f64, f64, f64, f64, f64, f64, f64) {
    // Returns: (success_rate, stretch, max_stretch, avg_hops, p95_stretch, p99_stretch, tz_pct)
    let mut rng = StdRng::seed_from_u64(seed + 1000);
    let n = nodes.len();
    let max_gravity = n as u32;

    let mut successes = 0u32;
    let mut total_hops = 0u64;
    let mut total_optimal = 0u64;
    let mut gravity_hops = 0u64;
    let mut tz_hops = 0u64;
    let mut max_stretch = 0.0f64;
    let mut stretch_samples = Vec::with_capacity(num_tests);

    for _ in 0..num_tests {
        let src = rng.gen_range(0..n);
        let mut dst = rng.gen_range(0..n);
        while dst == src { dst = rng.gen_range(0..n); }

        let (success, g_hops, final_node) = try_gravity_limited(router, &nodes[src], &nodes[dst], max_gravity);

        let (ok, hops, g, t) = if success {
            (true, g_hops as u64, g_hops as u64, 0u64)
        } else {
            if let Some(path) = tz_table.compute_path(&final_node, &nodes[dst]) {
                let tz_h = (path.len().saturating_sub(1)) as u64;
                (true, g_hops as u64 + tz_h, g_hops as u64, tz_h)
            } else {
                (false, 0, 0, 0)
            }
        };

        if ok {
            successes += 1;
            total_hops += hops;
            gravity_hops += g;
            tz_hops += t;

            if let Some(opt) = bfs_distance(router, &nodes[src], &nodes[dst]) {
                total_optimal += opt as u64;
                if opt > 0 {
                    let s = hops as f64 / opt as f64;
                    max_stretch = f64::max(max_stretch, s);
                    stretch_samples.push(s);
                }
            }
        }
    }

    let success_rate = successes as f64 / num_tests as f64;
    let stretch = if total_optimal > 0 { total_hops as f64 / total_optimal as f64 } else { 0.0 };
    let avg_hops = if successes > 0 { total_hops as f64 / successes as f64 } else { 0.0 };
    let all_hops = gravity_hops + tz_hops;
    let tz_pct = if all_hops > 0 { tz_hops as f64 / all_hops as f64 * 100.0 } else { 0.0 };

    stretch_samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p95 = if !stretch_samples.is_empty() {
        stretch_samples[(stretch_samples.len() as f64 * 0.95) as usize % stretch_samples.len()]
    } else { 0.0 };
    let p99 = if !stretch_samples.is_empty() {
        stretch_samples[(stretch_samples.len() as f64 * 0.99) as usize % stretch_samples.len()]
    } else { 0.0 };

    (success_rate, stretch, max_stretch, avg_hops, p95, p99, tz_pct)
}

fn try_gravity_limited(router: &GPRouter, src: &NodeId, dst: &NodeId, max: u32) -> (bool, u32, NodeId) {
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
        for j in (i + 1)..m.min(n) {
            adjacency_idx[i].push(j);
            adjacency_idx[j].push(i);
            degrees[i] += 1;
            degrees[j] += 1;
        }
    }

    for new_node in m..n {
        let total_degree: usize = degrees.iter().take(new_node).sum();
        let mut targets = HashSet::new();

        while targets.len() < m.min(new_node) {
            let r = rng.gen_range(0..total_degree.max(1));
            let mut cumul = 0;
            for j in 0..new_node {
                cumul += degrees[j];
                if cumul > r {
                    targets.insert(j);
                    break;
                }
            }
        }

        for &target in &targets {
            adjacency_idx[new_node].push(target);
            adjacency_idx[target].push(new_node);
            degrees[new_node] += 1;
            degrees[target] += 1;
        }
    }

    let mut adjacency = HashMap::new();
    for (i, neighbors) in adjacency_idx.iter().enumerate() {
        adjacency.insert(nodes[i].clone(), neighbors.iter().map(|&j| nodes[j].clone()).collect());
    }

    (nodes, adjacency_idx, adjacency)
}

fn generate_ws_network(n: usize, k: usize, beta: f64, seed: u64)
    -> (Vec<NodeId>, Vec<Vec<usize>>, HashMap<NodeId, Vec<NodeId>>)
{
    let mut rng = StdRng::seed_from_u64(seed);
    let nodes: Vec<NodeId> = (0..n).map(|i| NodeId::new(format!("node_{}", i))).collect();
    let mut adjacency_idx: Vec<Vec<usize>> = vec![Vec::new(); n];

    // Ring lattice
    for i in 0..n {
        for j in 1..=(k / 2) {
            let neighbor = (i + j) % n;
            if !adjacency_idx[i].contains(&neighbor) {
                adjacency_idx[i].push(neighbor);
                adjacency_idx[neighbor].push(i);
            }
        }
    }

    // Rewire
    for i in 0..n {
        for j in 1..=(k / 2) {
            if rng.gen::<f64>() < beta {
                let old = (i + j) % n;
                let mut new_target = rng.gen_range(0..n);
                let mut tries = 0;
                while (new_target == i || adjacency_idx[i].contains(&new_target)) && tries < n {
                    new_target = rng.gen_range(0..n);
                    tries += 1;
                }
                if tries < n {
                    adjacency_idx[i].retain(|&x| x != old);
                    adjacency_idx[old].retain(|&x| x != i);
                    adjacency_idx[i].push(new_target);
                    adjacency_idx[new_target].push(i);
                }
            }
        }
    }

    let mut adjacency = HashMap::new();
    for (i, neighbors) in adjacency_idx.iter().enumerate() {
        adjacency.insert(nodes[i].clone(), neighbors.iter().map(|&j| nodes[j].clone()).collect());
    }

    (nodes, adjacency_idx, adjacency)
}

/// Community-structure network mimicking real-world AS topologies
fn generate_community_network(n: usize, seed: u64)
    -> (Vec<NodeId>, Vec<Vec<usize>>, HashMap<NodeId, Vec<NodeId>>)
{
    let mut rng = StdRng::seed_from_u64(seed);
    let num_communities = ((n as f64).sqrt().ceil() as usize).max(2);
    let nodes: Vec<NodeId> = (0..n).map(|i| NodeId::new(format!("node_{}", i))).collect();
    let mut adjacency_idx: Vec<Vec<usize>> = vec![Vec::new(); n];

    let nodes_per_comm = n / num_communities;

    // Intra-community: BA within each community
    for comm in 0..num_communities {
        let start = comm * nodes_per_comm;
        let end = if comm == num_communities - 1 { n } else { (comm + 1) * nodes_per_comm };
        let comm_size = end - start;
        let m = 3.min(comm_size.saturating_sub(1));

        // BA-like within community
        let mut local_degrees = vec![0usize; comm_size];
        for i in 0..m.min(comm_size) {
            for j in (i + 1)..m.min(comm_size) {
                let gi = start + i;
                let gj = start + j;
                if !adjacency_idx[gi].contains(&gj) {
                    adjacency_idx[gi].push(gj);
                    adjacency_idx[gj].push(gi);
                    local_degrees[i] += 1;
                    local_degrees[j] += 1;
                }
            }
        }

        for local_new in m..comm_size {
            let total_deg: usize = local_degrees.iter().take(local_new).sum();
            let mut targets = HashSet::new();
            while targets.len() < m.min(local_new) {
                let r = rng.gen_range(0..total_deg.max(1));
                let mut cumul = 0;
                for j in 0..local_new {
                    cumul += local_degrees[j];
                    if cumul > r {
                        targets.insert(j);
                        break;
                    }
                }
            }
            for &t in &targets {
                let gi = start + local_new;
                let gj = start + t;
                if !adjacency_idx[gi].contains(&gj) {
                    adjacency_idx[gi].push(gj);
                    adjacency_idx[gj].push(gi);
                    local_degrees[local_new] += 1;
                    local_degrees[t] += 1;
                }
            }
        }
    }

    // Inter-community links
    let inter_links = (n as f64 * 0.3) as usize;
    for _ in 0..inter_links {
        let c1 = rng.gen_range(0..num_communities);
        let c2 = rng.gen_range(0..num_communities);
        if c1 == c2 { continue; }
        let s1 = c1 * nodes_per_comm;
        let e1 = if c1 == num_communities - 1 { n } else { (c1 + 1) * nodes_per_comm };
        let s2 = c2 * nodes_per_comm;
        let e2 = if c2 == num_communities - 1 { n } else { (c2 + 1) * nodes_per_comm };
        let i = rng.gen_range(s1..e1);
        let j = rng.gen_range(s2..e2);
        if !adjacency_idx[i].contains(&j) {
            adjacency_idx[i].push(j);
            adjacency_idx[j].push(i);
        }
    }

    // Ensure connectivity
    for i in 0..n {
        if adjacency_idx[i].is_empty() {
            let j = rng.gen_range(0..n);
            if i != j {
                adjacency_idx[i].push(j);
                adjacency_idx[j].push(i);
            }
        }
    }

    let mut adjacency = HashMap::new();
    for (i, neighbors) in adjacency_idx.iter().enumerate() {
        adjacency.insert(nodes[i].clone(), neighbors.iter().map(|&j| nodes[j].clone()).collect());
    }

    (nodes, adjacency_idx, adjacency)
}

/// Power-law cluster graph (Holme-Kim model)
fn generate_powerlaw_cluster(n: usize, seed: u64)
    -> (Vec<NodeId>, Vec<Vec<usize>>, HashMap<NodeId, Vec<NodeId>>)
{
    let mut rng = StdRng::seed_from_u64(seed);
    let m = 3;
    let p_triangle = 0.5; // Probability of triad formation
    let nodes: Vec<NodeId> = (0..n).map(|i| NodeId::new(format!("node_{}", i))).collect();
    let mut adjacency_idx: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut degrees = vec![0usize; n];

    // Initial clique
    for i in 0..m.min(n) {
        for j in (i + 1)..m.min(n) {
            adjacency_idx[i].push(j);
            adjacency_idx[j].push(i);
            degrees[i] += 1;
            degrees[j] += 1;
        }
    }

    for new_node in m..n {
        let total_degree: usize = degrees.iter().take(new_node).sum();
        let mut targets = Vec::new();

        // First edge: preferential attachment
        let r = rng.gen_range(0..total_degree.max(1));
        let mut cumul = 0;
        for j in 0..new_node {
            cumul += degrees[j];
            if cumul > r {
                targets.push(j);
                break;
            }
        }

        // Remaining edges: triad formation or preferential attachment
        while targets.len() < m.min(new_node) {
            if rng.gen::<f64>() < p_triangle && !targets.is_empty() {
                // Triad formation: connect to neighbor of last target
                let last = *targets.last().unwrap();
                let neighbors_of_last: Vec<usize> = adjacency_idx[last].iter()
                    .filter(|&&x| x != new_node && !targets.contains(&x))
                    .copied()
                    .collect();
                if !neighbors_of_last.is_empty() {
                    let pick = neighbors_of_last[rng.gen_range(0..neighbors_of_last.len())];
                    targets.push(pick);
                    continue;
                }
            }
            // Preferential attachment fallback
            let r = rng.gen_range(0..total_degree.max(1));
            let mut cumul = 0;
            for j in 0..new_node {
                cumul += degrees[j];
                if cumul > r && !targets.contains(&j) {
                    targets.push(j);
                    break;
                }
            }
        }

        for &t in &targets {
            if !adjacency_idx[new_node].contains(&t) {
                adjacency_idx[new_node].push(t);
                adjacency_idx[t].push(new_node);
                degrees[new_node] += 1;
                degrees[t] += 1;
            }
        }
    }

    let mut adjacency = HashMap::new();
    for (i, neighbors) in adjacency_idx.iter().enumerate() {
        adjacency.insert(nodes[i].clone(), neighbors.iter().map(|&j| nodes[j].clone()).collect());
    }

    (nodes, adjacency_idx, adjacency)
}

fn build_router_pie(
    nodes: &[NodeId],
    adj_idx: &[Vec<usize>],
    adjacency: &HashMap<NodeId, Vec<NodeId>>,
) -> GPRouter {
    let mut router = GPRouter::new();

    let embedder = GreedyEmbedding::new();
    let result = embedder.embed(adjacency).expect("Embedding failed");

    let mut tree_parent: HashMap<NodeId, Option<NodeId>> = HashMap::new();
    tree_parent.insert(result.root.clone(), None);
    for (parent, children) in &result.tree_children {
        for child in children {
            tree_parent.insert(child.clone(), Some(parent.clone()));
        }
    }

    for node_id in nodes {
        let point = result.coordinates.get(node_id).copied()
            .unwrap_or_else(PoincareDiskPoint::origin);
        let coord = RoutingCoordinate::new(point, 0);
        let mut rnode = RoutingNode::new(node_id.clone(), coord);
        let parent = tree_parent.get(node_id).cloned().flatten();
        let children = result.tree_children.get(node_id).cloned().unwrap_or_default();
        rnode.set_tree_info(parent, children);
        router.add_node(rnode);
    }

    for (i, neighbors) in adj_idx.iter().enumerate() {
        for &j in neighbors {
            if i < j {
                router.add_edge(&nodes[i], &nodes[j]);
            }
        }
    }

    router
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

        let parts: Vec<&str> = trimmed.split(|c: char| c.is_whitespace() || c == ',')
            .filter(|p| !p.is_empty())
            .collect();
        if parts.len() < 2 { continue; }

        let u = parts[0].to_string();
        let v = parts[1].to_string();
        if u == v { continue; }

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
        return Err("empty edge list".to_string());
    }

    let adjacency_idx = adjacency_sets.into_iter()
        .map(|set| set.into_iter().collect())
        .collect();

    Ok((nodes, adjacency_idx))
}

fn generate_summary(results: &[LargeTopoResult], seeds: &[u64]) {
    println!("\n{}", "=".repeat(80));
    println!("SUMMARY (averaged over {} seeds)", seeds.len());
    println!("{}", "=".repeat(80));

    let mut grouped: HashMap<(String, usize), Vec<&LargeTopoResult>> = HashMap::new();
    for r in results {
        grouped.entry((r.topology.clone(), r.nodes)).or_default().push(r);
    }

    let mut keys: Vec<_> = grouped.keys().cloned().collect();
    keys.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));

    println!("{:<20} {:<8} {:<12} {:<12} {:<12}", "Topology", "Nodes", "Success%", "Stretch", "MaxStretch");
    println!("{}", "-".repeat(65));

    for key in &keys {
        let entries = &grouped[key];
        let n = entries.len() as f64;
        let sr_mean = entries.iter().map(|r| r.success_rate).sum::<f64>() / n;
        let st_mean = entries.iter().map(|r| r.stretch).sum::<f64>() / n;
        let ms_mean = entries.iter().map(|r| r.max_stretch).sum::<f64>() / n;
        println!("{:<20} {:<8} {:<12.1} {:<12.3} {:<12.2}",
            key.0, key.1, sr_mean * 100.0, st_mean, ms_mean);
    }
}
