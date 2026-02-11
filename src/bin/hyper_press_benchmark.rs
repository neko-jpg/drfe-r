//! HYPER-PRESS Benchmark: Evaluate H^2 + Φ_t routing
//!
//! Compare HYPER-PRESS with PIE-DFS and PIE+TZ on various topologies

use std::collections::{HashMap, HashSet};
use drfe_r::coordinates::NodeId;
use drfe_r::hyper_press::HyperPress;
use rand::prelude::*;

/// Generate Barabasi-Albert graph
fn generate_ba_graph(n: usize, m: usize) -> HashMap<NodeId, Vec<NodeId>> {
    let mut adj: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    let mut rng = rand::thread_rng();
    
    // Start with a clique of m+1 nodes
    for i in 0..=m {
        let id = NodeId(format!("n{}", i));
        adj.entry(id.clone()).or_insert_with(Vec::new);
        for j in 0..i {
            let other = NodeId(format!("n{}", j));
            adj.get_mut(&id).unwrap().push(other.clone());
            adj.get_mut(&other).unwrap().push(id.clone());
        }
    }
    
    // Preferential attachment for remaining nodes
    for i in (m + 1)..n {
        let new_node = NodeId(format!("n{}", i));
        adj.insert(new_node.clone(), Vec::new());
        
        // Calculate degree sum for preferential attachment
        let degrees: Vec<(NodeId, usize)> = adj.iter()
            .filter(|(k, _)| **k != new_node)
            .map(|(k, v)| (k.clone(), v.len()))
            .collect();
        let total_degree: usize = degrees.iter().map(|(_, d)| d).sum();
        
        // Select m distinct targets
        let mut targets = HashSet::new();
        while targets.len() < m && targets.len() < degrees.len() {
            let r: f64 = rng.gen();
            let mut cumsum = 0.0;
            for (node, deg) in &degrees {
                cumsum += (*deg as f64) / (total_degree as f64);
                if r < cumsum && !targets.contains(node) {
                    targets.insert(node.clone());
                    break;
                }
            }
        }
        
        // Add edges
        for target in targets {
            adj.get_mut(&new_node).unwrap().push(target.clone());
            adj.get_mut(&target).unwrap().push(new_node.clone());
        }
    }
    
    adj
}

/// Test HYPER-PRESS routing success rate
fn test_hyper_press_routing(
    hp: &HyperPress,
    adj: &HashMap<NodeId, Vec<NodeId>>,
    samples: usize,
    use_lookahead: bool,
) -> (f64, f64, f64) {
    let nodes: Vec<NodeId> = adj.keys().cloned().collect();
    let mut rng = rand::thread_rng();
    
    let mut successes = 0;
    let mut total_hops = 0;
    let mut gravity_usage = 0;
    let mut total_decisions = 0;
    
    let max_hops = nodes.len() * 2;
    
    for _ in 0..samples {
        let source = nodes.choose(&mut rng).unwrap().clone();
        let target = nodes.choose(&mut rng).unwrap().clone();
        if source == target {
            continue;
        }
        
        let mut current = source;
        let mut visited = HashSet::new();
        let mut hops = 0;
        let mut reached = false;
        
        while hops < max_hops {
            visited.insert(current.clone());
            
            if current == target {
                reached = true;
                break;
            }
            
            let next = if use_lookahead {
                hp.find_best_neighbor_lookahead(&current, &target, &visited)
            } else {
                hp.find_best_neighbor(&current, &target, &visited)
            };
            
            match next {
                Some(n) => {
                    gravity_usage += 1;
                    total_decisions += 1;
                    current = n;
                    hops += 1;
                }
                None => {
                    // Fallback: try any unvisited neighbor
                    total_decisions += 1;
                    if let Some(neighbors) = adj.get(&current) {
                        if let Some(n) = neighbors.iter().find(|n| !visited.contains(*n)) {
                            current = n.clone();
                            hops += 1;
                            continue;
                        }
                    }
                    break;
                }
            }
        }
        
        if reached {
            successes += 1;
            total_hops += hops;
        }
    }
    
    let success_rate = (successes as f64) / (samples as f64) * 100.0;
    let avg_hops = if successes > 0 { (total_hops as f64) / (successes as f64) } else { 0.0 };
    let gravity_pct = if total_decisions > 0 { (gravity_usage as f64) / (total_decisions as f64) * 100.0 } else { 0.0 };
    
    (success_rate, avg_hops, gravity_pct)
}

fn main() {
    println!("╔════════════════════════════════════════════════════════════════════════╗");
    println!("║               HYPER-PRESS Benchmark (H^2 + Φ_t)                        ║");
    println!("╚════════════════════════════════════════════════════════════════════════╝");
    println!();
    
    let sizes = [100, 300, 500, 1000, 2000];
    let samples = 200;
    
    println!("Nodes    λ      Success%   AvgHops    Gravity%   Lookahead");
    println!("══════════════════════════════════════════════════════════════════");
    
    for &n in &sizes {
        println!("\n▶ Network size: {} nodes", n);
        
        // Generate BA graph
        let adj = generate_ba_graph(n, 3);
        
        // Test with different lambda values
        for &lambda in &[0.0, 0.3, 1.0] {
            let mut hp = HyperPress::new();
            hp.build_from_adjacency(&adj);
            hp.set_lambda(lambda);
            
            // Without lookahead
            let (success, hops, gravity) = test_hyper_press_routing(&hp, &adj, samples, false);
            println!("{:<8} {:<6.1} {:<10.2} {:<10.2} {:<10.2} No", n, lambda, success, hops, gravity);
            
            // With lookahead
            let (success_la, hops_la, gravity_la) = test_hyper_press_routing(&hp, &adj, samples, true);
            println!("{:<8} {:<6.1} {:<10.2} {:<10.2} {:<10.2} Yes", n, lambda, success_la, hops_la, gravity_la);
        }
    }
    
    println!("\n✓ Benchmark complete");
}
