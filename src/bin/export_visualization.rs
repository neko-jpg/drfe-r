//! Export visualization data for Poincaré disk frontend
//!
//! Generates JSON coordinate files for visualization comparison.

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

#[derive(Serialize, Deserialize)]
struct NetworkVisualization {
    embedding_type: String,
    topology: String,
    num_nodes: usize,
    nodes: Vec<NodeViz>,
    edges: Vec<EdgeViz>,
}

#[derive(Serialize, Deserialize)]
struct NodeViz {
    id: String,
    x: f64,
    y: f64,
    degree: usize,
}

#[derive(Serialize, Deserialize)]
struct EdgeViz {
    source: String,
    target: String,
}

fn build_ba_topology(n: usize, m: usize, seed: u64) -> HashMap<usize, HashSet<usize>> {
    let mut adj: HashMap<usize, HashSet<usize>> = HashMap::new();
    for i in 0..n {
        adj.insert(i, HashSet::new());
    }

    let mut rng = StdRng::seed_from_u64(seed);
    let mut degrees: Vec<usize> = vec![0; n];

    for i in 0..m.min(n) {
        for j in (i + 1)..m.min(n) {
            adj.get_mut(&i).unwrap().insert(j);
            adj.get_mut(&j).unwrap().insert(i);
            degrees[i] += 1;
            degrees[j] += 1;
        }
    }

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
        while connected.len() < m.min(i) {
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

    adj
}

fn export_pie_embedding(adj: &HashMap<usize, HashSet<usize>>, n: usize) -> NetworkVisualization {
    let nodes: Vec<NodeId> = (0..n).map(|i| NodeId::new(format!("node_{}", i))).collect();

    let mut adjacency: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for (&i, neighbors) in adj {
        adjacency.insert(
            nodes[i].clone(),
            neighbors.iter().map(|&j| nodes[j].clone()).collect(),
        );
    }

    let embedder = GreedyEmbedding::new();
    let result = embedder.embed(&adjacency).unwrap();

    let viz_nodes: Vec<NodeViz> = nodes
        .iter()
        .map(|id| {
            let point = result.coordinates.get(id).copied().unwrap_or_else(PoincareDiskPoint::origin);
            NodeViz {
                id: id.0.clone(),
                x: point.x,
                y: point.y,
                degree: adj.get(&id.0.trim_start_matches("node_").parse::<usize>().unwrap()).map(|s| s.len()).unwrap_or(0),
            }
        })
        .collect();

    let viz_edges: Vec<EdgeViz> = adj
        .iter()
        .flat_map(|(&i, neighbors)| {
            neighbors.iter().filter(move |&&j| i < j).map(move |&j| EdgeViz {
                source: format!("node_{}", i),
                target: format!("node_{}", j),
            })
        })
        .collect();

    NetworkVisualization {
        embedding_type: "PIE".to_string(),
        topology: "BA".to_string(),
        num_nodes: n,
        nodes: viz_nodes,
        edges: viz_edges,
    }
}

fn export_random_embedding(adj: &HashMap<usize, HashSet<usize>>, n: usize, seed: u64) -> NetworkVisualization {
    let mut rng = StdRng::seed_from_u64(seed);
    let nodes: Vec<NodeId> = (0..n).map(|i| NodeId::new(format!("node_{}", i))).collect();

    let viz_nodes: Vec<NodeViz> = (0..n)
        .map(|i| {
            let r = rng.gen::<f64>().sqrt() * 0.95;
            let theta = rng.gen::<f64>() * 2.0 * std::f64::consts::PI;
            let point = PoincareDiskPoint::from_polar(r, theta).unwrap();
            NodeViz {
                id: nodes[i].0.clone(),
                x: point.x,
                y: point.y,
                degree: adj.get(&i).map(|s| s.len()).unwrap_or(0),
            }
        })
        .collect();

    let viz_edges: Vec<EdgeViz> = adj
        .iter()
        .flat_map(|(&i, neighbors)| {
            neighbors.iter().filter(move |&&j| i < j).map(move |&j| EdgeViz {
                source: format!("node_{}", i),
                target: format!("node_{}", j),
            })
        })
        .collect();

    NetworkVisualization {
        embedding_type: "Random".to_string(),
        topology: "BA".to_string(),
        num_nodes: n,
        nodes: viz_nodes,
        edges: viz_edges,
    }
}

fn export_ricci_embedding(adj: &HashMap<usize, HashSet<usize>>, n: usize) -> NetworkVisualization {
    // Start with PIE, then apply Ricci
    let nodes: Vec<NodeId> = (0..n).map(|i| NodeId::new(format!("node_{}", i))).collect();

    let mut adjacency: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for (&i, neighbors) in adj {
        adjacency.insert(
            nodes[i].clone(),
            neighbors.iter().map(|&j| nodes[j].clone()).collect(),
        );
    }

    let embedder = GreedyEmbedding::new();
    let result = embedder.embed(&adjacency).unwrap();

    // Build Ricci graph
    let mut ricci_graph = RicciGraph::new();
    for i in 0..n {
        let point = result.coordinates.get(&nodes[i]).copied().unwrap_or_else(PoincareDiskPoint::origin);
        let coord = RoutingCoordinate::new(point, 0);
        ricci_graph.add_node(GraphNode {
            id: nodes[i].clone(),
            coord,
            neighbors: adj.get(&i).unwrap().iter().map(|&j| nodes[j].clone()).collect(),
        });
    }

    for (&i, neighbors) in adj {
        for &j in neighbors {
            if i < j {
                ricci_graph.add_edge(&nodes[i], &nodes[j]);
            }
        }
    }

    // Apply Ricci flow
    let flow = RicciFlow::new(0.05);
    let _ = flow.run_optimization(&mut ricci_graph, 30, 30);

    let viz_nodes: Vec<NodeViz> = nodes
        .iter()
        .map(|id| {
            let point = ricci_graph.nodes.get(id).map(|n| n.coord.point).unwrap_or_else(PoincareDiskPoint::origin);
            NodeViz {
                id: id.0.clone(),
                x: point.x,
                y: point.y,
                degree: adj.get(&id.0.trim_start_matches("node_").parse::<usize>().unwrap()).map(|s| s.len()).unwrap_or(0),
            }
        })
        .collect();

    let viz_edges: Vec<EdgeViz> = adj
        .iter()
        .flat_map(|(&i, neighbors)| {
            neighbors.iter().filter(move |&&j| i < j).map(move |&j| EdgeViz {
                source: format!("node_{}", i),
                target: format!("node_{}", j),
            })
        })
        .collect();

    NetworkVisualization {
        embedding_type: "Ricci-Fixed".to_string(),
        topology: "BA".to_string(),
        num_nodes: n,
        nodes: viz_nodes,
        edges: viz_edges,
    }
}

fn main() {
    println!("DRFE-R Visualization Export");
    println!("============================\n");

    let n = 100;
    let seed = 12345u64;

    println!("Generating BA topology with {} nodes...", n);
    let adj = build_ba_topology(n, 3, seed);
    println!("Topology has {} edges", adj.values().map(|s| s.len()).sum::<usize>() / 2);

    println!("\nExporting PIE embedding...");
    let pie_viz = export_pie_embedding(&adj, n);
    let json = serde_json::to_string_pretty(&pie_viz).unwrap();
    let mut file = File::create("paper_data/visualization/coordinates_pie.json").unwrap();
    file.write_all(json.as_bytes()).unwrap();
    println!("✓ Saved coordinates_pie.json");

    println!("Exporting Random embedding...");
    let random_viz = export_random_embedding(&adj, n, seed);
    let json = serde_json::to_string_pretty(&random_viz).unwrap();
    let mut file = File::create("paper_data/visualization/coordinates_random.json").unwrap();
    file.write_all(json.as_bytes()).unwrap();
    println!("✓ Saved coordinates_random.json");

    println!("Exporting Ricci-Fixed embedding...");
    let ricci_viz = export_ricci_embedding(&adj, n);
    let json = serde_json::to_string_pretty(&ricci_viz).unwrap();
    let mut file = File::create("paper_data/visualization/coordinates_ricci.json").unwrap();
    file.write_all(json.as_bytes()).unwrap();
    println!("✓ Saved coordinates_ricci.json");

    println!("\nAll visualizations exported to paper_data/visualization/");
}
