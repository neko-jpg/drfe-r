//! Stretch Optimization Benchmark
//! 
//! Tests different embedding optimization strategies to minimize stretch ratio.
//! Compares: PIE only, PIE + Refine, PIE + Ricci Flow

use drfe_r::coordinates::{NodeId, RoutingCoordinate};
use drfe_r::greedy_embedding::GreedyEmbedding;
use drfe_r::routing::{GPRouter, RoutingNode};
use drfe_r::PoincareDiskPoint;
use rand::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Instant;

fn main() {
    println!("Stretch Optimization Benchmark");
    println!("==============================\n");

    let sizes = [100, 300, 500];
    let num_tests = 200;
    let seed = 42u64;

    println!("{:<8} {:<15} {:<10} {:<10} {:<10} {:<10}", 
             "Nodes", "Strategy", "Success%", "AvgHops", "Stretch", "Gravity%");
    println!("{}", "-".repeat(70));

    for &n in &sizes {
        // Generate BA network adjacency
        let (nodes, adjacency_idx, adjacency) = generate_ba_adjacency(n, 3, seed);

        // Strategy 1: PIE only
        let (router_pie, time_pie) = build_router_pie_only(&nodes, &adjacency_idx, &adjacency);
        let results_pie = run_tests(&router_pie, &nodes, n, num_tests, seed);
        println!("{:<8} {:<15} {:<10.2} {:<10.2} {:<10.2} {:<10.2}",
                 n, "PIE", results_pie.success_rate * 100.0,
                 results_pie.avg_hops, results_pie.stretch, results_pie.gravity_pct);

        // Strategy 2: PIE + Refine (100 iterations)
        let (router_refine, time_refine) = build_router_pie_refine(&nodes, &adjacency_idx, &adjacency, 100);
        let results_refine = run_tests(&router_refine, &nodes, n, num_tests, seed);
        println!("{:<8} {:<15} {:<10.2} {:<10.2} {:<10.2} {:<10.2}",
                 n, "PIE+Refine100", results_refine.success_rate * 100.0,
                 results_refine.avg_hops, results_refine.stretch, results_refine.gravity_pct);

        // Strategy 3: PIE + Refine (500 iterations)
        let (router_refine500, _) = build_router_pie_refine(&nodes, &adjacency_idx, &adjacency, 500);
        let results_refine500 = run_tests(&router_refine500, &nodes, n, num_tests, seed);
        println!("{:<8} {:<15} {:<10.2} {:<10.2} {:<10.2} {:<10.2}",
                 n, "PIE+Refine500", results_refine500.success_rate * 100.0,
                 results_refine500.avg_hops, results_refine500.stretch, results_refine500.gravity_pct);

        println!();
    }

    println!("\nConclusion:");
    println!("  - PIE provides tree-based greedy, but non-tree edges are ignored");
    println!("  - Refine tries to adjust coordinates, but may break greedy property");
    println!("  - Need a different approach that preserves greedy while optimizing stretch");
}

struct TestResults {
    success_rate: f64,
    avg_hops: f64,
    stretch: f64,
    gravity_pct: f64,
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

fn build_router_pie_only(
    nodes: &[NodeId], 
    adjacency_idx: &[Vec<usize>], 
    adjacency: &HashMap<NodeId, Vec<NodeId>>
) -> (GPRouter, u128) {
    let start = Instant::now();
    let embedder = GreedyEmbedding::new();
    let result = embedder.embed(adjacency).expect("Embedding failed");

    let mut tree_parent: HashMap<NodeId, Option<NodeId>> = HashMap::new();
    tree_parent.insert(result.root.clone(), None);
    for (parent, children) in &result.tree_children {
        for child in children {
            tree_parent.insert(child.clone(), Some(parent.clone()));
        }
    }

    let mut router = GPRouter::new();
    for (i, node_id) in nodes.iter().enumerate() {
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

fn build_router_pie_refine(
    nodes: &[NodeId],
    adjacency_idx: &[Vec<usize>],
    adjacency: &HashMap<NodeId, Vec<NodeId>>,
    refine_iterations: usize,
) -> (GPRouter, u128) {
    let start = Instant::now();
    let embedder = GreedyEmbedding::new();
    let result = embedder.embed(adjacency).expect("Embedding failed");

    // Refine coordinates
    let mut coords = result.coordinates.clone();
    GreedyEmbedding::refine_embedding(&mut coords, adjacency, refine_iterations, 0.1);

    let mut tree_parent: HashMap<NodeId, Option<NodeId>> = HashMap::new();
    tree_parent.insert(result.root.clone(), None);
    for (parent, children) in &result.tree_children {
        for child in children {
            tree_parent.insert(child.clone(), Some(parent.clone()));
        }
    }

    let mut router = GPRouter::new();
    for (i, node_id) in nodes.iter().enumerate() {
        let point = coords.get(node_id).copied().unwrap_or_else(PoincareDiskPoint::origin);
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

fn run_tests(router: &GPRouter, nodes: &[NodeId], n: usize, num_tests: usize, seed: u64) -> TestResults {
    let mut rng = StdRng::seed_from_u64(seed + 1000);
    let max_ttl = (n * 20) as u32;

    let mut successes = 0;
    let mut total_hops = 0u32;
    let mut total_optimal = 0u32;
    let mut gravity_hops = 0u32;
    let mut total_all_hops = 0u32;

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

                // BFS for optimal
                if let Some(opt) = bfs_shortest_path(router, source, dest) {
                    total_optimal += opt;
                }
            }
        }
    }

    let success_rate = successes as f64 / num_tests as f64;
    let avg_hops = if successes > 0 { total_hops as f64 / successes as f64 } else { 0.0 };
    let stretch = if total_optimal > 0 { total_hops as f64 / total_optimal as f64 } else { 0.0 };
    let gravity_pct = if total_all_hops > 0 { gravity_hops as f64 / total_all_hops as f64 * 100.0 } else { 0.0 };

    TestResults { success_rate, avg_hops, stretch, gravity_pct }
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
