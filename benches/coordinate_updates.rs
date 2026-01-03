//! Benchmark for coordinate updates
//!
//! Measures the time taken to update node coordinates using Ricci flow.
//! This benchmark evaluates the computational overhead of maintaining
//! optimal embeddings as the network topology changes.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use drfe_r::greedy_embedding::GreedyEmbedding;
use drfe_r::PoincareDiskPoint;
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

/// Benchmark initial embedding computation
fn bench_initial_embedding(c: &mut Criterion) {
    use std::collections::HashMap;
    
    let mut group = c.benchmark_group("initial_embedding");

    for size in [50, 100, 200, 500].iter() {
        let n = *size;
        let m = 3;
        let edges = create_ba_network(n, m);

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("compute_embedding", n), &n, |b, &_n| {
            b.iter(|| {
                // Build adjacency list
                let mut adjacency: HashMap<drfe_r::coordinates::NodeId, Vec<drfe_r::coordinates::NodeId>> = HashMap::new();
                
                for i in 0..n {
                    adjacency.insert(drfe_r::coordinates::NodeId::new(format!("node_{}", i)), Vec::new());
                }
                
                for &(i, j) in &edges {
                    let id_i = drfe_r::coordinates::NodeId::new(format!("node_{}", i));
                    let id_j = drfe_r::coordinates::NodeId::new(format!("node_{}", j));
                    
                    adjacency.get_mut(&id_i).unwrap().push(id_j.clone());
                    adjacency.get_mut(&id_j).unwrap().push(id_i.clone());
                }

                // Compute embedding
                let embedding_engine = GreedyEmbedding::new();
                let result = embedding_engine.embed(&adjacency).unwrap();

                black_box(&result);
            });
        });
    }

    group.finish();
}

/// Benchmark coordinate refinement
fn bench_coordinate_refinement(c: &mut Criterion) {
    use std::collections::HashMap;
    
    let mut group = c.benchmark_group("coordinate_refinement");

    for size in [50, 100, 200].iter() {
        let n = *size;
        let m = 3;
        let edges = create_ba_network(n, m);

        // Build adjacency list
        let mut adjacency: HashMap<drfe_r::coordinates::NodeId, Vec<drfe_r::coordinates::NodeId>> = HashMap::new();
        
        for i in 0..n {
            adjacency.insert(drfe_r::coordinates::NodeId::new(format!("node_{}", i)), Vec::new());
        }
        
        for &(i, j) in &edges {
            let id_i = drfe_r::coordinates::NodeId::new(format!("node_{}", i));
            let id_j = drfe_r::coordinates::NodeId::new(format!("node_{}", j));
            
            adjacency.get_mut(&id_i).unwrap().push(id_j.clone());
            adjacency.get_mut(&id_j).unwrap().push(id_i.clone());
        }

        // Create initial embedding
        let embedding_engine = GreedyEmbedding::new();
        let result = embedding_engine.embed(&adjacency).unwrap();

        // Get initial coordinates
        let mut coords: HashMap<drfe_r::coordinates::NodeId, PoincareDiskPoint> = result.coordinates.clone();

        group.throughput(Throughput::Elements(edges.len() as u64));
        group.bench_with_input(BenchmarkId::new("refine_step", n), &n, |b, &_n| {
            b.iter(|| {
                // Perform one refinement step
                GreedyEmbedding::refine_embedding(&mut coords, &adjacency, 1, 0.01);

                black_box(&coords);
            });
        });
    }

    group.finish();
}

/// Benchmark coordinate update after topology change
fn bench_topology_change(c: &mut Criterion) {
    use std::collections::HashMap;
    
    let mut group = c.benchmark_group("topology_change");
    let n = 100;
    let m = 3;

    group.bench_function("add_node_and_update", |b| {
        b.iter(|| {
            let edges = create_ba_network(n, m);

            // Build adjacency list
            let mut adjacency: HashMap<drfe_r::coordinates::NodeId, Vec<drfe_r::coordinates::NodeId>> = HashMap::new();
            
            for i in 0..n {
                adjacency.insert(drfe_r::coordinates::NodeId::new(format!("node_{}", i)), Vec::new());
            }
            
            for &(i, j) in &edges {
                let id_i = drfe_r::coordinates::NodeId::new(format!("node_{}", i));
                let id_j = drfe_r::coordinates::NodeId::new(format!("node_{}", j));
                
                adjacency.get_mut(&id_i).unwrap().push(id_j.clone());
                adjacency.get_mut(&id_j).unwrap().push(id_i.clone());
            }

            let embedding_engine = GreedyEmbedding::new();
            let _result = embedding_engine.embed(&adjacency).unwrap();

            // Add a new node
            let new_node_id = drfe_r::coordinates::NodeId::new(format!("node_{}", n));
            adjacency.insert(new_node_id.clone(), Vec::new());

            // Connect to 3 random existing nodes
            let mut rng = rand::thread_rng();
            for _ in 0..3 {
                let target = rng.gen_range(0..n);
                let target_id = drfe_r::coordinates::NodeId::new(format!("node_{}", target));
                adjacency.get_mut(&new_node_id).unwrap().push(target_id.clone());
                adjacency.get_mut(&target_id).unwrap().push(new_node_id.clone());
            }

            // Recompute embedding
            let result = embedding_engine.embed(&adjacency).unwrap();

            black_box(&result);
        });
    });

    group.finish();
}

/// Benchmark convergence of coordinate refinement
fn bench_refinement_convergence(c: &mut Criterion) {
    use std::collections::HashMap;
    
    let mut group = c.benchmark_group("refinement_convergence");
    let n = 100;
    let m = 3;
    let edges = create_ba_network(n, m);

    // Build adjacency list
    let mut adjacency: HashMap<drfe_r::coordinates::NodeId, Vec<drfe_r::coordinates::NodeId>> = HashMap::new();
    
    for i in 0..n {
        adjacency.insert(drfe_r::coordinates::NodeId::new(format!("node_{}", i)), Vec::new());
    }
    
    for &(i, j) in &edges {
        let id_i = drfe_r::coordinates::NodeId::new(format!("node_{}", i));
        let id_j = drfe_r::coordinates::NodeId::new(format!("node_{}", j));
        
        adjacency.get_mut(&id_i).unwrap().push(id_j.clone());
        adjacency.get_mut(&id_j).unwrap().push(id_i.clone());
    }

    // Create initial embedding
    let embedding_engine = GreedyEmbedding::new();
    let result = embedding_engine.embed(&adjacency).unwrap();

    let coords = result.coordinates.clone();

    group.bench_function("converge_10_steps", |b| {
        b.iter(|| {
            let mut local_coords = coords.clone();

            // Run 10 refinement steps
            GreedyEmbedding::refine_embedding(&mut local_coords, &adjacency, 10, 0.01);

            black_box(&local_coords);
        });
    });

    group.finish();
}

/// Benchmark memory usage of coordinate storage
fn bench_coordinate_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("coordinate_memory");

    for size in [100, 500, 1000, 5000].iter() {
        let n = *size;

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("allocate_coords", n), &n, |b, &_n| {
            b.iter(|| {
                let coords: Vec<PoincareDiskPoint> = (0..n)
                    .map(|i| {
                        let angle = (i as f64 / n as f64) * 2.0 * std::f64::consts::PI;
                        let r = 0.5;
                        PoincareDiskPoint::from_polar(r, angle).unwrap()
                    })
                    .collect();

                black_box(coords);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_initial_embedding,
    bench_coordinate_refinement,
    bench_topology_change,
    bench_refinement_convergence,
    bench_coordinate_memory
);
criterion_main!(benches);
