//! TZ Landmark Sensitivity Benchmark
//!
//! Varies the number of TZ landmarks to measure stretch vs memory tradeoffs.

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
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SensitivityResult {
    landmark_target: usize,
    landmark_count: usize,
    tz_avg_stretch: f64,
    tz_max_stretch: f64,
    tz_p95_stretch: f64,
    tz_p99_stretch: f64,
    tz_violations: usize,
    tz_table_entries: usize,
    tz_bytes_estimated: usize,
    bytes_per_node: f64,
    avg_bunch_size: f64,
    tz_build_ms: u128,
    pie_tz_success_rate: f64,
    pie_tz_avg_hops: f64,
    pie_tz_stretch: f64,
    pie_tz_max_stretch: f64,
    pie_tz_p95_stretch: f64,
    pie_tz_p99_stretch: f64,
    pie_tz_tz_pct: f64,
}

#[derive(Debug)]
struct PieTzResult {
    success_rate: f64,
    avg_hops: f64,
    stretch: f64,
    max_stretch: f64,
    p95_stretch: f64,
    p99_stretch: f64,
    tz_pct: f64,
}

#[derive(Debug)]
struct StretchStats {
    avg: f64,
    max: f64,
    p95: f64,
    p99: f64,
    violations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetricSummary {
    mean: f64,
    ci95: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SensitivityRun {
    seed: u64,
    results: Vec<SensitivityResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SensitivitySummary {
    landmark_target: usize,
    landmark_count: MetricSummary,
    tz_avg_stretch: MetricSummary,
    tz_p95_stretch: MetricSummary,
    tz_p99_stretch: MetricSummary,
    tz_max_stretch: MetricSummary,
    bytes_per_node: MetricSummary,
    pie_tz_success_rate: MetricSummary,
    pie_tz_avg_hops: MetricSummary,
    pie_tz_stretch: MetricSummary,
    pie_tz_p95_stretch: MetricSummary,
    pie_tz_p99_stretch: MetricSummary,
    pie_tz_max_stretch: MetricSummary,
    pie_tz_tz_pct: MetricSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SensitivityReport {
    nodes: usize,
    tests: usize,
    landmarks: Vec<usize>,
    seeds: Vec<u64>,
    runs: Vec<SensitivityRun>,
    summary: Vec<SensitivitySummary>,
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

fn try_gravity_only_limited(
    router: &GPRouter,
    src: &NodeId,
    dst: &NodeId,
    max: u32,
) -> (bool, u32, NodeId) {
    if src == dst {
        return (true, 0, src.clone());
    }

    let dest_coord = match router.get_node(dst) {
        Some(n) => n.coord.point,
        None => return (false, 0, src.clone()),
    };

    let mut current = src.clone();
    let mut hops = 0;
    let mut visited = HashSet::new();
    visited.insert(src.clone());

    for _ in 0..max {
        if &current == dst {
            return (true, hops, current);
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
                hops += 1;
            }
            None => break,
        }
    }

    (&current == dst, hops, current)
}

fn run_pie_tz_tests(
    router: &GPRouter,
    tz_table: &TZRoutingTable,
    nodes: &[NodeId],
    num_tests: usize,
    seed: u64,
) -> PieTzResult {
    let mut rng = StdRng::seed_from_u64(seed + 1000);
    let n = nodes.len();
    let max_gravity = n as u32;

    let mut successes = 0u32;
    let mut total_hops = 0u32;
    let mut total_optimal = 0u32;
    let mut tz_hops_total = 0u32;
    let mut all_hops = 0u32;
    let mut max_stretch = 0.0f64;
    let mut stretch_samples: Vec<f64> = Vec::with_capacity(num_tests);

    for _ in 0..num_tests {
        let src = rng.gen_range(0..n);
        let mut dst = rng.gen_range(0..n);
        while dst == src {
            dst = rng.gen_range(0..n);
        }

        let (success, g_hops, final_node) =
            try_gravity_only_limited(router, &nodes[src], &nodes[dst], max_gravity);

        let (ok, hops, tz_hops) = if success {
            (true, g_hops, 0)
        } else if let Some(path) = tz_table.compute_path(&final_node, &nodes[dst]) {
            let tz_hops = (path.len().saturating_sub(1)) as u32;
            (true, g_hops + tz_hops, tz_hops)
        } else {
            (false, 0, 0)
        };

        if ok {
            successes += 1;
            total_hops += hops;
            tz_hops_total += tz_hops;
            all_hops += hops;

            if let Some(opt) = bfs_distance(router, &nodes[src], &nodes[dst]) {
                total_optimal += opt;
                if opt > 0 {
                    let s = hops as f64 / opt as f64;
                    max_stretch = f64::max(max_stretch, s);
                    stretch_samples.push(s);
                }
            }
        }
    }

    let (p95, p99) = stretch_percentiles(&stretch_samples);

    PieTzResult {
        success_rate: successes as f64 / num_tests as f64,
        avg_hops: if successes > 0 {
            total_hops as f64 / successes as f64
        } else {
            0.0
        },
        stretch: if total_optimal > 0 {
            total_hops as f64 / total_optimal as f64
        } else {
            0.0
        },
        max_stretch,
        p95_stretch: p95,
        p99_stretch: p99,
        tz_pct: if all_hops > 0 {
            tz_hops_total as f64 / all_hops as f64 * 100.0
        } else {
            0.0
        },
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

fn tz_stretch_stats(
    router: &GPRouter,
    tz_table: &TZRoutingTable,
    nodes: &[NodeId],
    num_samples: usize,
) -> StretchStats {
    let max_pairs = nodes.len().saturating_mul(nodes.len());
    let sample_count = num_samples.min(max_pairs);
    let mut stretches: Vec<f64> = Vec::new();
    let mut max_stretch = 0.0;
    let mut violations = 0;

    for i in 0..sample_count {
        let src_idx = i % nodes.len();
        let dst_idx = (i / nodes.len()) % nodes.len();
        if src_idx == dst_idx {
            continue;
        }
        let source = &nodes[src_idx];
        let destination = &nodes[dst_idx];
        if let Some(path) = tz_table.compute_path(source, destination) {
            if path.len() < 2 {
                continue;
            }
            let tz_len = path.len() as u32 - 1;
            if let Some(opt) = bfs_distance(router, source, destination) {
                if opt > 0 {
                    let stretch = tz_len as f64 / opt as f64;
                    stretches.push(stretch);
                    if stretch > max_stretch {
                        max_stretch = stretch;
                    }
                    if stretch > 3.0 {
                        violations += 1;
                    }
                }
            }
        }
    }

    let avg = if stretches.is_empty() {
        0.0
    } else {
        stretches.iter().sum::<f64>() / stretches.len() as f64
    };
    let (p95, p99) = stretch_percentiles(&stretches);

    StretchStats {
        avg,
        max: max_stretch,
        p95,
        p99,
        violations,
    }
}

fn summarize_sensitivity(runs: &[SensitivityRun]) -> Vec<SensitivitySummary> {
    let mut by_landmark: HashMap<usize, Vec<&SensitivityResult>> = HashMap::new();
    for run in runs {
        for result in &run.results {
            by_landmark
                .entry(result.landmark_target)
                .or_default()
                .push(result);
        }
    }

    let mut keys: Vec<usize> = by_landmark.keys().copied().collect();
    keys.sort_unstable();

    let mut summary = Vec::new();
    for key in keys {
        let items = &by_landmark[&key];
        let values = |f: fn(&SensitivityResult) -> f64| {
            items.iter().map(|r| f(r)).collect::<Vec<f64>>()
        };

        summary.push(SensitivitySummary {
            landmark_target: key,
            landmark_count: metric_summary(&values(|r| r.landmark_count as f64)),
            tz_avg_stretch: metric_summary(&values(|r| r.tz_avg_stretch)),
            tz_p95_stretch: metric_summary(&values(|r| r.tz_p95_stretch)),
            tz_p99_stretch: metric_summary(&values(|r| r.tz_p99_stretch)),
            tz_max_stretch: metric_summary(&values(|r| r.tz_max_stretch)),
            bytes_per_node: metric_summary(&values(|r| r.bytes_per_node)),
            pie_tz_success_rate: metric_summary(&values(|r| r.pie_tz_success_rate)),
            pie_tz_avg_hops: metric_summary(&values(|r| r.pie_tz_avg_hops)),
            pie_tz_stretch: metric_summary(&values(|r| r.pie_tz_stretch)),
            pie_tz_p95_stretch: metric_summary(&values(|r| r.pie_tz_p95_stretch)),
            pie_tz_p99_stretch: metric_summary(&values(|r| r.pie_tz_p99_stretch)),
            pie_tz_max_stretch: metric_summary(&values(|r| r.pie_tz_max_stretch)),
            pie_tz_tz_pct: metric_summary(&values(|r| r.pie_tz_tz_pct)),
        });
    }

    summary
}

fn parse_landmark_list(input: &str) -> Vec<usize> {
    input
        .split(',')
        .filter_map(|s| s.trim().parse::<usize>().ok())
        .filter(|v| *v > 0)
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
    println!("TZ Landmark Sensitivity Benchmark");
    println!("=================================\n");

    let args: Vec<String> = std::env::args().collect();

    let mut num_nodes = 2000usize;
    let mut num_tests = 500usize;
    let mut seed = 42u64;
    let mut seeds_override: Option<Vec<u64>> = None;
    let mut output_file =
        "paper_data/landmark_sensitivity/landmark_sensitivity.json".to_string();
    let mut landmarks = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];

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
            "--landmarks" | "-l" => {
                if i + 1 < args.len() {
                    let parsed = parse_landmark_list(&args[i + 1]);
                    if !parsed.is_empty() {
                        landmarks = parsed;
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
                println!("Usage: tz_landmark_sensitivity [OPTIONS]");
                println!();
                println!("Options:");
                println!("  -n, --nodes NUM         Number of nodes (default: 2000)");
                println!("  -t, --tests NUM         Number of routing tests (default: 500)");
                println!("  -l, --landmarks LIST    Comma-separated landmark counts");
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
    println!("  Landmarks:   {:?}", landmarks);
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

        let mut results = Vec::new();

        println!(
            "{:<10} {:<10} {:<10} {:<10} {:<10} {:<10}",
            "Target", "Actual", "TZ-Avg", "TZ-Max", "Mem/Node", "PIE+TZ"
        );
        println!("{}", "-".repeat(64));

        for &landmark_target in &landmarks {
            let start = Instant::now();
            let tz_table = TZRoutingTable::build(
                &adjacency,
                TZConfig {
                    num_landmarks: Some(landmark_target),
                    seed: *current_seed,
                },
            )
            .expect("TZ build failed");
            let tz_build_ms = start.elapsed().as_millis();

            let tz_stats = tz_stretch_stats(&router, &tz_table, &nodes, num_tests);
            let entries = tz_table.memory_usage();
            let landmark_count = tz_table.landmarks.len();
            let bytes_estimated = entries * 48 + landmark_count * 32;
            let bytes_per_node = bytes_estimated as f64 / num_nodes as f64;
            let avg_bunch = tz_table
                .node_info
                .values()
                .map(|info| info.bunch.len())
                .sum::<usize>() as f64
                / num_nodes as f64;

            let pie_tz = run_pie_tz_tests(&router, &tz_table, &nodes, num_tests, *current_seed);

            println!(
                "{:<10} {:<10} {:<10.2} {:<10.2} {:<10.1} {:<10.2}",
                landmark_target,
                landmark_count,
                tz_stats.avg,
                tz_stats.max,
                bytes_per_node,
                pie_tz.stretch
            );

            results.push(SensitivityResult {
                landmark_target,
                landmark_count,
                tz_avg_stretch: tz_stats.avg,
                tz_max_stretch: tz_stats.max,
                tz_p95_stretch: tz_stats.p95,
                tz_p99_stretch: tz_stats.p99,
                tz_violations: tz_stats.violations,
                tz_table_entries: entries,
                tz_bytes_estimated: bytes_estimated,
                bytes_per_node,
                avg_bunch_size: avg_bunch,
                tz_build_ms,
                pie_tz_success_rate: pie_tz.success_rate,
                pie_tz_avg_hops: pie_tz.avg_hops,
                pie_tz_stretch: pie_tz.stretch,
                pie_tz_max_stretch: pie_tz.max_stretch,
                pie_tz_p95_stretch: pie_tz.p95_stretch,
                pie_tz_p99_stretch: pie_tz.p99_stretch,
                pie_tz_tz_pct: pie_tz.tz_pct,
            });
        }

        runs.push(SensitivityRun {
            seed: *current_seed,
            results,
        });
        println!();
    }

    let summary = summarize_sensitivity(&runs);
    let report = SensitivityReport {
        nodes: num_nodes,
        tests: num_tests,
        landmarks,
        seeds,
        runs,
        summary,
    };

    ensure_output_dir(&output_file);
    let json = serde_json::to_string_pretty(&report).expect("Failed to serialize results");
    let mut file = File::create(&output_file).expect("Failed to create output file");
    file.write_all(json.as_bytes())
        .expect("Failed to write results");

    println!("\nResults saved to {}", output_file);
}
