//! Baseline Comparison Experiments
//!
//! Compare DRFE-R routing performance with Chord and Kademlia DHTs.
//! 
//! This experiment measures:
//! - Success rate
//! - Average hop count
//! - Routing latency
//! - Scalability (performance vs network size)

use drfe_r::baselines::{ChordDHT, DHTRouter, KademliaDHT};
use drfe_r::coordinates::{NodeId, RoutingCoordinate};
use drfe_r::greedy_embedding::GreedyEmbedding;
use drfe_r::routing::{GPRouter, RoutingNode};
use drfe_r::PoincareDiskPoint;
use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ComparisonResult {
    protocol: String,
    network_size: usize,
    topology: String,
    success_rate: f64,
    avg_hops: f64,
    avg_latency_us: f64,
    total_tests: usize,
    successful_tests: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExperimentSummary {
    timestamp: String,
    results: Vec<ComparisonResult>,
}

/// Generate a random graph topology
fn generate_topology(n: usize, topology_type: &str) -> Vec<(usize, usize)> {
    let mut rng = rand::thread_rng();
    let mut edges = Vec::new();

    match topology_type {
        "ba" => {
            // Barabási-Albert (scale-free)
            let m = 3; // edges per new node
            
            // Start with complete graph of m nodes
            for i in 0..m {
                for j in (i + 1)..m {
                    edges.push((i, j));
                }
            }
            
            // Add remaining nodes with preferential attachment
            for i in m..n {
                let mut degrees: HashMap<usize, usize> = HashMap::new();
                for &(a, b) in &edges {
                    *degrees.entry(a).or_insert(0) += 1;
                    *degrees.entry(b).or_insert(0) += 1;
                }
                
                let total_degree: usize = degrees.values().sum();
                let mut targets = Vec::new();
                
                while targets.len() < m && targets.len() < i {
                    let r: f64 = rng.gen();
                    let mut cumulative = 0.0;
                    
                    for node in 0..i {
                        if targets.contains(&node) {
                            continue;
                        }
                        let prob = *degrees.get(&node).unwrap_or(&1) as f64 / total_degree as f64;
                        cumulative += prob;
                        
                        if r <= cumulative {
                            targets.push(node);
                            break;
                        }
                    }
                }
                
                for &target in &targets {
                    edges.push((i, target));
                }
            }
        }
        "random" => {
            // Erdős-Rényi random graph
            let p = 4.0 / n as f64; // Average degree ~4
            for i in 0..n {
                for j in (i + 1)..n {
                    if rng.gen::<f64>() < p {
                        edges.push((i, j));
                    }
                }
            }
        }
        "grid" => {
            // 2D grid
            let side = (n as f64).sqrt().ceil() as usize;
            for i in 0..n {
                let row = i / side;
                let col = i % side;
                
                // Right neighbor
                if col + 1 < side && i + 1 < n {
                    edges.push((i, i + 1));
                }
                
                // Bottom neighbor
                if row + 1 < side && i + side < n {
                    edges.push((i, i + side));
                }
            }
        }
        _ => panic!("Unknown topology type: {}", topology_type),
    }

    edges
}

/// Build DRFE-R router from topology
fn build_drfer_router(n: usize, edges: &[(usize, usize)]) -> GPRouter {
    // Create adjacency list
    let mut adjacency: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    
    for i in 0..n {
        adjacency.insert(NodeId::new(&format!("node{}", i)), Vec::new());
    }
    
    for &(i, j) in edges {
        let id_i = NodeId::new(&format!("node{}", i));
        let id_j = NodeId::new(&format!("node{}", j));
        
        adjacency.get_mut(&id_i).unwrap().push(id_j.clone());
        adjacency.get_mut(&id_j).unwrap().push(id_i.clone());
    }
    
    // Create greedy embedding
    let embedding = GreedyEmbedding::new();
    let result = embedding.embed(&adjacency).expect("Failed to create embedding");
    
    // Build router
    let mut router = GPRouter::new();
    
    for i in 0..n {
        let node_id = NodeId::new(&format!("node{}", i));
        let coord_point = result.coordinates.get(&node_id).cloned().unwrap_or_else(|| {
            PoincareDiskPoint::origin()
        });
        let coord = RoutingCoordinate::new(coord_point, 0);
        
        let mut node = RoutingNode::new(node_id.clone(), coord);
        
        // Set tree structure from embedding result
        let children = result.tree_children.get(&node_id).cloned().unwrap_or_default();
        node.tree_children = children;
        
        // Find parent by checking which node has this node as a child
        for (parent_id, parent_children) in &result.tree_children {
            if parent_children.contains(&node_id) {
                node.tree_parent = Some(parent_id.clone());
                break;
            }
        }
        
        router.add_node(node);
    }
    
    // Add edges
    for &(i, j) in edges {
        let id_i = NodeId::new(&format!("node{}", i));
        let id_j = NodeId::new(&format!("node{}", j));
        router.add_edge(&id_i, &id_j);
    }
    
    router
}

/// Build Chord DHT from topology
fn build_chord_dht(n: usize, _edges: &[(usize, usize)]) -> ChordDHT {
    let m = (n as f64).log2().ceil() as usize + 1;
    let mut chord = ChordDHT::new(m);
    
    for i in 0..n {
        chord.add_node(NodeId::new(&format!("node{}", i)));
    }
    
    chord.build_finger_tables();
    chord
}

/// Build Kademlia DHT from topology
fn build_kademlia_dht(n: usize, _edges: &[(usize, usize)]) -> KademliaDHT {
    let k = 20.min(n / 2);
    let mut kad = KademliaDHT::new(160, k);
    
    for i in 0..n {
        kad.add_node(NodeId::new(&format!("node{}", i)));
    }
    
    kad.build_routing_tables();
    kad
}

/// Run routing tests for DRFE-R
fn test_drfer(router: &GPRouter, num_tests: usize) -> (f64, f64, f64) {
    let mut rng = rand::thread_rng();
    let node_ids = router.node_ids();
    
    let mut successes = 0;
    let mut total_hops = 0;
    let mut total_time_us = 0.0;
    
    for _ in 0..num_tests {
        let source = node_ids.choose(&mut rng).unwrap();
        let dest = node_ids.choose(&mut rng).unwrap();
        
        if source == dest {
            continue;
        }
        
        let dest_coord = router.get_node(dest).unwrap().coord.point;
        
        let start = Instant::now();
        let result = router.simulate_delivery(source, dest, dest_coord, 1000);
        let elapsed = start.elapsed();
        
        if result.success {
            successes += 1;
            total_hops += result.hops;
            total_time_us += elapsed.as_micros() as f64;
        }
    }
    
    let success_rate = successes as f64 / num_tests as f64;
    let avg_hops = if successes > 0 {
        total_hops as f64 / successes as f64
    } else {
        0.0
    };
    let avg_latency = if successes > 0 {
        total_time_us / successes as f64
    } else {
        0.0
    };
    
    (success_rate, avg_hops, avg_latency)
}

/// Run routing tests for DHT
fn test_dht<T: DHTRouter>(dht: &T, num_tests: usize) -> (f64, f64, f64) {
    let mut rng = rand::thread_rng();
    let node_ids: Vec<NodeId> = (0..dht.node_count())
        .map(|i| NodeId::new(&format!("node{}", i)))
        .collect();
    
    let mut successes = 0;
    let mut total_hops = 0;
    let mut total_time_us = 0.0;
    
    for _ in 0..num_tests {
        let source = node_ids.choose(&mut rng).unwrap();
        let dest = node_ids.choose(&mut rng).unwrap();
        
        if source == dest {
            continue;
        }
        
        let start = Instant::now();
        let result = dht.route(source, dest, 1000);
        let elapsed = start.elapsed();
        
        if result.success {
            successes += 1;
            total_hops += result.hops;
            total_time_us += elapsed.as_micros() as f64;
        }
    }
    
    let success_rate = successes as f64 / num_tests as f64;
    let avg_hops = if successes > 0 {
        total_hops as f64 / successes as f64
    } else {
        0.0
    };
    let avg_latency = if successes > 0 {
        total_time_us / successes as f64
    } else {
        0.0
    };
    
    (success_rate, avg_hops, avg_latency)
}

fn main() {
    println!("=== Baseline Comparison Experiments ===\n");
    
    let network_sizes = vec![50, 100, 200, 300];
    let topologies = vec!["ba", "random", "grid"];
    let num_tests = 100;
    
    let mut all_results = Vec::new();
    
    for &n in &network_sizes {
        for topology in &topologies {
            println!("Testing N={}, Topology={}", n, topology);
            
            // Generate topology
            let edges = generate_topology(n, topology);
            
            // Ensure connectivity (add edges if needed)
            let mut edge_set: std::collections::HashSet<_> = edges.iter().cloned().collect();
            for i in 1..n {
                if !edge_set.contains(&(i - 1, i)) && !edge_set.contains(&(i, i - 1)) {
                    edge_set.insert((i - 1, i));
                }
            }
            let edges: Vec<_> = edge_set.into_iter().collect();
            
            println!("  Generated {} edges", edges.len());
            
            // Test DRFE-R
            println!("  Testing DRFE-R...");
            let drfer = build_drfer_router(n, &edges);
            let (sr, ah, al) = test_drfer(&drfer, num_tests);
            
            all_results.push(ComparisonResult {
                protocol: "DRFE-R".to_string(),
                network_size: n,
                topology: topology.to_string(),
                success_rate: sr,
                avg_hops: ah,
                avg_latency_us: al,
                total_tests: num_tests,
                successful_tests: (sr * num_tests as f64) as usize,
            });
            
            println!("    Success Rate: {:.2}%", sr * 100.0);
            println!("    Avg Hops: {:.2}", ah);
            println!("    Avg Latency: {:.2} μs", al);
            
            // Test Chord
            println!("  Testing Chord...");
            let chord = build_chord_dht(n, &edges);
            let (sr, ah, al) = test_dht(&chord, num_tests);
            
            all_results.push(ComparisonResult {
                protocol: "Chord".to_string(),
                network_size: n,
                topology: topology.to_string(),
                success_rate: sr,
                avg_hops: ah,
                avg_latency_us: al,
                total_tests: num_tests,
                successful_tests: (sr * num_tests as f64) as usize,
            });
            
            println!("    Success Rate: {:.2}%", sr * 100.0);
            println!("    Avg Hops: {:.2}", ah);
            println!("    Avg Latency: {:.2} μs", al);
            
            // Test Kademlia
            println!("  Testing Kademlia...");
            let kad = build_kademlia_dht(n, &edges);
            let (sr, ah, al) = test_dht(&kad, num_tests);
            
            all_results.push(ComparisonResult {
                protocol: "Kademlia".to_string(),
                network_size: n,
                topology: topology.to_string(),
                success_rate: sr,
                avg_hops: ah,
                avg_latency_us: al,
                total_tests: num_tests,
                successful_tests: (sr * num_tests as f64) as usize,
            });
            
            println!("    Success Rate: {:.2}%", sr * 100.0);
            println!("    Avg Hops: {:.2}", ah);
            println!("    Avg Latency: {:.2} μs\n", al);
        }
    }
    
    // Save results
    let summary = ExperimentSummary {
        timestamp: chrono::Utc::now().to_rfc3339(),
        results: all_results,
    };
    
    let json = serde_json::to_string_pretty(&summary).unwrap();
    let mut file = File::create("baseline_comparison.json").unwrap();
    file.write_all(json.as_bytes()).unwrap();
    
    println!("\n=== Results saved to baseline_comparison.json ===");
    
    // Print summary table
    println!("\n=== Summary ===\n");
    println!("{:<12} {:<8} {:<10} {:<12} {:<10} {:<15}", 
             "Protocol", "Size", "Topology", "Success %", "Avg Hops", "Avg Latency(μs)");
    println!("{}", "-".repeat(80));
    
    for result in &summary.results {
        println!("{:<12} {:<8} {:<10} {:<12.2} {:<10.2} {:<15.2}",
                 result.protocol,
                 result.network_size,
                 result.topology,
                 result.success_rate * 100.0,
                 result.avg_hops,
                 result.avg_latency_us);
    }
}
