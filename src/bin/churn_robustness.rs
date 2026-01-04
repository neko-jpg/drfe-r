//! Dynamic Churn Robustness Benchmark
//!
//! Removes nodes or edges without recomputing coordinates or TZ tables.

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
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct EdgeKey(String, String);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChurnResult {
    removal_rate: f64,
    mode: String,
    selection: String,
    strategy: String,
    nodes_remaining: usize,
    edges_remaining: usize,
    success_rate: f64,
    avg_hops: f64,
    avg_stretch: f64,
    max_stretch: f64,
    p95_stretch: f64,
    p99_stretch: f64,
    tz_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetricSummary {
    mean: f64,
    ci95: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChurnRun {
    seed: u64,
    results: Vec<ChurnResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChurnSummary {
    removal_rate: f64,
    mode: String,
    selection: String,
    strategy: String,
    success_rate: MetricSummary,
    avg_hops: MetricSummary,
    avg_stretch: MetricSummary,
    max_stretch: MetricSummary,
    p95_stretch: MetricSummary,
    p99_stretch: MetricSummary,
    tz_pct: MetricSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChurnReport {
    nodes: usize,
    tests: usize,
    removals: Vec<usize>,
    mode: String,
    selections: Vec<String>,
    seeds: Vec<u64>,
    runs: Vec<ChurnRun>,
    summary: Vec<ChurnSummary>,
}

fn edge_key(a: &NodeId, b: &NodeId) -> EdgeKey {
    if a.0 <= b.0 {
        EdgeKey(a.0.clone(), b.0.clone())
    } else {
        EdgeKey(b.0.clone(), a.0.clone())
    }
}

fn generate_ba_network(
    n: usize,
    m: usize,
    seed: u64,
) -> (Vec<NodeId>, Vec<Vec<usize>>, HashMap<NodeId, Vec<NodeId>>) {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut degrees: Vec<usize> = vec![0; n];
    let mut adjacency_idx: Vec<Vec<usize>> = vec![Vec::new(); n];
    let nodes: Vec<NodeId> = (0..n)
        .map(|i| NodeId::new(format!("node_{}", i)))
        .collect();

    for i in 0..m.min(n) {
        for j in (i + 1)..m.min(n) {
            adjacency_idx[i].push(j);
            adjacency_idx[j].push(i);
            degrees[i] += 1;
            degrees[j] += 1;
        }
    }

    for i in m..n {
        let total: usize = degrees.iter().take(i).sum();
        if total == 0 {
            continue;
        }

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
        adjacency.insert(
            nodes[i].clone(),
            adjacency_idx[i]
                .iter()
                .map(|&j| nodes[j].clone())
                .collect(),
        );
    }

    (nodes, adjacency_idx, adjacency)
}

fn build_router_pie(
    nodes: &[NodeId],
    adjacency_idx: &[Vec<usize>],
    adjacency: &HashMap<NodeId, Vec<NodeId>>,
) -> GPRouter {
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
    for node_id in nodes {
        let point = result
            .coordinates
            .get(node_id)
            .copied()
            .unwrap_or_else(PoincareDiskPoint::origin);
        let coord = RoutingCoordinate::new(point, 0);
        let mut rn = RoutingNode::new(node_id.clone(), coord);
        rn.set_tree_info(
            tree_parent.get(node_id).cloned().flatten(),
            result.tree_children.get(node_id).cloned().unwrap_or_default(),
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

    router
}

fn bfs_distance(router: &GPRouter, start: &NodeId, end: &NodeId) -> Option<u32> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back((start.clone(), 0u32));
    visited.insert(start.clone());

    while let Some((cur, dist)) = queue.pop_front() {
        if &cur == end {
            return Some(dist);
        }
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

fn gravity_path_limited(
    router: &GPRouter,
    src: &NodeId,
    dst: &NodeId,
    max: u32,
) -> (bool, Vec<NodeId>, NodeId) {
    if src == dst {
        return (true, vec![src.clone()], src.clone());
    }

    let dest_coord = match router.get_node(dst) {
        Some(n) => n.coord.point,
        None => return (false, vec![src.clone()], src.clone()),
    };

    let mut current = src.clone();
    let mut visited = HashSet::new();
    let mut path = vec![current.clone()];
    visited.insert(src.clone());

    for _ in 0..max {
        if &current == dst {
            return (true, path, current);
        }

        let node = match router.get_node(&current) {
            Some(n) => n,
            None => break,
        };

        let cur_dist = node.coord.point.hyperbolic_distance(&dest_coord);
        let mut best: Option<NodeId> = None;
        let mut best_dist = cur_dist;

        for neighbor in &node.neighbors {
            if visited.contains(neighbor) {
                continue;
            }
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
                path.push(current.clone());
            }
            None => break,
        }
    }

    (&current == dst, path, current)
}

fn path_is_alive(path: &[NodeId], alive: &HashSet<NodeId>) -> bool {
    if path.len() < 2 {
        return false;
    }
    path.iter().all(|node| alive.contains(node))
}

fn path_avoids_removed_edges(path: &[NodeId], removed_edges: &HashSet<EdgeKey>) -> bool {
    if removed_edges.is_empty() {
        return true;
    }
    for window in path.windows(2) {
        let key = edge_key(&window[0], &window[1]);
        if removed_edges.contains(&key) {
            return false;
        }
    }
    true
}

fn pie_tz_path_pruned(
    router: &GPRouter,
    tz_table: &TZRoutingTable,
    src: &NodeId,
    dst: &NodeId,
    max_gravity: u32,
    alive: &HashSet<NodeId>,
    removed_edges: &HashSet<EdgeKey>,
) -> Option<(Vec<NodeId>, u32, u32)> {
    let (success, mut path, final_node) = gravity_path_limited(router, src, dst, max_gravity);
    let g_hops = path.len().saturating_sub(1) as u32;

    if success {
        if path.last() == Some(dst) && path_is_alive(&path, alive) {
            return Some((path, g_hops, 0));
        }
        return None;
    }

    let tz_path = tz_table.compute_path(&final_node, dst)?;
    if tz_path.len() < 2 {
        return None;
    }
    let tz_hops = tz_path.len().saturating_sub(1) as u32;

    if let Some(last) = path.last() {
        if last == &tz_path[0] {
            path.extend(tz_path.into_iter().skip(1));
        } else {
            path.extend(tz_path);
        }
    } else {
        path = tz_path;
    }

    if path.last() != Some(dst) || !path_is_alive(&path, alive) {
        return None;
    }

    if !path_avoids_removed_edges(&path, removed_edges) {
        return None;
    }

    Some((path, g_hops, tz_hops))
}

fn build_pruned_router(
    base: &GPRouter,
    alive: &HashSet<NodeId>,
    removed_edges: &HashSet<EdgeKey>,
) -> GPRouter {
    let mut router = GPRouter::new();

    for node_id in base.node_ids() {
        if !alive.contains(&node_id) {
            continue;
        }
        if let Some(node) = base.get_node(&node_id) {
            let mut new_node = RoutingNode::new(node.id.clone(), node.coord);
            new_node.set_tree_info(node.tree_parent.clone(), node.tree_children.clone());
            router.add_node(new_node);
        }
    }

    for (u, v) in base.get_edges() {
        if alive.contains(&u) && alive.contains(&v) {
            let key = edge_key(&u, &v);
            if !removed_edges.contains(&key) {
                router.add_edge(&u, &v);
            }
        }
    }

    router
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

fn summarize_churn(runs: &[ChurnRun]) -> Vec<ChurnSummary> {
    let mut by_key: HashMap<String, Vec<&ChurnResult>> = HashMap::new();
    for run in runs {
        for result in &run.results {
            let key = format!(
                "{:.3}|{}|{}|{}",
                result.removal_rate, result.mode, result.selection, result.strategy
            );
            by_key.entry(key).or_default().push(result);
        }
    }

    let mut keys: Vec<String> = by_key.keys().cloned().collect();
    keys.sort();

    let mut summary = Vec::new();
    for key in keys {
        let items = &by_key[&key];
        if items.is_empty() {
            continue;
        }
        let first = items[0];
        let values = |f: fn(&ChurnResult) -> f64| {
            items.iter().map(|r| f(r)).collect::<Vec<f64>>()
        };

        summary.push(ChurnSummary {
            removal_rate: first.removal_rate,
            mode: first.mode.clone(),
            selection: first.selection.clone(),
            strategy: first.strategy.clone(),
            success_rate: metric_summary(&values(|r| r.success_rate)),
            avg_hops: metric_summary(&values(|r| r.avg_hops)),
            avg_stretch: metric_summary(&values(|r| r.avg_stretch)),
            max_stretch: metric_summary(&values(|r| r.max_stretch)),
            p95_stretch: metric_summary(&values(|r| r.p95_stretch)),
            p99_stretch: metric_summary(&values(|r| r.p99_stretch)),
            tz_pct: metric_summary(&values(|r| r.tz_pct)),
        });
    }

    summary
}

fn parse_usize_list(input: &str) -> Vec<usize> {
    input
        .split(',')
        .filter_map(|s| s.trim().parse::<usize>().ok())
        .collect()
}

fn parse_string_list(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_seed_list(input: &str) -> Vec<u64> {
    input
        .split(',')
        .filter_map(|s| s.trim().parse::<u64>().ok())
        .collect()
}

fn ensure_output_dir(path: &str) {
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
}

fn main() {
    println!("Dynamic Churn Robustness Benchmark");
    println!("==================================\n");

    let args: Vec<String> = std::env::args().collect();

    let mut num_nodes = 1000usize;
    let mut num_tests = 1000usize;
    let mut seed = 42u64;
    let mut seeds_override: Option<Vec<u64>> = None;
    let mut output_file = "paper_data/churn/churn_robustness.json".to_string();
    let mut removals = vec![1usize, 5, 10, 20];
    let mut mode = "node".to_string();
    let mut selections = vec!["random".to_string(), "targeted".to_string()];

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--nodes" | "-n" => {
                if i + 1 < args.len() {
                    num_nodes = args[i + 1].parse().unwrap_or(num_nodes);
                    i += 1;
                }
            }
            "--tests" | "-t" => {
                if i + 1 < args.len() {
                    num_tests = args[i + 1].parse().unwrap_or(num_tests);
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
            "--removals" | "-r" => {
                if i + 1 < args.len() {
                    let parsed = parse_usize_list(&args[i + 1]);
                    if !parsed.is_empty() {
                        removals = parsed;
                    }
                    i += 1;
                }
            }
            "--mode" | "-m" => {
                if i + 1 < args.len() {
                    mode = args[i + 1].to_lowercase();
                    i += 1;
                }
            }
            "--selection" | "-s" => {
                if i + 1 < args.len() {
                    let parsed = parse_string_list(&args[i + 1]);
                    if !parsed.is_empty() {
                        selections = parsed;
                    }
                    i += 1;
                }
            }
            "--output" | "-o" => {
                if i + 1 < args.len() {
                    output_file = args[i + 1].clone();
                    i += 1;
                }
            }
            "--help" | "-h" => {
                println!("Usage: churn_robustness [OPTIONS]");
                println!();
                println!("Options:");
                println!("  -n, --nodes NUM         Number of nodes (default: 1000)");
                println!("  -t, --tests NUM         Number of pairs per ratio (default: 1000)");
                println!("  -r, --removals LIST     Comma-separated removal rates (%)");
                println!("  -m, --mode MODE         node or edge (default: node)");
                println!("  -s, --selection LIST    random,targeted (default: both)");
                println!("  --seed NUM              Random seed (default: 42)");
                println!("  --seeds LIST            Comma-separated seeds (overrides --seed)");
                println!("  -o, --output FILE       Output JSON file");
                println!("  -h, --help              Show this help");
                return;
            }
            _ => {}
        }
        i += 1;
    }

    let seeds = seeds_override.unwrap_or_else(|| vec![seed]);

    println!("Configuration:");
    println!("  Nodes:       {}", num_nodes);
    println!("  Tests:       {}", num_tests);
    println!("  Removals:    {:?}%", removals);
    println!("  Mode:        {}", mode);
    println!("  Selection:   {:?}", selections);
    println!("  Seeds:       {:?}", seeds);
    println!("  Output file: {}", output_file);
    println!();

    let mut runs = Vec::new();

    for current_seed in &seeds {
        if seeds.len() > 1 {
            println!("Seed {}", current_seed);
        }

        let (nodes, adjacency_idx, adjacency) =
            generate_ba_network(num_nodes, 3, *current_seed);
        let base_router = build_router_pie(&nodes, &adjacency_idx, &adjacency);
        let tz_table =
            TZRoutingTable::build(&adjacency, TZConfig::default()).expect("TZ build failed");

        let node_ids = base_router.node_ids();
        let edge_list = base_router.get_edges();

        let mut results = Vec::new();
        let mut rng = StdRng::seed_from_u64(*current_seed + 1000);

        for &rate in &removals {
            let removal_frac = rate as f64 / 100.0;

            for selection in &selections {
                let mut removed_nodes = HashSet::new();
                let mut removed_edges = HashSet::new();

                if mode == "node" {
                    let total = node_ids.len();
                    let mut remove_count = (total as f64 * removal_frac).round() as usize;
                    if total > 2 {
                        remove_count = remove_count.min(total - 2);
                    } else {
                        remove_count = 0;
                    }

                    if selection == "targeted" {
                        let mut nodes_by_degree: Vec<(NodeId, usize)> = node_ids
                            .iter()
                            .filter_map(|id| base_router.get_node(id).map(|n| (id.clone(), n.neighbors.len())))
                            .collect();
                        nodes_by_degree.sort_by(|a, b| b.1.cmp(&a.1));
                        for (id, _) in nodes_by_degree.into_iter().take(remove_count) {
                            removed_nodes.insert(id);
                        }
                    } else {
                        let mut shuffled = node_ids.clone();
                        shuffled.shuffle(&mut rng);
                        for id in shuffled.into_iter().take(remove_count) {
                            removed_nodes.insert(id);
                        }
                    }
                } else {
                    let total = edge_list.len();
                    let mut remove_count = (total as f64 * removal_frac).round() as usize;
                    if total > 1 {
                        remove_count = remove_count.min(total - 1);
                    } else {
                        remove_count = 0;
                    }

                    if selection == "targeted" {
                        println!("Targeted selection is not supported for edge removals; using random.");
                    }
                    let mut shuffled = edge_list.clone();
                    shuffled.shuffle(&mut rng);
                    for (u, v) in shuffled.into_iter().take(remove_count) {
                        removed_edges.insert(edge_key(&u, &v));
                    }
                }

                let alive_nodes: HashSet<NodeId> = node_ids
                    .iter()
                    .filter(|id| !removed_nodes.contains(*id))
                    .cloned()
                    .collect();

                let alive_vec: Vec<NodeId> = alive_nodes.iter().cloned().collect();
                if alive_vec.len() < 2 {
                    continue;
                }

                let router = build_pruned_router(&base_router, &alive_nodes, &removed_edges);
                let edges_remaining = router.edge_count();

                let mut pairs = Vec::with_capacity(num_tests);
                for _ in 0..num_tests {
                    let src_idx = rng.gen_range(0..alive_vec.len());
                    let mut dst_idx = rng.gen_range(0..alive_vec.len());
                    while dst_idx == src_idx {
                        dst_idx = rng.gen_range(0..alive_vec.len());
                    }
                    pairs.push((alive_vec[src_idx].clone(), alive_vec[dst_idx].clone()));
                }

                for strategy in ["pie", "pie_tz"] {
                    let mut successes = 0u64;
                    let mut total_hops = 0u64;
                    let mut total_optimal = 0u64;
                    let mut max_stretch = 0.0f64;
                    let mut tz_hops_total = 0u64;
                    let mut all_hops = 0u64;
                    let mut stretch_samples: Vec<f64> = Vec::with_capacity(num_tests);

                    for (src, dst) in &pairs {
                        let result = if strategy == "pie" {
                            let dest_coord = match router.get_node(dst) {
                                Some(n) => n.coord.point,
                                None => continue,
                            };
                            let delivery =
                                router.simulate_delivery(src, dst, dest_coord, (alive_vec.len() * 20) as u32);
                            if delivery.success {
                                Some((delivery.path, 0u64))
                            } else {
                                None
                            }
                        } else {
                            let max_gravity = alive_vec.len() as u32;
                            pie_tz_path_pruned(
                                &router,
                                &tz_table,
                                src,
                                dst,
                                max_gravity,
                                &alive_nodes,
                                &removed_edges,
                            )
                            .map(|(path, _g, tz)| (path, tz as u64))
                        };

                        if let Some((path, tz_hops)) = result {
                            if path.len() < 2 {
                                continue;
                            }
                            successes += 1;
                            let hops = (path.len() - 1) as u64;
                            total_hops += hops;
                            tz_hops_total += tz_hops;
                            all_hops += hops;

                            if let Some(opt) = bfs_distance(&router, src, dst) {
                                total_optimal += opt as u64;
                                if opt > 0 {
                                    let stretch = hops as f64 / opt as f64;
                                    max_stretch = f64::max(max_stretch, stretch);
                                    stretch_samples.push(stretch);
                                }
                            }
                        }
                    }

                    let avg_hops = if successes > 0 {
                        total_hops as f64 / successes as f64
                    } else {
                        0.0
                    };
                    let avg_stretch = if total_optimal > 0 {
                        total_hops as f64 / total_optimal as f64
                    } else {
                        0.0
                    };
                    let tz_pct = if all_hops > 0 {
                        tz_hops_total as f64 / all_hops as f64 * 100.0
                    } else {
                        0.0
                    };
                    let (p95, p99) = stretch_percentiles(&stretch_samples);

                    results.push(ChurnResult {
                        removal_rate: rate as f64,
                        mode: mode.clone(),
                        selection: selection.clone(),
                        strategy: strategy.to_string(),
                        nodes_remaining: alive_vec.len(),
                        edges_remaining,
                        success_rate: successes as f64 / num_tests as f64,
                        avg_hops,
                        avg_stretch,
                        max_stretch,
                        p95_stretch: p95,
                        p99_stretch: p99,
                        tz_pct,
                    });
                }
            }
        }

        runs.push(ChurnRun {
            seed: *current_seed,
            results,
        });

        println!();
    }

    let summary = summarize_churn(&runs);
    let report = ChurnReport {
        nodes: num_nodes,
        tests: num_tests,
        removals,
        mode,
        selections,
        seeds,
        runs,
        summary,
    };

    ensure_output_dir(&output_file);
    let json = serde_json::to_string_pretty(&report).expect("Failed to serialize results");
    let mut file = File::create(&output_file).expect("Failed to create output file");
    file.write_all(json.as_bytes())
        .expect("Failed to write results");

    println!("Results saved to {}", output_file);
}
