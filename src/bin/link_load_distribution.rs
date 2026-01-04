//! Link Load Distribution Benchmark
//!
//! Measures edge load concentration under different routing strategies.

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
struct EdgeLoadEntry {
    u: String,
    v: String,
    count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LoadSummary {
    total_traversals: u64,
    unique_edges_used: usize,
    max_count: u64,
    mean_count: f64,
    p95: u64,
    p99: u64,
    gini: f64,
    top_1_share: f64,
    top_5_share: f64,
    top_10_share: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StrategyResult {
    strategy: String,
    success_rate: f64,
    avg_hops: f64,
    avg_stretch: f64,
    max_stretch: f64,
    p95_stretch: f64,
    p99_stretch: f64,
    load_summary: LoadSummary,
    edge_loads: Vec<EdgeLoadEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetricSummary {
    mean: f64,
    ci95: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LinkLoadRun {
    seed: u64,
    edges: usize,
    strategies: Vec<StrategyResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LinkLoadSummary {
    strategy: String,
    success_rate: MetricSummary,
    avg_hops: MetricSummary,
    avg_stretch: MetricSummary,
    max_stretch: MetricSummary,
    p95_stretch: MetricSummary,
    p99_stretch: MetricSummary,
    gini: MetricSummary,
    top_1_share: MetricSummary,
    top_5_share: MetricSummary,
    top_10_share: MetricSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LinkLoadReport {
    topology: String,
    nodes: usize,
    pairs: usize,
    seeds: Vec<u64>,
    runs: Vec<LinkLoadRun>,
    summary: Vec<LinkLoadSummary>,
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

fn bfs_path(router: &GPRouter, start: &NodeId, end: &NodeId) -> Option<Vec<NodeId>> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut parent: HashMap<NodeId, NodeId> = HashMap::new();

    visited.insert(start.clone());
    queue.push_back(start.clone());

    while let Some(cur) = queue.pop_front() {
        if &cur == end {
            break;
        }
        if let Some(node) = router.get_node(&cur) {
            for neighbor in &node.neighbors {
                if !visited.contains(neighbor) {
                    visited.insert(neighbor.clone());
                    parent.insert(neighbor.clone(), cur.clone());
                    queue.push_back(neighbor.clone());
                }
            }
        }
    }

    if !visited.contains(end) {
        return None;
    }

    let mut path = Vec::new();
    let mut current = end.clone();
    path.push(current.clone());
    while &current != start {
        current = parent.get(&current)?.clone();
        path.push(current.clone());
    }
    path.reverse();
    Some(path)
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

fn pie_tz_path(
    router: &GPRouter,
    tz_table: &TZRoutingTable,
    src: &NodeId,
    dst: &NodeId,
    max_gravity: u32,
) -> Option<Vec<NodeId>> {
    let (success, mut path, final_node) = gravity_path_limited(router, src, dst, max_gravity);
    if success {
        return Some(path);
    }

    let tz_path = tz_table.compute_path(&final_node, dst)?;
    if tz_path.len() <= 1 {
        return None;
    }
    if let Some(last) = path.last() {
        if last == &tz_path[0] {
            path.extend(tz_path.into_iter().skip(1));
        } else {
            path.extend(tz_path);
        }
    } else {
        path = tz_path;
    }
    Some(path)
}

fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn gini_coefficient(values: &[u64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted: Vec<u64> = values.to_vec();
    sorted.sort_unstable();
    let n = sorted.len() as f64;
    let sum: f64 = sorted.iter().map(|v| *v as f64).sum();
    if sum == 0.0 {
        return 0.0;
    }
    let mut cum = 0.0;
    for (i, v) in sorted.iter().enumerate() {
        cum += (i as f64 + 1.0) * (*v as f64);
    }
    (2.0 * cum) / (n * sum) - (n + 1.0) / n
}

fn top_k_share(sorted_desc: &[u64], k: usize) -> f64 {
    if sorted_desc.is_empty() {
        return 0.0;
    }
    let total: u64 = sorted_desc.iter().sum();
    if total == 0 {
        return 0.0;
    }
    let take = k.min(sorted_desc.len());
    let top_sum: u64 = sorted_desc.iter().take(take).sum();
    top_sum as f64 / total as f64
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

fn summarize_link_load(runs: &[LinkLoadRun]) -> Vec<LinkLoadSummary> {
    let mut by_strategy: HashMap<String, Vec<&StrategyResult>> = HashMap::new();
    for run in runs {
        for strategy in &run.strategies {
            by_strategy
                .entry(strategy.strategy.clone())
                .or_default()
                .push(strategy);
        }
    }

    let mut keys: Vec<String> = by_strategy.keys().cloned().collect();
    keys.sort();

    let mut summary = Vec::new();
    for key in keys {
        let items = &by_strategy[&key];
        let values = |f: fn(&StrategyResult) -> f64| {
            items.iter().map(|r| f(r)).collect::<Vec<f64>>()
        };

        summary.push(LinkLoadSummary {
            strategy: key,
            success_rate: metric_summary(&values(|r| r.success_rate)),
            avg_hops: metric_summary(&values(|r| r.avg_hops)),
            avg_stretch: metric_summary(&values(|r| r.avg_stretch)),
            max_stretch: metric_summary(&values(|r| r.max_stretch)),
            p95_stretch: metric_summary(&values(|r| r.p95_stretch)),
            p99_stretch: metric_summary(&values(|r| r.p99_stretch)),
            gini: metric_summary(&values(|r| r.load_summary.gini)),
            top_1_share: metric_summary(&values(|r| r.load_summary.top_1_share)),
            top_5_share: metric_summary(&values(|r| r.load_summary.top_5_share)),
            top_10_share: metric_summary(&values(|r| r.load_summary.top_10_share)),
        });
    }

    summary
}

fn build_summary(edge_counts: &HashMap<EdgeKey, u64>) -> LoadSummary {
    let mut counts: Vec<u64> = edge_counts.values().copied().collect();
    counts.sort_unstable();
    let total_traversals: u64 = counts.iter().sum();
    let unique_edges_used = counts.len();
    let max_count = counts.last().copied().unwrap_or(0);
    let mean_count = if unique_edges_used > 0 {
        total_traversals as f64 / unique_edges_used as f64
    } else {
        0.0
    };
    let p95 = percentile(&counts, 0.95);
    let p99 = percentile(&counts, 0.99);
    let gini = gini_coefficient(&counts);

    let mut desc = counts.clone();
    desc.sort_unstable_by(|a, b| b.cmp(a));
    let top_1_share = top_k_share(&desc, 1);
    let top_5_share = top_k_share(&desc, 5);
    let top_10_share = top_k_share(&desc, 10);

    LoadSummary {
        total_traversals,
        unique_edges_used,
        max_count,
        mean_count,
        p95,
        p99,
        gini,
        top_1_share,
        top_5_share,
        top_10_share,
    }
}

fn ensure_output_dir(path: &str) {
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
}

fn parse_list(input: &str) -> Vec<String> {
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

fn main() {
    println!("Link Load Distribution Benchmark");
    println!("================================\n");

    let args: Vec<String> = std::env::args().collect();

    let mut num_nodes = 1000usize;
    let mut num_pairs = 10_000usize;
    let mut seed = 42u64;
    let mut seeds_override: Option<Vec<u64>> = None;
    let mut output_file = "paper_data/link_load/link_load_ba_1000.json".to_string();
    let mut strategies = vec![
        "pie".to_string(),
        "pie_tz".to_string(),
        "tz_only".to_string(),
        "shortest".to_string(),
    ];

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--nodes" | "-n" => {
                if i + 1 < args.len() {
                    num_nodes = args[i + 1].parse().unwrap_or(num_nodes);
                    i += 1;
                }
            }
            "--pairs" | "-p" => {
                if i + 1 < args.len() {
                    num_pairs = args[i + 1].parse().unwrap_or(num_pairs);
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
            "--strategies" | "-s" => {
                if i + 1 < args.len() {
                    let parsed = parse_list(&args[i + 1]);
                    if !parsed.is_empty() {
                        strategies = parsed;
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
                println!("Usage: link_load_distribution [OPTIONS]");
                println!();
                println!("Options:");
                println!("  -n, --nodes NUM         Number of nodes (default: 1000)");
                println!("  -p, --pairs NUM         Number of pairs (default: 10000)");
                println!("  -s, --strategies LIST   Comma-separated strategies");
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
    println!("  Pairs:       {}", num_pairs);
    println!("  Strategies:  {:?}", strategies);
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
        let router = build_router_pie(&nodes, &adjacency_idx, &adjacency);
        let tz_table =
            TZRoutingTable::build(&adjacency, TZConfig::default()).expect("TZ build failed");

        let mut rng = StdRng::seed_from_u64(*current_seed + 1000);
        let mut pairs = Vec::with_capacity(num_pairs);
        for _ in 0..num_pairs {
            let src = rng.gen_range(0..nodes.len());
            let mut dst = rng.gen_range(0..nodes.len());
            while dst == src {
                dst = rng.gen_range(0..nodes.len());
            }
            pairs.push((nodes[src].clone(), nodes[dst].clone()));
        }

        let mut opt_distances = Vec::with_capacity(num_pairs);
        for (src, dst) in &pairs {
            opt_distances.push(bfs_distance(&router, src, dst));
        }

        let mut results = Vec::new();
        let max_gravity = nodes.len() as u32;

        for strategy in &strategies {
            let mut edge_counts: HashMap<EdgeKey, u64> = HashMap::new();
            let mut successes = 0u64;
            let mut total_hops = 0u64;
            let mut total_optimal = 0u64;
            let mut max_stretch = 0.0f64;
            let mut stretch_samples: Vec<f64> = Vec::with_capacity(num_pairs);

            for (idx, (src, dst)) in pairs.iter().enumerate() {
                let path = match strategy.as_str() {
                    "pie" => {
                        let dest_coord = match router.get_node(dst) {
                            Some(n) => n.coord.point,
                            None => continue,
                        };
                        let result =
                            router.simulate_delivery(src, dst, dest_coord, (num_nodes * 20) as u32);
                        if result.success {
                            Some(result.path)
                        } else {
                            None
                        }
                    }
                    "pie_tz" => pie_tz_path(&router, &tz_table, src, dst, max_gravity),
                    "tz_only" => tz_table.compute_path(src, dst),
                    "shortest" => bfs_path(&router, src, dst),
                    _ => None,
                };

                if let Some(path) = path {
                    if path.len() < 2 {
                        continue;
                    }
                    successes += 1;
                    total_hops += (path.len() - 1) as u64;

                    for window in path.windows(2) {
                        let key = edge_key(&window[0], &window[1]);
                        *edge_counts.entry(key).or_insert(0) += 1;
                    }

                    if let Some(opt) = opt_distances[idx] {
                        total_optimal += opt as u64;
                        if opt > 0 {
                            let stretch = (path.len() as f64 - 1.0) / opt as f64;
                            max_stretch = f64::max(max_stretch, stretch);
                            stretch_samples.push(stretch);
                        }
                    }
                }
            }

            let load_summary = build_summary(&edge_counts);
            let mut edge_loads: Vec<EdgeLoadEntry> = edge_counts
                .into_iter()
                .map(|(k, count)| EdgeLoadEntry {
                    u: k.0,
                    v: k.1,
                    count,
                })
                .collect();
            edge_loads.sort_by(|a, b| b.count.cmp(&a.count));

            let (p95, p99) = stretch_percentiles(&stretch_samples);

            let result = StrategyResult {
                strategy: strategy.clone(),
                success_rate: successes as f64 / num_pairs as f64,
                avg_hops: if successes > 0 {
                    total_hops as f64 / successes as f64
                } else {
                    0.0
                },
                avg_stretch: if total_optimal > 0 {
                    total_hops as f64 / total_optimal as f64
                } else {
                    0.0
                },
                max_stretch,
                p95_stretch: p95,
                p99_stretch: p99,
                load_summary,
                edge_loads,
            };

            println!(
                "{:<10} | Success {:>6.2}% | Avg hops {:>6.2} | Max stretch {:>6.2}",
                result.strategy,
                result.success_rate * 100.0,
                result.avg_hops,
                result.max_stretch
            );

            results.push(result);
        }

        runs.push(LinkLoadRun {
            seed: *current_seed,
            edges: router.edge_count(),
            strategies: results,
        });

        println!();
    }

    let summary = summarize_link_load(&runs);
    let report = LinkLoadReport {
        topology: "ba".to_string(),
        nodes: num_nodes,
        pairs: num_pairs,
        seeds,
        runs,
        summary,
    };

    ensure_output_dir(&output_file);
    let json = serde_json::to_string_pretty(&report).expect("Failed to serialize report");
    let mut file = File::create(&output_file).expect("Failed to create output file");
    file.write_all(json.as_bytes())
        .expect("Failed to write results");

    println!("\nResults saved to {}", output_file);
}
