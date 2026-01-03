//! Baseline DHT implementations for comparison with DRFE-R
//!
//! This module implements simplified versions of Chord and Kademlia DHTs
//! for performance comparison purposes.

use crate::coordinates::NodeId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

/// Trait for DHT routing protocols
pub trait DHTRouter {
    /// Route a packet from source to destination
    fn route(&self, source: &NodeId, destination: &NodeId, max_hops: u32) -> DHTRoutingResult;
    
    /// Get the name of the protocol
    fn protocol_name(&self) -> &str;
    
    /// Get number of nodes in the network
    fn node_count(&self) -> usize;
}

/// Result of DHT routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DHTRoutingResult {
    pub success: bool,
    pub hops: u32,
    pub path: Vec<NodeId>,
    pub failure_reason: Option<String>,
}

/// Chord DHT implementation
/// 
/// Chord uses consistent hashing to map keys to nodes in a ring.
/// Each node maintains a finger table with O(log N) entries pointing to
/// nodes at exponentially increasing distances around the ring.
pub struct ChordDHT {
    /// All nodes in the ring, sorted by ID
    nodes: Vec<ChordNode>,
    /// Map from NodeId to index in nodes vector
    node_map: HashMap<NodeId, usize>,
    /// Number of bits in the identifier space (m)
    m: usize,
}

#[derive(Debug, Clone)]
struct ChordNode {
    id: NodeId,
    /// Hash value in the ring (0 to 2^m - 1)
    hash: u64,
    /// Finger table: finger[i] points to successor of (n + 2^i) mod 2^m
    finger_table: Vec<NodeId>,
}

impl ChordDHT {
    /// Create a new Chord DHT with m-bit identifier space
    pub fn new(m: usize) -> Self {
        Self {
            nodes: Vec::new(),
            node_map: HashMap::new(),
            m,
        }
    }
    
    /// Add a node to the Chord ring
    pub fn add_node(&mut self, id: NodeId) {
        let hash = self.hash_node_id(&id);
        let node = ChordNode {
            id: id.clone(),
            hash,
            finger_table: Vec::new(),
        };
        
        self.nodes.push(node);
        self.node_map.insert(id, self.nodes.len() - 1);
    }
    
    /// Build finger tables for all nodes
    pub fn build_finger_tables(&mut self) {
        // Sort nodes by hash value
        self.nodes.sort_by_key(|n| n.hash);
        
        // Rebuild node_map after sorting
        self.node_map.clear();
        for (idx, node) in self.nodes.iter().enumerate() {
            self.node_map.insert(node.id.clone(), idx);
        }
        
        // Build finger table for each node
        for i in 0..self.nodes.len() {
            let node_hash = self.nodes[i].hash;
            let mut finger_table = Vec::new();
            
            for k in 0..self.m {
                let target = (node_hash + (1u64 << k)) % (1u64 << self.m);
                let successor = self.find_successor(target);
                finger_table.push(successor);
            }
            
            self.nodes[i].finger_table = finger_table;
        }
    }
    
    /// Hash a node ID to the ring space
    fn hash_node_id(&self, id: &NodeId) -> u64 {
        let mut hasher = Sha256::new();
        hasher.update(id.0.as_bytes());
        let result = hasher.finalize();
        
        // Take first 8 bytes and mask to m bits
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&result[0..8]);
        let hash = u64::from_be_bytes(bytes);
        
        hash % (1u64 << self.m)
    }
    
    /// Find the successor node for a given hash value
    fn find_successor(&self, hash: u64) -> NodeId {
        // Binary search for the first node with hash >= target
        let idx = self.nodes.partition_point(|n| n.hash < hash);
        
        if idx < self.nodes.len() {
            self.nodes[idx].id.clone()
        } else {
            // Wrap around to first node
            self.nodes[0].id.clone()
        }
    }
    
    /// Find the closest preceding node in the finger table
    fn closest_preceding_finger(&self, node_idx: usize, target_hash: u64) -> Option<NodeId> {
        let node = &self.nodes[node_idx];
        
        // Search finger table in reverse order for closest preceding node
        for finger_id in node.finger_table.iter().rev() {
            if let Some(&finger_idx) = self.node_map.get(finger_id) {
                let finger_hash = self.nodes[finger_idx].hash;
                
                // Check if finger is between node and target (circular)
                if self.is_between(node.hash, finger_hash, target_hash) {
                    return Some(finger_id.clone());
                }
            }
        }
        
        None
    }
    
    /// Check if value is between start and end in circular space
    fn is_between(&self, start: u64, value: u64, end: u64) -> bool {
        if start < end {
            start < value && value < end
        } else {
            value > start || value < end
        }
    }
}

impl DHTRouter for ChordDHT {
    fn route(&self, source: &NodeId, destination: &NodeId, max_hops: u32) -> DHTRoutingResult {
        let dest_hash = self.hash_node_id(destination);
        
        let mut current_id = source.clone();
        let mut path = vec![current_id.clone()];
        let mut hops = 0;
        
        while hops < max_hops {
            // Check if we've reached the destination
            if &current_id == destination {
                return DHTRoutingResult {
                    success: true,
                    hops,
                    path,
                    failure_reason: None,
                };
            }
            
            // Get current node index
            let current_idx = match self.node_map.get(&current_id) {
                Some(&idx) => idx,
                None => {
                    return DHTRoutingResult {
                        success: false,
                        hops,
                        path,
                        failure_reason: Some("Current node not found".to_string()),
                    };
                }
            };
            
            let current_hash = self.nodes[current_idx].hash;
            
            // Check if destination is between current and successor
            let successor_id = if current_idx + 1 < self.nodes.len() {
                self.nodes[current_idx + 1].id.clone()
            } else {
                self.nodes[0].id.clone()
            };
            
            let successor_idx = self.node_map[&successor_id];
            let successor_hash = self.nodes[successor_idx].hash;
            
            if dest_hash == current_hash || 
               self.is_between(current_hash, dest_hash, successor_hash) ||
               dest_hash == successor_hash {
                // Destination is our successor
                current_id = successor_id;
                path.push(current_id.clone());
                hops += 1;
                continue;
            }
            
            // Find closest preceding finger
            if let Some(next_id) = self.closest_preceding_finger(current_idx, dest_hash) {
                current_id = next_id;
                path.push(current_id.clone());
                hops += 1;
            } else {
                // No better finger, go to successor
                current_id = successor_id;
                path.push(current_id.clone());
                hops += 1;
            }
        }
        
        DHTRoutingResult {
            success: false,
            hops,
            path,
            failure_reason: Some("Max hops exceeded".to_string()),
        }
    }
    
    fn protocol_name(&self) -> &str {
        "Chord"
    }
    
    fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

/// Kademlia DHT implementation
///
/// Kademlia uses XOR metric for distance and maintains k-buckets
/// for routing. Each node maintains O(log N) contacts.
pub struct KademliaDHT {
    /// All nodes in the network
    nodes: HashMap<NodeId, KademliaNode>,
    /// Number of bits in node ID
    id_bits: usize,
    /// Bucket size (k parameter)
    k: usize,
}

#[derive(Debug, Clone)]
struct KademliaNode {
    /// Hash value for XOR distance calculation
    hash: Vec<u8>,
    /// K-buckets: buckets[i] contains nodes at distance 2^i to 2^(i+1)
    buckets: Vec<Vec<NodeId>>,
}

impl KademliaDHT {
    /// Create a new Kademlia DHT
    pub fn new(id_bits: usize, k: usize) -> Self {
        Self {
            nodes: HashMap::new(),
            id_bits,
            k,
        }
    }
    
    /// Add a node to the network
    pub fn add_node(&mut self, id: NodeId) {
        let hash = self.hash_node_id(&id);
        let node = KademliaNode {
            hash,
            buckets: vec![Vec::new(); self.id_bits],
        };
        
        self.nodes.insert(id, node);
    }
    
    /// Build k-buckets for all nodes
    pub fn build_routing_tables(&mut self) {
        let all_ids: Vec<NodeId> = self.nodes.keys().cloned().collect();
        
        for node_id in &all_ids {
            let node_hash = self.nodes[node_id].hash.clone();
            
            for other_id in &all_ids {
                if node_id == other_id {
                    continue;
                }
                
                let other_hash = &self.nodes[other_id].hash;
                let distance = self.xor_distance(&node_hash, other_hash);
                let bucket_idx = self.get_bucket_index(distance);
                
                if bucket_idx < self.id_bits {
                    let node = self.nodes.get_mut(node_id).unwrap();
                    if node.buckets[bucket_idx].len() < self.k {
                        node.buckets[bucket_idx].push(other_id.clone());
                    }
                }
            }
        }
    }
    
    /// Hash a node ID
    fn hash_node_id(&self, id: &NodeId) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(id.0.as_bytes());
        let result = hasher.finalize();
        result[0..(self.id_bits / 8)].to_vec()
    }
    
    /// Calculate XOR distance between two hashes
    fn xor_distance(&self, a: &[u8], b: &[u8]) -> Vec<u8> {
        a.iter().zip(b.iter()).map(|(x, y)| x ^ y).collect()
    }
    
    /// Get bucket index for a distance (position of most significant bit)
    fn get_bucket_index(&self, distance: Vec<u8>) -> usize {
        for (byte_idx, &byte) in distance.iter().enumerate() {
            if byte != 0 {
                let bit_pos = 7 - byte.leading_zeros() as usize;
                return byte_idx * 8 + bit_pos;
            }
        }
        self.id_bits // All zeros (same node)
    }
    
    /// Find k closest nodes to a target
    fn find_closest_nodes(&self, current_id: &NodeId, target_hash: &[u8], k: usize) -> Vec<NodeId> {
        let current_node = match self.nodes.get(current_id) {
            Some(n) => n,
            None => return Vec::new(),
        };
        
        let mut candidates: Vec<(NodeId, Vec<u8>)> = Vec::new();
        
        // Collect all nodes from all buckets
        for bucket in &current_node.buckets {
            for node_id in bucket {
                if let Some(node) = self.nodes.get(node_id) {
                    let distance = self.xor_distance(&node.hash, target_hash);
                    candidates.push((node_id.clone(), distance));
                }
            }
        }
        
        // Sort by distance
        candidates.sort_by(|a, b| a.1.cmp(&b.1));
        
        // Return k closest
        candidates.into_iter().take(k).map(|(id, _)| id).collect()
    }
}

impl DHTRouter for KademliaDHT {
    fn route(&self, source: &NodeId, destination: &NodeId, max_hops: u32) -> DHTRoutingResult {
        let dest_hash = self.hash_node_id(destination);
        
        let mut current_id = source.clone();
        let mut path = vec![current_id.clone()];
        let mut visited = HashSet::new();
        visited.insert(current_id.clone());
        let mut hops = 0;
        
        while hops < max_hops {
            // Check if we've reached the destination
            if &current_id == destination {
                return DHTRoutingResult {
                    success: true,
                    hops,
                    path,
                    failure_reason: None,
                };
            }
            
            // Find closest unvisited node to destination
            let closest_nodes = self.find_closest_nodes(&current_id, &dest_hash, self.k);
            
            let mut next_id = None;
            let mut best_distance = self.xor_distance(
                &self.nodes[&current_id].hash,
                &dest_hash
            );
            
            for candidate_id in closest_nodes {
                if visited.contains(&candidate_id) {
                    continue;
                }
                
                if let Some(candidate) = self.nodes.get(&candidate_id) {
                    let distance = self.xor_distance(&candidate.hash, &dest_hash);
                    if distance < best_distance {
                        best_distance = distance;
                        next_id = Some(candidate_id);
                    }
                }
            }
            
            match next_id {
                Some(id) => {
                    current_id = id;
                    path.push(current_id.clone());
                    visited.insert(current_id.clone());
                    hops += 1;
                }
                None => {
                    // No closer node found
                    return DHTRoutingResult {
                        success: false,
                        hops,
                        path,
                        failure_reason: Some("No closer node found".to_string()),
                    };
                }
            }
        }
        
        DHTRoutingResult {
            success: false,
            hops,
            path,
            failure_reason: Some("Max hops exceeded".to_string()),
        }
    }
    
    fn protocol_name(&self) -> &str {
        "Kademlia"
    }
    
    fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_chord_basic_routing() {
        let mut chord = ChordDHT::new(8); // 8-bit identifier space
        
        // Add some nodes
        for i in 0..10 {
            chord.add_node(NodeId::new(&format!("node{}", i)));
        }
        
        chord.build_finger_tables();
        
        let source = NodeId::new("node0");
        let dest = NodeId::new("node5");
        
        let result = chord.route(&source, &dest, 20);
        assert!(result.success);
        assert!(result.hops <= 8); // O(log N) hops
    }
    
    #[test]
    fn test_kademlia_basic_routing() {
        let mut kad = KademliaDHT::new(160, 20); // 160-bit IDs, k=20
        
        // Add some nodes
        for i in 0..10 {
            kad.add_node(NodeId::new(&format!("node{}", i)));
        }
        
        kad.build_routing_tables();
        
        let source = NodeId::new("node0");
        let dest = NodeId::new("node5");
        
        let result = kad.route(&source, &dest, 20);
        assert!(result.success);
        assert!(result.hops <= 10); // O(log N) hops
    }
}
