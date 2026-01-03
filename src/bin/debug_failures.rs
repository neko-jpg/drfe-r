//! Debug script to analyze routing failures
//! 
//! Identifies specific node pairs that fail and why

use drfe_r::coordinates::{NodeId, RoutingCoordinate};
use drfe_r::greedy_embedding::GreedyEmbedding;
use drfe_r::routing::{GPRouter, RoutingNode};
use drfe_r::PoincareDiskPoint;
use rand::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};

fn main() {
    println!("DRFE-R Failure Analysis");
    println!("=======================\n");

    let seed = 42u64;
    let num_nodes = 500;
    let m = 3; // BA parameter

    // Generate BA network
    let mut rng = StdRng::seed_from_u64(seed);
    let mut degrees: Vec<usize> = Vec::new();
    let mut nodes: Vec<NodeId> = Vec::new();
    let mut adjacency_idx: Vec<Vec<usize>> = Vec::new();

    for i in 0..num_nodes {
        let id = NodeId::new(format!("node_{}", i));
        nodes.push(id);
        degrees.push(0);
        adjacency_idx.push(Vec::new());
    }

    // Build initial complete graph of m nodes
    for i in 0..m.min(num_nodes) {
        for j in (i + 1)..m.min(num_nodes) {
            adjacency_idx[i].push(j);
            adjacency_idx[j].push(i);
            degrees[i] += 1;
            degrees[j] += 1;
        }
    }

    // Preferential attachment
    for i in m..num_nodes {
        let total_degree: usize = degrees.iter().take(i).sum();
        if total_degree == 0 {
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

    // Check connectivity using BFS
    let mut visited = vec![false; num_nodes];
    let mut queue = VecDeque::new();
    queue.push_back(0);
    visited[0] = true;
    let mut count = 1;
    
    while let Some(node) = queue.pop_front() {
        for &neighbor in &adjacency_idx[node] {
            if !visited[neighbor] {
                visited[neighbor] = true;
                count += 1;
                queue.push_back(neighbor);
            }
        }
    }

    println!("Graph connectivity check:");
    println!("  Total nodes: {}", num_nodes);
    println!("  Reachable from node 0: {}", count);
    if count == num_nodes {
        println!("  ✓ Graph is connected");
    } else {
        println!("  ✗ Graph is DISCONNECTED!");
        println!("  Unreachable nodes:");
        for (i, &v) in visited.iter().enumerate() {
            if !v {
                println!("    - node_{}", i);
            }
        }
    }

    // Now create router with embedding
    let mut adjacency: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for i in 0..num_nodes {
        let neighbor_ids: Vec<NodeId> = adjacency_idx[i]
            .iter()
            .map(|&j| nodes[j].clone())
            .collect();
        adjacency.insert(nodes[i].clone(), neighbor_ids);
    }

    let embedder = GreedyEmbedding::new();
    let embedding_result = embedder.embed(&adjacency).expect("Embedding should succeed");

    // Build tree parent map
    let mut tree_parent: HashMap<NodeId, Option<NodeId>> = HashMap::new();
    tree_parent.insert(embedding_result.root.clone(), None);
    for (parent_id, children) in &embedding_result.tree_children {
        for child_id in children {
            tree_parent.insert(child_id.clone(), Some(parent_id.clone()));
        }
    }

    // Create router
    let mut router = GPRouter::new();
    for i in 0..num_nodes {
        let node_id = &nodes[i];
        let point = embedding_result
            .coordinates
            .get(node_id)
            .copied()
            .unwrap_or_else(PoincareDiskPoint::origin);
        let coord = RoutingCoordinate::new(point, 0);
        let mut routing_node = RoutingNode::new(node_id.clone(), coord);

        let parent = tree_parent.get(node_id).cloned().flatten();
        let children = embedding_result
            .tree_children
            .get(node_id)
            .cloned()
            .unwrap_or_default();
        routing_node.set_tree_info(parent, children);
        
        router.add_node(routing_node);
    }

    for i in 0..num_nodes {
        for &j in &adjacency_idx[i] {
            if i < j {
                router.add_edge(&nodes[i], &nodes[j]);
            }
        }
    }

    println!("\nRouter statistics:");
    println!("  Nodes: {}", router.node_count());
    println!("  Edges: {}", router.edge_count());

    // Test a few routing pairs
    println!("\nTesting specific routing pairs:");
    let test_pairs = [
        (0, 1),
        (0, 499),
        (100, 200),
        (250, 400),
    ];

    for (src_idx, dst_idx) in test_pairs {
        let source = &nodes[src_idx];
        let dest = &nodes[dst_idx];

        if let Some(dest_node) = router.get_node(dest) {
            let result = router.simulate_delivery(
                source,
                dest,
                dest_node.coord.point,
                10000, // Very high TTL
            );

            println!("  {} -> {}: {} (hops: {}, reason: {:?})",
                source.0, dest.0,
                if result.success { "SUCCESS" } else { "FAILED" },
                result.hops,
                result.failure_reason
            );
        }
    }

    // Run bulk test and collect failure statistics
    println!("\nBulk routing test (200 pairs):");
    let mut successes = 0;
    let mut failures = 0;
    let mut failure_reasons: HashMap<String, usize> = HashMap::new();
    let mut failed_pairs: Vec<(usize, usize)> = Vec::new();

    let mut test_rng = StdRng::seed_from_u64(seed + 1000);
    for _ in 0..200 {
        let src_idx = test_rng.gen_range(0..num_nodes);
        let mut dst_idx = test_rng.gen_range(0..num_nodes);
        while dst_idx == src_idx {
            dst_idx = test_rng.gen_range(0..num_nodes);
        }

        let source = &nodes[src_idx];
        let dest = &nodes[dst_idx];

        if let Some(dest_node) = router.get_node(dest) {
            let result = router.simulate_delivery(
                source,
                dest,
                dest_node.coord.point,
                10000,
            );

            if result.success {
                successes += 1;
            } else {
                failures += 1;
                if let Some(reason) = &result.failure_reason {
                    *failure_reasons.entry(reason.clone()).or_insert(0) += 1;
                }
                if failed_pairs.len() < 5 {
                    failed_pairs.push((src_idx, dst_idx));
                }
            }
        }
    }

    println!("  Successes: {}", successes);
    println!("  Failures: {}", failures);
    println!("  Success rate: {:.2}%", successes as f64 / 200.0 * 100.0);
    
    if !failure_reasons.is_empty() {
        println!("\nFailure reasons:");
        for (reason, count) in &failure_reasons {
            println!("  {}: {}", reason, count);
        }
    }

    if !failed_pairs.is_empty() {
        println!("\nSample failed pairs:");
        for (src, dst) in &failed_pairs {
            println!("  {} -> {}", src, dst);
        }
        
        // Detailed trace of first failed pair
        let (src_idx, dst_idx) = failed_pairs[0];
        let source = &nodes[src_idx];
        let dest = &nodes[dst_idx];
        
        println!("\n=== Detailed trace of {} -> {} ===", src_idx, dst_idx);
        
        // Check if there's a path using BFS
        let mut visited_bfs = vec![false; num_nodes];
        let mut queue = VecDeque::new();
        queue.push_back(src_idx);
        visited_bfs[src_idx] = true;
        let mut found = false;
        let mut dist = 0;
        
        while let Some(node) = queue.pop_front() {
            if node == dst_idx {
                found = true;
                break;
            }
            for &neighbor in &adjacency_idx[node] {
                if !visited_bfs[neighbor] {
                    visited_bfs[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }
        
        println!("  BFS path exists: {}", found);
        
        // Check connectivity of both nodes
        println!("  Source node_{} degree: {}", src_idx, adjacency_idx[src_idx].len());
        println!("  Dest node_{} degree: {}", dst_idx, adjacency_idx[dst_idx].len());
    }
}
