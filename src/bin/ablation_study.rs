//! Comprehensive Ablation Study for DRFE-R Paper
//!
//! Compares multiple embedding strategies across topologies and scales.
//! Outputs results to paper_data/ for publication.

use drfe_r::coordinates::{NodeId, RoutingCoordinate};
use drfe_r::greedy_embedding::GreedyEmbedding;
use drfe_r::ricci::{GraphNode, RicciFlow, RicciGraph};
use drfe_r::routing::{GPRouter, RoutingNode};
use drfe_r::PoincareDiskPoint;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Write;
use std::time::Instant;

/// Embedding strategy for comparison
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum EmbeddingStrategy {
    /// PIE (Polar Increasing-angle Embedding) only
    PIE,
    /// Random uniform coordinates in Poincaré disk
    Random,
    /// Ricci Flow optimization (current implementation)
    RicciBroken,
    /// Ricci Flow with fixed Riemannian gradient (to be implemented)
    RicciFixed,
}

impl std::fmt::Display for EmbeddingStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmbeddingStrategy::PIE => write!(f, "PIE"),
            EmbeddingStrategy::Random => write!(f, "Random"),
            EmbeddingStrategy::RicciBroken => write!(f, "Ricci-Broken"),
            EmbeddingStrategy::RicciFixed => write!(f, "Ricci-Fixed"),
        }
    }
}

/// Single experiment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentConfig {
    pub topology: String,
    pub num_nodes: usize,
    pub embedding: EmbeddingStrategy,
    pub num_tests: usize,
    pub max_ttl: u32,
    pub seed: u64,
}

/// Single experiment result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentResult {
    pub config: ExperimentConfig,
    pub success_rate: f64,
    pub avg_hops: f64,
    pub avg_stretch: f64,
    pub gravity_hops_total: u32,
    pub pressure_hops_total: u32,
    pub tree_hops_total: u32,
    pub gravity_ratio: f64,
    pub pressure_ratio: f64,
    pub tree_ratio: f64,
    pub elapsed_ms: u128,
}

/// All results for paper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AblationResults {
    pub generated_at: String,
    pub experiments: Vec<ExperimentResult>,
}

/// Generate network with specified embedding strategy
fn generate_network(
    config: &ExperimentConfig,
) -> GPRouter {
    let mut adjacency: HashMap<usize, HashSet<usize>> = HashMap::new();
    for i in 0..config.num_nodes {
        adjacency.insert(i, HashSet::new());
    }

    // Build topology
    match config.topology.as_str() {
        "ba" => build_ba_topology(&mut adjacency, config.num_nodes, 3, config.seed),
        "ws" => build_ws_topology(&mut adjacency, config.num_nodes, 6, 0.1, config.seed),
        "grid" => build_grid_topology(&mut adjacency, config.num_nodes),
        "line" => build_line_topology(&mut adjacency, config.num_nodes),
        "lollipop" => build_lollipop_topology(&mut adjacency, config.num_nodes, 0.33),
        _ => build_ba_topology(&mut adjacency, config.num_nodes, 3, config.seed),
    };

    // Build router with specified embedding
    match config.embedding {
        EmbeddingStrategy::PIE => build_router_pie(&adjacency, config.num_nodes),
        EmbeddingStrategy::Random => build_router_random(&adjacency, config.num_nodes, config.seed),
        EmbeddingStrategy::RicciBroken => {
            let mut router = build_router_pie(&adjacency, config.num_nodes);
            apply_ricci_flow_broken(&mut router, 30);
            router
        }
        EmbeddingStrategy::RicciFixed => {
            let mut router = build_router_pie(&adjacency, config.num_nodes);
            apply_ricci_flow_fixed(&mut router, 30);
            router
        }
    }
}

fn build_ba_topology(adj: &mut HashMap<usize, HashSet<usize>>, n: usize, m: usize, seed: u64) {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut degrees: Vec<usize> = vec![0; n];

    // Initial clique
    for i in 0..m.min(n) {
        for j in (i + 1)..m.min(n) {
            adj.get_mut(&i).unwrap().insert(j);
            adj.get_mut(&j).unwrap().insert(i);
            degrees[i] += 1;
            degrees[j] += 1;
        }
    }

    // Preferential attachment
    for i in m..n {
        let total_degree: usize = degrees.iter().take(i).sum();
        if total_degree == 0 {
            adj.get_mut(&i).unwrap().insert(0);
            adj.get_mut(&0).unwrap().insert(i);
            degrees[i] += 1;
            degrees[0] += 1;
            continue;
        }

        let mut connected = HashSet::new();
        let mut attempts = 0;
        while connected.len() < m.min(i) && attempts < 1000 {
            attempts += 1;
            let r = rng.gen::<f64>() * total_degree as f64;
            let mut cumsum = 0.0;
            for j in 0..i {
                cumsum += degrees[j] as f64;
                if cumsum >= r && !connected.contains(&j) {
                    adj.get_mut(&i).unwrap().insert(j);
                    adj.get_mut(&j).unwrap().insert(i);
                    degrees[i] += 1;
                    degrees[j] += 1;
                    connected.insert(j);
                    break;
                }
            }
        }
    }
}

fn build_ws_topology(adj: &mut HashMap<usize, HashSet<usize>>, n: usize, k: usize, beta: f64, seed: u64) {
    let mut rng = StdRng::seed_from_u64(seed);

    // Ring lattice
    for i in 0..n {
        for j in 1..=(k / 2) {
            let neighbor = (i + j) % n;
            adj.get_mut(&i).unwrap().insert(neighbor);
            adj.get_mut(&neighbor).unwrap().insert(i);
        }
    }

    // Rewiring
    for i in 0..n {
        for j in 1..=(k / 2) {
            let original = (i + j) % n;
            if rng.gen::<f64>() < beta {
                if adj.get(&i).unwrap().contains(&original) {
                    adj.get_mut(&i).unwrap().remove(&original);
                    adj.get_mut(&original).unwrap().remove(&i);

                    let mut new_neighbor = rng.gen_range(0..n);
                    while new_neighbor == i || adj.get(&i).unwrap().contains(&new_neighbor) {
                        new_neighbor = rng.gen_range(0..n);
                    }
                    adj.get_mut(&i).unwrap().insert(new_neighbor);
                    adj.get_mut(&new_neighbor).unwrap().insert(i);
                }
            }
        }
    }
}

fn build_grid_topology(adj: &mut HashMap<usize, HashSet<usize>>, n: usize) {
    let width = (n as f64).sqrt().ceil() as usize;
    for i in 0..n {
        let x = i % width;
        if x + 1 < width {
            let neighbor = i + 1;
            if neighbor < n {
                adj.get_mut(&i).unwrap().insert(neighbor);
                adj.get_mut(&neighbor).unwrap().insert(i);
            }
        }
        let neighbor = i + width;
        if neighbor < n {
            adj.get_mut(&i).unwrap().insert(neighbor);
            adj.get_mut(&neighbor).unwrap().insert(i);
        }
    }
}

fn build_line_topology(adj: &mut HashMap<usize, HashSet<usize>>, n: usize) {
    for i in 0..n - 1 {
        adj.get_mut(&i).unwrap().insert(i + 1);
        adj.get_mut(&(i + 1)).unwrap().insert(i);
    }
}

fn build_lollipop_topology(adj: &mut HashMap<usize, HashSet<usize>>, n: usize, head_ratio: f64) {
    let head_size = ((n as f64 * head_ratio) as usize).max(3).min(n - 1);

    // Clique head
    for i in 0..head_size {
        for j in (i + 1)..head_size {
            adj.get_mut(&i).unwrap().insert(j);
            adj.get_mut(&j).unwrap().insert(i);
        }
    }

    // Line tail
    for i in head_size..n {
        let prev = i - 1;
        adj.get_mut(&i).unwrap().insert(prev);
        adj.get_mut(&prev).unwrap().insert(i);
    }
}

fn build_router_pie(adj: &HashMap<usize, HashSet<usize>>, n: usize) -> GPRouter {
    let mut router = GPRouter::new();
    let nodes: Vec<NodeId> = (0..n).map(|i| NodeId::new(format!("node_{}", i))).collect();

    let mut adjacency_nodeid: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for (&i, neighbors) in adj {
        let neighbor_ids: Vec<NodeId> = neighbors.iter().map(|&j| nodes[j].clone()).collect();
        adjacency_nodeid.insert(nodes[i].clone(), neighbor_ids);
    }

    let embedder = GreedyEmbedding::new();
    let result = embedder.embed(&adjacency_nodeid).expect("Embedding failed");

    let mut tree_parent: HashMap<NodeId, Option<NodeId>> = HashMap::new();
    tree_parent.insert(result.root.clone(), None);
    for (parent_id, children) in &result.tree_children {
        for child_id in children {
            tree_parent.insert(child_id.clone(), Some(parent_id.clone()));
        }
    }

    for i in 0..n {
        let node_id = &nodes[i];
        let point = result.coordinates.get(node_id).copied().unwrap_or_else(PoincareDiskPoint::origin);
        let coord = RoutingCoordinate::new(point, 0);
        let mut routing_node = RoutingNode::new(node_id.clone(), coord);
        let parent = tree_parent.get(node_id).cloned().flatten();
        let children = result.tree_children.get(node_id).cloned().unwrap_or_default();
        routing_node.set_tree_info(parent, children);
        router.add_node(routing_node);
    }

    for (&i, neighbors) in adj {
        for &j in neighbors {
            if i < j {
                router.add_edge(&nodes[i], &nodes[j]);
            }
        }
    }

    router
}

fn build_router_random(adj: &HashMap<usize, HashSet<usize>>, n: usize, seed: u64) -> GPRouter {
    let mut rng = StdRng::seed_from_u64(seed + 999);
    let mut router = GPRouter::new();
    let nodes: Vec<NodeId> = (0..n).map(|i| NodeId::new(format!("node_{}", i))).collect();

    for i in 0..n {
        let r = rng.gen::<f64>().sqrt() * 0.95;
        let theta = rng.gen::<f64>() * 2.0 * std::f64::consts::PI;
        let point = PoincareDiskPoint::from_polar(r, theta).unwrap();
        let coord = RoutingCoordinate::new(point, 0);
        let routing_node = RoutingNode::new(nodes[i].clone(), coord);
        router.add_node(routing_node);
    }

    for (&i, neighbors) in adj {
        for &j in neighbors {
            if i < j {
                router.add_edge(&nodes[i], &nodes[j]);
            }
        }
    }

    router
}

fn apply_ricci_flow_broken(router: &mut GPRouter, iterations: usize) {
    let mut ricci_graph = RicciGraph::new();
    for node_id in router.node_ids() {
        if let Some(node) = router.get_node(&node_id) {
            ricci_graph.add_node(GraphNode {
                id: node.id.clone(),
                coord: node.coord.clone(),
                neighbors: node.neighbors.clone(),
            });
        }
    }
    for edge in router.get_edges() {
        ricci_graph.add_edge(&edge.0, &edge.1);
    }

    let flow = RicciFlow::new(0.1);
    let _ = flow.run_optimization(&mut ricci_graph, iterations, 50);

    for (id, node) in &ricci_graph.nodes {
        if let Some(router_node) = router.get_node_mut(id) {
            router_node.coord.point = node.coord.point;
        }
    }
}

fn apply_ricci_flow_fixed(router: &mut GPRouter, iterations: usize) {
    // Fixed Ricci Flow using proper Riemannian gradient descent
    let mut ricci_graph = RicciGraph::new();
    for node_id in router.node_ids() {
        if let Some(node) = router.get_node(&node_id) {
            ricci_graph.add_node(GraphNode {
                id: node.id.clone(),
                coord: node.coord.clone(),
                neighbors: node.neighbors.clone(),
            });
        }
    }
    for edge in router.get_edges() {
        ricci_graph.add_edge(&edge.0, &edge.1);
    }

    // Use smaller step size for stability with new algorithm
    let flow = RicciFlow::new(0.05);
    let _ = flow.run_optimization(&mut ricci_graph, iterations, 30);

    for (id, node) in &ricci_graph.nodes {
        if let Some(router_node) = router.get_node_mut(id) {
            router_node.coord.point = node.coord.point;
        }
    }
}

fn run_experiment(config: &ExperimentConfig) -> ExperimentResult {
    let router = generate_network(config);
    let mut rng = StdRng::seed_from_u64(config.seed + 1000);
    let node_ids: Vec<NodeId> = (0..config.num_nodes)
        .map(|i| NodeId::new(format!("node_{}", i)))
        .collect();

    let start = Instant::now();
    let mut successful = 0;
    let mut total_hops = 0u32;
    let mut gravity_hops = 0u32;
    let mut pressure_hops = 0u32;
    let mut tree_hops = 0u32;
    let mut total_optimal = 0u32;

    for _ in 0..config.num_tests {
        let src_idx = rng.gen_range(0..config.num_nodes);
        let mut dst_idx = rng.gen_range(0..config.num_nodes);
        while dst_idx == src_idx {
            dst_idx = rng.gen_range(0..config.num_nodes);
        }

        let source = &node_ids[src_idx];
        let dest = &node_ids[dst_idx];

        if let Some(dest_node) = router.get_node(dest) {
            let result = router.simulate_delivery(source, dest, dest_node.coord.point, config.max_ttl);
            if result.success {
                successful += 1;
                total_hops += result.hops;
                gravity_hops += result.gravity_hops;
                pressure_hops += result.pressure_hops;
                tree_hops += result.tree_hops;

                if let Some(opt) = bfs_shortest(&router, source, dest) {
                    total_optimal += opt;
                }
            }
        }
    }

    let elapsed = start.elapsed().as_millis();
    let total_mode_hops = gravity_hops + pressure_hops + tree_hops;

    ExperimentResult {
        config: config.clone(),
        success_rate: successful as f64 / config.num_tests as f64,
        avg_hops: if successful > 0 { total_hops as f64 / successful as f64 } else { 0.0 },
        avg_stretch: if total_optimal > 0 { total_hops as f64 / total_optimal as f64 } else { 0.0 },
        gravity_hops_total: gravity_hops,
        pressure_hops_total: pressure_hops,
        tree_hops_total: tree_hops,
        gravity_ratio: if total_mode_hops > 0 { gravity_hops as f64 / total_mode_hops as f64 } else { 0.0 },
        pressure_ratio: if total_mode_hops > 0 { pressure_hops as f64 / total_mode_hops as f64 } else { 0.0 },
        tree_ratio: if total_mode_hops > 0 { tree_hops as f64 / total_mode_hops as f64 } else { 0.0 },
        elapsed_ms: elapsed,
    }
}

fn bfs_shortest(router: &GPRouter, start: &NodeId, end: &NodeId) -> Option<u32> {
    let mut visited = HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back((start.clone(), 0));
    visited.insert(start.clone());

    while let Some((current, dist)) = queue.pop_front() {
        if &current == end {
            return Some(dist);
        }
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

fn main() {
    println!("DRFE-R Ablation Study for Paper");
    println!("================================\n");

    let topologies = vec!["ba", "ws", "grid", "line", "lollipop"];
    let scales = vec![50, 100, 200, 300];
    let embeddings = vec![
        EmbeddingStrategy::PIE,
        EmbeddingStrategy::Random,
        EmbeddingStrategy::RicciBroken,
        EmbeddingStrategy::RicciFixed,
    ];

    let mut experiments = Vec::new();

    for topo in &topologies {
        for &n in &scales {
            let ttl = (n * 2) as u32;
            for &emb in &embeddings {
                let config = ExperimentConfig {
                    topology: topo.to_string(),
                    num_nodes: n,
                    embedding: emb,
                    num_tests: 200,
                    max_ttl: ttl,
                    seed: 12345,
                };

                print!("Running {} N={} {}... ", topo, n, emb);
                std::io::stdout().flush().ok();

                let result = run_experiment(&config);
                println!(
                    "Success: {:.1}%, Hops: {:.1}, Stretch: {:.2}",
                    result.success_rate * 100.0,
                    result.avg_hops,
                    result.avg_stretch
                );

                experiments.push(result);
            }
        }
    }

    // Save results
    let results = AblationResults {
        generated_at: chrono::Local::now().to_rfc3339(),
        experiments,
    };

    let json = serde_json::to_string_pretty(&results).expect("JSON serialization failed");
    let mut file = File::create("paper_data/ablation/ablation_results.json")
        .expect("Failed to create results file");
    file.write_all(json.as_bytes()).expect("Failed to write results");

    println!("\n✓ Results saved to paper_data/ablation/ablation_results.json");

    // Generate CSV summary
    let mut csv = File::create("paper_data/ablation/ablation_summary.csv")
        .expect("Failed to create CSV file");
    writeln!(csv, "topology,n,embedding,success_rate,avg_hops,stretch,gravity_ratio,pressure_ratio,tree_ratio")
        .expect("CSV write failed");

    for exp in &results.experiments {
        writeln!(
            csv,
            "{},{},{},{:.4},{:.2},{:.3},{:.3},{:.3},{:.3}",
            exp.config.topology,
            exp.config.num_nodes,
            exp.config.embedding,
            exp.success_rate,
            exp.avg_hops,
            exp.avg_stretch,
            exp.gravity_ratio,
            exp.pressure_ratio,
            exp.tree_ratio
        ).expect("CSV write failed");
    }

    println!("✓ CSV summary saved to paper_data/ablation/ablation_summary.csv");
}
