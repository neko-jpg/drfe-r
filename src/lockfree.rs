//! Lock-Free Concurrent Data Structures for DRFE-R
//!
//! Replaces Arc<RwLock<...>> with lock-free alternatives for
//! maximum CPU parallelism at 100K+ node scale.

use crossbeam::queue::SegQueue;
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use crate::coordinates::{NodeId, RoutingCoordinate};
use crate::routing::PacketHeader;
use crate::PoincareDiskPoint;

/// Lock-free node storage using DashMap
pub struct LockFreeNodeStore {
    nodes: DashMap<NodeId, LockFreeNode>,
    node_count: AtomicUsize,
}

/// Node data optimized for concurrent access
#[derive(Debug, Clone)]
pub struct LockFreeNode {
    pub id: NodeId,
    pub coord: RoutingCoordinate,
    pub neighbors: Vec<NodeId>,
    pub version: u64,
}

impl LockFreeNodeStore {
    pub fn new() -> Self {
        Self {
            nodes: DashMap::new(),
            node_count: AtomicUsize::new(0),
        }
    }

    /// Insert or update a node
    pub fn upsert(&self, node: LockFreeNode) {
        let existed = self.nodes.insert(node.id.clone(), node).is_some();
        if !existed {
            self.node_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Get a node (returns clone for safety)
    pub fn get(&self, id: &NodeId) -> Option<LockFreeNode> {
        self.nodes.get(id).map(|n| n.clone())
    }

    /// Get node count
    pub fn count(&self) -> usize {
        self.node_count.load(Ordering::Relaxed)
    }

    /// Check if node exists
    pub fn contains(&self, id: &NodeId) -> bool {
        self.nodes.contains_key(id)
    }

    /// Update node coordinates atomically
    pub fn update_coord(&self, id: &NodeId, new_coord: PoincareDiskPoint) -> bool {
        if let Some(mut entry) = self.nodes.get_mut(id) {
            entry.coord.point = new_coord;
            entry.version += 1;
            true
        } else {
            false
        }
    }

    /// Get all node IDs
    pub fn all_ids(&self) -> Vec<NodeId> {
        self.nodes.iter().map(|entry| entry.key().clone()).collect()
    }

    /// Remove a node
    pub fn remove(&self, id: &NodeId) -> Option<LockFreeNode> {
        self.nodes.remove(id).map(|(_, n)| {
            self.node_count.fetch_sub(1, Ordering::Relaxed);
            n
        })
    }
}

impl Default for LockFreeNodeStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Lock-free message queue for packet routing
pub struct MessageQueue {
    queue: SegQueue<RoutingMessage>,
    message_count: AtomicU64,
}

/// Message types for routing
#[derive(Debug, Clone)]
pub enum RoutingMessage {
    /// Forward packet to next hop
    Forward {
        packet_id: u64,
        from: NodeId,
        to: NodeId,
        header: PacketHeader,
    },
    /// Coordinate update broadcast
    CoordinateUpdate {
        node_id: NodeId,
        new_coord: PoincareDiskPoint,
        version: u64,
    },
    /// Heartbeat for liveness
    Heartbeat {
        from: NodeId,
        timestamp: u64,
    },
    /// Node join announcement
    NodeJoin {
        node: LockFreeNode,
    },
    /// Node leave announcement
    NodeLeave {
        node_id: NodeId,
    },
}

impl MessageQueue {
    pub fn new() -> Self {
        Self {
            queue: SegQueue::new(),
            message_count: AtomicU64::new(0),
        }
    }

    /// Push a message to the queue
    pub fn push(&self, msg: RoutingMessage) {
        self.queue.push(msg);
        self.message_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Pop a message from the queue
    pub fn pop(&self) -> Option<RoutingMessage> {
        self.queue.pop()
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Get total messages ever enqueued
    pub fn total_messages(&self) -> u64 {
        self.message_count.load(Ordering::Relaxed)
    }
}

impl Default for MessageQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-node message queues for actor-style message passing
pub struct NodeMailboxes {
    mailboxes: DashMap<NodeId, Arc<MessageQueue>>,
}

impl NodeMailboxes {
    pub fn new() -> Self {
        Self {
            mailboxes: DashMap::new(),
        }
    }

    /// Get or create mailbox for a node
    pub fn get_or_create(&self, node_id: &NodeId) -> Arc<MessageQueue> {
        self.mailboxes
            .entry(node_id.clone())
            .or_insert_with(|| Arc::new(MessageQueue::new()))
            .clone()
    }

    /// Send message to a node
    pub fn send(&self, to: &NodeId, msg: RoutingMessage) {
        let mailbox = self.get_or_create(to);
        mailbox.push(msg);
    }

    /// Remove mailbox when node leaves
    pub fn remove(&self, node_id: &NodeId) {
        self.mailboxes.remove(node_id);
    }
}

impl Default for NodeMailboxes {
    fn default() -> Self {
        Self::new()
    }
}

/// Lock-free routing statistics
pub struct RoutingStats {
    packets_routed: AtomicU64,
    packets_delivered: AtomicU64,
    packets_failed: AtomicU64,
    total_hops: AtomicU64,
    gravity_hops: AtomicU64,
    pressure_hops: AtomicU64,
    tree_hops: AtomicU64,
}

impl RoutingStats {
    pub fn new() -> Self {
        Self {
            packets_routed: AtomicU64::new(0),
            packets_delivered: AtomicU64::new(0),
            packets_failed: AtomicU64::new(0),
            total_hops: AtomicU64::new(0),
            gravity_hops: AtomicU64::new(0),
            pressure_hops: AtomicU64::new(0),
            tree_hops: AtomicU64::new(0),
        }
    }

    pub fn record_route(&self) {
        self.packets_routed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_delivery(&self, hops: u64) {
        self.packets_delivered.fetch_add(1, Ordering::Relaxed);
        self.total_hops.fetch_add(hops, Ordering::Relaxed);
    }

    pub fn record_failure(&self) {
        self.packets_failed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_hop(&self, mode: &str) {
        match mode {
            "Gravity" => self.gravity_hops.fetch_add(1, Ordering::Relaxed),
            "Pressure" => self.pressure_hops.fetch_add(1, Ordering::Relaxed),
            "Tree" => self.tree_hops.fetch_add(1, Ordering::Relaxed),
            _ => 0,
        };
    }

    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            packets_routed: self.packets_routed.load(Ordering::Relaxed),
            packets_delivered: self.packets_delivered.load(Ordering::Relaxed),
            packets_failed: self.packets_failed.load(Ordering::Relaxed),
            total_hops: self.total_hops.load(Ordering::Relaxed),
            gravity_hops: self.gravity_hops.load(Ordering::Relaxed),
            pressure_hops: self.pressure_hops.load(Ordering::Relaxed),
            tree_hops: self.tree_hops.load(Ordering::Relaxed),
        }
    }
}

impl Default for RoutingStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of routing statistics
#[derive(Debug, Clone)]
pub struct StatsSnapshot {
    pub packets_routed: u64,
    pub packets_delivered: u64,
    pub packets_failed: u64,
    pub total_hops: u64,
    pub gravity_hops: u64,
    pub pressure_hops: u64,
    pub tree_hops: u64,
}

impl StatsSnapshot {
    pub fn success_rate(&self) -> f64 {
        if self.packets_routed == 0 {
            0.0
        } else {
            self.packets_delivered as f64 / self.packets_routed as f64
        }
    }

    pub fn average_hops(&self) -> f64 {
        if self.packets_delivered == 0 {
            0.0
        } else {
            self.total_hops as f64 / self.packets_delivered as f64
        }
    }
}

/// Lock-free concurrent router using actor model
pub struct ConcurrentRouter {
    pub nodes: Arc<LockFreeNodeStore>,
    pub mailboxes: Arc<NodeMailboxes>,
    pub stats: Arc<RoutingStats>,
    pub global_queue: Arc<MessageQueue>,
}

impl ConcurrentRouter {
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(LockFreeNodeStore::new()),
            mailboxes: Arc::new(NodeMailboxes::new()),
            stats: Arc::new(RoutingStats::new()),
            global_queue: Arc::new(MessageQueue::new()),
        }
    }

    /// Add a node to the router
    pub fn add_node(&self, node: LockFreeNode) {
        self.nodes.upsert(node.clone());
        self.global_queue.push(RoutingMessage::NodeJoin { node });
    }

    /// Remove a node from the router
    pub fn remove_node(&self, node_id: &NodeId) {
        if let Some(_) = self.nodes.remove(node_id) {
            self.mailboxes.remove(node_id);
            self.global_queue.push(RoutingMessage::NodeLeave {
                node_id: node_id.clone(),
            });
        }
    }

    /// Update node coordinates
    pub fn update_coordinates(&self, node_id: &NodeId, new_coord: PoincareDiskPoint, version: u64) {
        if self.nodes.update_coord(node_id, new_coord) {
            // Broadcast update to neighbors
            if let Some(node) = self.nodes.get(node_id) {
                for neighbor_id in &node.neighbors {
                    self.mailboxes.send(
                        neighbor_id,
                        RoutingMessage::CoordinateUpdate {
                            node_id: node_id.clone(),
                            new_coord,
                            version,
                        },
                    );
                }
            }
        }
    }

    /// Get router statistics
    pub fn get_stats(&self) -> StatsSnapshot {
        self.stats.snapshot()
    }

    /// Get node count
    pub fn node_count(&self) -> usize {
        self.nodes.count()
    }
}

impl Default for ConcurrentRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lockfree_node_store() {
        let store = LockFreeNodeStore::new();
        
        let node = LockFreeNode {
            id: NodeId::new("test"),
            coord: RoutingCoordinate::new(PoincareDiskPoint::new(0.0, 0.0).unwrap(), 0),
            neighbors: vec![],
            version: 0,
        };
        
        store.upsert(node);
        assert_eq!(store.count(), 1);
        assert!(store.contains(&NodeId::new("test")));
    }

    #[test]
    fn test_message_queue() {
        let queue = MessageQueue::new();
        
        queue.push(RoutingMessage::Heartbeat {
            from: NodeId::new("node1"),
            timestamp: 12345,
        });
        
        assert!(!queue.is_empty());
        assert!(queue.pop().is_some());
        assert!(queue.is_empty());
    }

    #[test]
    fn test_routing_stats() {
        let stats = RoutingStats::new();
        
        stats.record_route();
        stats.record_route();
        stats.record_delivery(5);
        stats.record_failure();
        
        let snapshot = stats.snapshot();
        assert_eq!(snapshot.packets_routed, 2);
        assert_eq!(snapshot.packets_delivered, 1);
        assert_eq!(snapshot.packets_failed, 1);
        assert_eq!(snapshot.success_rate(), 0.5);
    }
}
