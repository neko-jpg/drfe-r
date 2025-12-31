//! DRFE-R Simulator
//!
//! Large-scale simulation for testing routing performance and
//! verifying theoretical guarantees.

use drfe_r::coordinates::{NodeId, RoutingCoordinate};
use drfe_r::greedy_embedding::GreedyEmbedding;
use drfe_r::ricci::{GraphNode, RicciFlow, RicciGraph};
use drfe_r::routing::{GPRouter, RoutingNode};
use drfe_r::PoincareDiskPoint;
use rand::prelude::*;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// Configuration for the simulation
#[derive(Debug, Clone)]
pub struct SimConfig {
    pub num_nodes: usize,
    pub edge_probability: f64,
    pub num_routing_tests: usize,
    pub max_ttl: u32,
    pub seed: u64,
    pub ricci_optimize: bool,
    pub ricci_iterations: usize,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            num_nodes: 100,
            edge_probability: 0.1,
            num_routing_tests: 100,
            max_ttl: 50,
            seed: 42,
            ricci_optimize: false,
            ricci_iterations: 10,
        }
    }
}

/// Results of the simulation
#[derive(Debug, Clone)]
pub struct SimResults {
    pub total_tests: usize,
    pub successful_deliveries: usize,
    pub failed_deliveries: usize,
    pub total_hops: u32,
    pub gravity_hops: u32,
    pub pressure_hops: u32,
    pub avg_hops: f64,
    pub success_rate: f64,
    pub ttl_failures: usize,
    pub no_path_failures: usize,
    pub elapsed_ms: u128,
}

impl std::fmt::Display for SimResults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "=== Simulation Results ===")?;
        writeln!(f, "Total tests:          {}", self.total_tests)?;
        writeln!(f, "Successful:           {}", self.successful_deliveries)?;
        writeln!(f, "Failed:               {}", self.failed_deliveries)?;
        writeln!(f, "Success rate:         {:.2}%", self.success_rate * 100.0)?;
        writeln!(f, "Average hops:         {:.2}", self.avg_hops)?;
        writeln!(f, "Gravity hops:         {}", self.gravity_hops)?;
        writeln!(f, "Pressure hops:        {}", self.pressure_hops)?;
        writeln!(f, "TTL failures:         {}", self.ttl_failures)?;
        writeln!(f, "No path failures:     {}", self.no_path_failures)?;
        writeln!(f, "Elapsed time:         {} ms", self.elapsed_ms)?;
        Ok(())
    }
}

/// Generate a random network using Erdős–Rényi model
fn generate_random_network(config: &SimConfig) -> GPRouter {
    let mut rng = StdRng::seed_from_u64(config.seed);
    let mut router = GPRouter::new();

    // Generate nodes with random positions in the Poincaré disk
    let nodes: Vec<NodeId> = (0..config.num_nodes)
        .map(|i| {
            // Random position in Poincaré disk (uniform in Euclidean coordinates, biased toward center)
            let r = rng.gen::<f64>().sqrt() * 0.9; // sqrt for uniform area distribution
            let theta = rng.gen::<f64>() * 2.0 * std::f64::consts::PI;
            let point = PoincareDiskPoint::from_polar(r, theta).unwrap();
            let coord = RoutingCoordinate::new(point, 0);
            let id = NodeId::new(format!("node_{}", i));
            router.add_node(RoutingNode::new(id.clone(), coord));
            id
        })
        .collect();

    // Generate edges with given probability
    for i in 0..config.num_nodes {
        for j in (i + 1)..config.num_nodes {
            if rng.gen::<f64>() < config.edge_probability {
                router.add_edge(&nodes[i], &nodes[j]);
            }
        }
    }

    // Ensure connectivity by adding edges to isolated nodes
    let mut connected: HashSet<usize> = HashSet::new();
    connected.insert(0);
    
    for i in 1..config.num_nodes {
        let current_node = router.get_node(&nodes[i]);
        if current_node.map(|n| n.neighbors.is_empty()).unwrap_or(true) {
            // Connect to a random already-connected node
            let connected_idx = *connected.iter().nth(rng.gen_range(0..connected.len())).unwrap();
            router.add_edge(&nodes[i], &nodes[connected_idx]);
        }
        connected.insert(i);
    }

    router
}

/// Generate a Barabási-Albert scale-free network with PIE greedy embedding
/// Uses Polar Increasing-angle Embedding for guaranteed greedy routing success
fn generate_barabasi_albert_network(config: &SimConfig, m: usize) -> GPRouter {
    let mut rng = StdRng::seed_from_u64(config.seed);
    let mut router = GPRouter::new();
    let mut degrees: Vec<usize> = Vec::new();
    let mut nodes: Vec<NodeId> = Vec::new();
    let mut adjacency_idx: Vec<Vec<usize>> = Vec::new();

    // Create nodes first (without coordinates)
    for i in 0..config.num_nodes {
        let id = NodeId::new(format!("node_{}", i));
        nodes.push(id);
        degrees.push(0);
        adjacency_idx.push(Vec::new());
    }

    // Build initial complete graph of m nodes
    for i in 0..m.min(config.num_nodes) {
        for j in (i + 1)..m.min(config.num_nodes) {
            adjacency_idx[i].push(j);
            adjacency_idx[j].push(i);
            degrees[i] += 1;
            degrees[j] += 1;
        }
    }

    // Add remaining nodes with preferential attachment
    for i in m..config.num_nodes {
        let total_degree: usize = degrees.iter().take(i).sum();
        if total_degree == 0 {
            // Connect to node 0 if no degrees yet
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
            let r_val = rng.gen::<f64>() * total_degree as f64;
            let mut cumsum = 0.0;
            for j in 0..i {
                cumsum += degrees[j] as f64;
                if cumsum >= r_val && !connected.contains(&j) {
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

    // Convert index-based adjacency to NodeId-based adjacency for GreedyEmbedding
    let mut adjacency: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for i in 0..config.num_nodes {
        let neighbors: Vec<NodeId> = adjacency_idx[i]
            .iter()
            .map(|&j| nodes[j].clone())
            .collect();
        adjacency.insert(nodes[i].clone(), neighbors);
    }

    // Use PIE (Polar Increasing-angle Embedding) for greedy routing guarantee
    let embedder = GreedyEmbedding::new();
    let embedding_result = embedder.embed(&adjacency).expect("Embedding should succeed");

    // Note: Coordinate refinement disabled as it degrades performance
    // The PIE embedding alone provides the best structure for greedy routing

    // Create router nodes with embedded coordinates
    for i in 0..config.num_nodes {
        let node_id = &nodes[i];
        let point = embedding_result
            .coordinates
            .get(node_id)
            .copied()
            .unwrap_or_else(PoincareDiskPoint::origin);
        let coord = RoutingCoordinate::new(point, 0);
        router.add_node(RoutingNode::new(node_id.clone(), coord));
    }

    // Add edges
    for i in 0..config.num_nodes {
        for &j in &adjacency_idx[i] {
            if i < j {
                router.add_edge(&nodes[i], &nodes[j]);
            }
        }
    }

    router
}

/// Run routing simulation
fn run_simulation(router: &GPRouter, config: &SimConfig) -> SimResults {
    let mut rng = StdRng::seed_from_u64(config.seed + 1000);
    let node_ids: Vec<NodeId> = (0..config.num_nodes)
        .map(|i| NodeId::new(format!("node_{}", i)))
        .collect();

    let start = Instant::now();
    let mut successful = 0;
    let mut failed = 0;
    let mut total_hops = 0u32;
    let mut gravity_hops = 0u32;
    let mut pressure_hops = 0u32;
    let mut ttl_failures = 0usize;
    let mut no_path_failures = 0usize;

    for _ in 0..config.num_routing_tests {
        // Pick random source and destination
        let src_idx = rng.gen_range(0..config.num_nodes);
        let mut dst_idx = rng.gen_range(0..config.num_nodes);
        while dst_idx == src_idx {
            dst_idx = rng.gen_range(0..config.num_nodes);
        }

        let source = &node_ids[src_idx];
        let dest = &node_ids[dst_idx];

        // Get destination coordinate for routing target
        if let Some(dest_node) = router.get_node(dest) {
            let result = router.simulate_delivery(
                source,
                dest,
                dest_node.coord.point,
                config.max_ttl,
            );

            if result.success {
                successful += 1;
                total_hops += result.hops;
                gravity_hops += result.gravity_hops;
                pressure_hops += result.pressure_hops;
            } else {
                failed += 1;
                if let Some(ref reason) = result.failure_reason {
                    if reason.contains("TTL") {
                        ttl_failures += 1;
                    } else {
                        no_path_failures += 1;
                    }
                }
            }
        } else {
            failed += 1;
            no_path_failures += 1;
        }
    }

    let elapsed = start.elapsed().as_millis();
    let avg_hops = if successful > 0 {
        total_hops as f64 / successful as f64
    } else {
        0.0
    };

    SimResults {
        total_tests: config.num_routing_tests,
        successful_deliveries: successful,
        failed_deliveries: failed,
        total_hops,
        gravity_hops,
        pressure_hops,
        avg_hops,
        success_rate: successful as f64 / config.num_routing_tests as f64,
        ttl_failures,
        no_path_failures,
        elapsed_ms: elapsed,
    }
}

fn main() {
    println!("DRFE-R Simulator");
    println!("================\n");

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    
    let mut config = SimConfig::default();
    let mut topology = "barabasi-albert";

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--nodes" | "-n" => {
                if i + 1 < args.len() {
                    config.num_nodes = args[i + 1].parse().unwrap_or(100);
                    i += 1;
                }
            }
            "--topology" | "-t" => {
                if i + 1 < args.len() {
                    topology = if args[i + 1] == "random" { "random" } else { "barabasi-albert" };
                    i += 1;
                }
            }
            "--tests" => {
                if i + 1 < args.len() {
                    config.num_routing_tests = args[i + 1].parse().unwrap_or(100);
                    i += 1;
                }
            }
            "--ttl" => {
                if i + 1 < args.len() {
                    config.max_ttl = args[i + 1].parse().unwrap_or(50);
                    i += 1;
                }
            }
            "--seed" => {
                if i + 1 < args.len() {
                    config.seed = args[i + 1].parse().unwrap_or(42);
                    i += 1;
                }
            }
            "--optimize" | "-o" => {
                config.ricci_optimize = true;
            }
            "--ricci-iter" => {
                if i + 1 < args.len() {
                    config.ricci_iterations = args[i + 1].parse().unwrap_or(10);
                    i += 1;
                }
            }
            "--help" | "-h" => {
                println!("Usage: simulator [OPTIONS]");
                println!();
                println!("Options:");
                println!("  -n, --nodes NUM       Number of nodes (default: 100)");
                println!("  -t, --topology TYPE   Topology type: random, barabasi-albert (default: barabasi-albert)");
                println!("      --tests NUM       Number of routing tests (default: 100)");
                println!("      --ttl NUM         Max TTL for routing (default: 50)");
                println!("      --seed NUM        Random seed (default: 42)");
                println!("  -o, --optimize        Enable Ricci Flow coordinate optimization");
                println!("      --ricci-iter NUM  Ricci Flow iterations (default: 10)");
                println!("  -h, --help            Show this help");
                return;
            }
            _ => {}
        }
        i += 1;
    }

    println!("Configuration:");
    println!("  Nodes:     {}", config.num_nodes);
    println!("  Topology:  {}", topology);
    println!("  Tests:     {}", config.num_routing_tests);
    println!("  Max TTL:   {}", config.max_ttl);
    println!("  Seed:      {}", config.seed);
    println!("  Optimize:  {}", if config.ricci_optimize { "yes" } else { "no" });
    println!();

    // Generate network
    print!("Generating network... ");
    let gen_start = Instant::now();
    let mut router = match topology {
        "random" => generate_random_network(&config),
        _ => generate_barabasi_albert_network(&config, 3),
    };
    println!("done ({} ms)", gen_start.elapsed().as_millis());
    println!("  Nodes: {}", router.node_count());
    println!("  Edges: {}", router.edge_count());
    println!();

    // Apply Ricci Flow optimization if enabled
    if config.ricci_optimize {
        print!("Running Ricci Flow optimization ({} iterations)... ", config.ricci_iterations);
        let opt_start = Instant::now();
        
        // Build RicciGraph from router
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
        
        // Add edges to ricci_graph
        for edge in router.get_edges() {
            ricci_graph.add_edge(&edge.0, &edge.1);
        }
        
        // Run optimization
        let flow = RicciFlow::new(0.1);
        let stress = flow.run_optimization(&mut ricci_graph, config.ricci_iterations, 50);
        
        // Update router coordinates from optimized ricci_graph
        for (id, node) in &ricci_graph.nodes {
            if let Some(router_node) = router.get_node_mut(id) {
                router_node.coord.point = node.coord.point;
            }
        }
        
        println!("done ({} ms, residual stress: {:.4})", opt_start.elapsed().as_millis(), stress);
        println!();
    }

    // Run simulation
    print!("Running routing simulation... ");
    let results = run_simulation(&router, &config);
    println!("done\n");

    println!("{}", results);

    // Verdict
    if results.success_rate >= 1.0 {
        println!("✓ VERIFIED: 100% routing success rate achieved");
        println!("  Theory prediction (Theorem 1): Connected graph + TTL → guaranteed delivery");
    } else if results.success_rate >= 0.95 {
        println!("○ MOSTLY VERIFIED: {:.1}% success rate", results.success_rate * 100.0);
        println!("  Minor failures may be due to disconnected components or TTL exhaustion");
    } else {
        println!("✗ VERIFICATION ISSUE: {:.1}% success rate", results.success_rate * 100.0);
        println!("  Check network connectivity or increase TTL");
    }
}
