//! Benchmark for routing latency
//!
//! Measures the time taken to route packets through networks of various sizes
//! and topologies. This benchmark evaluates the core routing performance of
//! the GP algorithm.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use drfe_r::coordinates::{NodeId, RoutingCoordinate};
use drfe_r::greedy_embedding::GreedyEmbedding;
use drfe_r::routing::{GPRouter, RoutingNode};
use rand::Rng;

/// Create a Barabási-Albert (BA) scale-free network
fn create_ba_network(n: usize, m: usize) -> Vec<(usize, usize)> {
    let mut edges = Vec::new();
    let mut degrees = vec![0; n];

    // Start with a complete graph of m nodes
    for i in 0..m {
        for j in (i + 1)..m {
            edges.push((i, j));
            degrees[i] += 1;
            degrees[j] += 1;
        }
    }

    let mut rng = rand::thread_rng();

    // Add remaining nodes using preferential attachment
    for i in m..n {
        let total_degree: usize = degrees.iter().sum();
        let mut targets = Vec::new();

        for _ in 0..m {
            let mut r = rng.gen_range(0..total_degree);
            for (j, &deg) in degrees.iter().enumerate() {
                if r < deg && !targets.contains(&j) && j != i {
                    targets.push(j);
                    break;
                }
                r = r.saturating_sub(deg);
            }
        }

        for &target in &targets {
            edges.push((i, target));
            degrees[i] += 1;
            degrees[target] += 1;
        }
    }

    edges
}

/// Create a router with embedded coordinates for a given network
fn create_embedded_router(n: usize, edges: &[(usize, usize)]) -> GPRouter {
    use std::collections::HashMap;

    // Build adjacency list
    let mut adjacency: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    
    for i in 0..n {
        adjacency.insert(NodeId::new(format!("node_{}", i)), Vec::new());
    }
    
    for &(i, j) in edges {
        let id_i = NodeId::new(format!("node_{}", i));
        let id_j = NodeId::new(format!("node_{}", j));
        
        adjacency.get_mut(&id_i).unwrap().push(id_j.clone());
        adjacency.get_mut(&id_j).unwrap().push(id_i.clone());
    }

    // Create PIE embedding
    let embedding_engine = GreedyEmbedding::new();
    let result = embedding_engine.embed(&adjacency).unwrap();

    // Create router with embedded coordinates
    let mut router = GPRouter::new();

    for i in 0..n {
        let node_id = NodeId::new(format!("node_{}", i));
        let coord = result.coordinates.get(&node_id).unwrap();
        let routing_coord = RoutingCoordinate::new(*coord, 0);
        let mut node = RoutingNode::new(node_id.clone(), routing_coord);

        // Set tree structure from embedding
        if let Some(children) = result.tree_children.get(&node_id) {
            node.tree_children = children.clone();
        }

        router.add_node(node);
    }

    // Add edges to router
    for &(i, j) in edges {
        let id_i = NodeId::new(format!("node_{}", i));
        let id_j = NodeId::new(format!("node_{}", j));
        router.add_edge(&id_i, &id_j);
    }

    router
}

/// Benchmark routing latency for different network sizes
fn bench_routing_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("routing_latency");

    for size in [50, 100, 200, 500].iter() {
        let n = *size;
        let m = 3; // Average degree ≈ 6

        // Create network
        let edges = create_ba_network(n, m);
        let router = create_embedded_router(n, &edges);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::new("ba_network", n), &n, |b, &_n| {
            let mut rng = rand::thread_rng();

            b.iter(|| {
                // Pick random source and destination
                let source_idx = rng.gen_range(0..n);
                let dest_idx = rng.gen_range(0..n);

                let source = NodeId::new(format!("node_{}", source_idx));
                let dest = NodeId::new(format!("node_{}", dest_idx));

                // Get destination coordinate
                let dest_coord = router.get_node(&dest).unwrap().coord.point;

                // Simulate delivery
                let result = router.simulate_delivery(&source, &dest, dest_coord, 1000);

                black_box(result);
            });
        });
    }

    group.finish();
}

/// Benchmark routing latency for different topologies
fn bench_routing_topologies(c: &mut Criterion) {
    let mut group = c.benchmark_group("routing_topologies");
    let n = 100;

    // BA network
    {
        let edges = create_ba_network(n, 3);
        let router = create_embedded_router(n, &edges);

        group.bench_function("ba_topology", |b| {
            let mut rng = rand::thread_rng();

            b.iter(|| {
                let source_idx = rng.gen_range(0..n);
                let dest_idx = rng.gen_range(0..n);

                let source = NodeId::new(format!("node_{}", source_idx));
                let dest = NodeId::new(format!("node_{}", dest_idx));
                let dest_coord = router.get_node(&dest).unwrap().coord.point;

                let result = router.simulate_delivery(&source, &dest, dest_coord, 1000);
                black_box(result);
            });
        });
    }

    // Grid network
    {
        let grid_size = 10; // 10x10 = 100 nodes
        let mut edges = Vec::new();

        for i in 0..grid_size {
            for j in 0..grid_size {
                let node = i * grid_size + j;

                // Right neighbor
                if j < grid_size - 1 {
                    edges.push((node, node + 1));
                }

                // Bottom neighbor
                if i < grid_size - 1 {
                    edges.push((node, node + grid_size));
                }
            }
        }

        let router = create_embedded_router(n, &edges);

        group.bench_function("grid_topology", |b| {
            let mut rng = rand::thread_rng();

            b.iter(|| {
                let source_idx = rng.gen_range(0..n);
                let dest_idx = rng.gen_range(0..n);

                let source = NodeId::new(format!("node_{}", source_idx));
                let dest = NodeId::new(format!("node_{}", dest_idx));
                let dest_coord = router.get_node(&dest).unwrap().coord.point;

                let result = router.simulate_delivery(&source, &dest, dest_coord, 1000);
                black_box(result);
            });
        });
    }

    group.finish();
}

/// Benchmark hop count distribution
fn bench_hop_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("hop_count");
    let n = 200;
    let edges = create_ba_network(n, 3);
    let router = create_embedded_router(n, &edges);

    group.bench_function("average_hops", |b| {
        let mut rng = rand::thread_rng();

        b.iter(|| {
            let source_idx = rng.gen_range(0..n);
            let dest_idx = rng.gen_range(0..n);

            let source = NodeId::new(format!("node_{}", source_idx));
            let dest = NodeId::new(format!("node_{}", dest_idx));
            let dest_coord = router.get_node(&dest).unwrap().coord.point;

            let result = router.simulate_delivery(&source, &dest, dest_coord, 1000);
            black_box(result.hops);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_routing_latency,
    bench_routing_topologies,
    bench_hop_count
);
criterion_main!(benches);
