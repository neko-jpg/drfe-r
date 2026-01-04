//! Landmark-guided routing utilities.

use crate::coordinates::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LandmarkRoutingConfig {
    pub num_landmarks: Option<usize>,
    pub lookahead_depth: usize,
    pub lookahead_max_nodes: usize,
    pub landmark_weight: f64,
    pub hyperbolic_weight: f64,
}

impl Default for LandmarkRoutingConfig {
    fn default() -> Self {
        Self {
            num_landmarks: None,
            lookahead_depth: 3,
            lookahead_max_nodes: 5000,
            landmark_weight: 1.0,
            hyperbolic_weight: 0.15,
        }
    }
}

impl LandmarkRoutingConfig {
    pub fn effective_landmark_count(&self, n: usize) -> usize {
        match self.num_landmarks {
            Some(k) if k > 0 => k.min(n),
            _ => {
                let sqrt_n = (n as f64).sqrt().ceil() as usize;
                (2 * sqrt_n).min(64).max(4).min(n)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct LandmarkRoutingTable {
    pub landmarks: Vec<NodeId>,
    pub distances: HashMap<NodeId, Vec<u32>>,
}

impl LandmarkRoutingTable {
    pub fn build(
        adjacency: &HashMap<NodeId, Vec<NodeId>>,
        config: &LandmarkRoutingConfig,
    ) -> Result<Self, String> {
        if adjacency.is_empty() {
            return Err("Empty graph".to_string());
        }

        let num_landmarks = config.effective_landmark_count(adjacency.len());
        let landmarks = select_landmarks(adjacency, num_landmarks);
        if landmarks.is_empty() {
            return Err("No landmarks selected".to_string());
        }

        let distances = compute_all_landmark_distances(adjacency, &landmarks);
        Ok(Self {
            landmarks,
            distances,
        })
    }

    pub fn distance(&self, from: &NodeId, to: &NodeId) -> Option<f64> {
        let from_vec = self.distances.get(from)?;
        let to_vec = self.distances.get(to)?;
        if from_vec.len() != self.landmarks.len() || to_vec.len() != self.landmarks.len() {
            return None;
        }
        let mut sum: u64 = 0;
        for i in 0..self.landmarks.len() {
            let a = from_vec[i];
            let b = to_vec[i];
            if a == u32::MAX || b == u32::MAX {
                return None;
            }
            sum += (a as i64 - b as i64).abs() as u64;
        }
        let denom = self.landmarks.len().max(1) as f64;
        Some(sum as f64 / denom)
    }
}

fn select_landmarks(
    adjacency: &HashMap<NodeId, Vec<NodeId>>,
    num_landmarks: usize,
) -> Vec<NodeId> {
    if adjacency.is_empty() || num_landmarks == 0 {
        return Vec::new();
    }

    let first = adjacency
        .iter()
        .max_by_key(|(_, neighbors)| neighbors.len())
        .map(|(id, _)| id.clone())
        .unwrap();

    let mut landmarks = Vec::with_capacity(num_landmarks);
    let mut selected = HashSet::new();
    landmarks.push(first.clone());
    selected.insert(first.clone());

    let mut min_distances: HashMap<NodeId, u32> = bfs_distances(adjacency, &first);

    while landmarks.len() < num_landmarks {
        let next = min_distances
            .iter()
            .filter(|(id, _)| !selected.contains(*id))
            .max_by_key(|(_, dist)| *dist)
            .map(|(id, _)| id.clone());

        let next = match next {
            Some(id) => id,
            None => break,
        };

        let distances = bfs_distances(adjacency, &next);
        for (node, dist) in distances {
            let entry = min_distances.entry(node).or_insert(u32::MAX);
            if dist < *entry {
                *entry = dist;
            }
        }

        selected.insert(next.clone());
        landmarks.push(next);
    }

    landmarks
}

fn compute_all_landmark_distances(
    adjacency: &HashMap<NodeId, Vec<NodeId>>,
    landmarks: &[NodeId],
) -> HashMap<NodeId, Vec<u32>> {
    let mut all_distances: HashMap<NodeId, Vec<u32>> = HashMap::new();

    for node in adjacency.keys() {
        all_distances.insert(node.clone(), vec![u32::MAX; landmarks.len()]);
    }

    for (i, landmark) in landmarks.iter().enumerate() {
        let distances = bfs_distances(adjacency, landmark);
        for (node, dist) in distances {
            if let Some(vec) = all_distances.get_mut(&node) {
                vec[i] = dist;
            }
        }
    }

    all_distances
}

fn bfs_distances(
    adjacency: &HashMap<NodeId, Vec<NodeId>>,
    source: &NodeId,
) -> HashMap<NodeId, u32> {
    let mut distances = HashMap::new();
    let mut queue = VecDeque::new();
    distances.insert(source.clone(), 0);
    queue.push_back(source.clone());

    while let Some(current) = queue.pop_front() {
        let current_dist = *distances.get(&current).unwrap_or(&0);
        if let Some(neighbors) = adjacency.get(&current) {
            for neighbor in neighbors {
                if !distances.contains_key(neighbor) {
                    distances.insert(neighbor.clone(), current_dist + 1);
                    queue.push_back(neighbor.clone());
                }
            }
        }
    }

    distances
}
