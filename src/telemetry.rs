//! OpenTelemetry Integration for DRFE-R
//!
//! Provides distributed tracing to visualize packet routing paths
//! in real-time across the network.

use std::collections::HashMap;
use std::sync::Arc;

use crate::coordinates::NodeId;
use crate::routing::RoutingMode;

/// Tracing context for a packet
#[derive(Debug, Clone)]
pub struct PacketTrace {
    /// Unique packet identifier
    pub packet_id: String,
    /// Source node
    pub source: NodeId,
    /// Destination node  
    pub destination: NodeId,
    /// Start timestamp (nanoseconds since epoch)
    pub start_time_ns: u64,
    /// List of hops with timing
    pub hops: Vec<HopTrace>,
    /// Whether packet was delivered
    pub delivered: bool,
    /// Failure reason if not delivered
    pub failure_reason: Option<String>,
}

/// Single hop in a packet trace
#[derive(Debug, Clone)]
pub struct HopTrace {
    /// Node processing this hop
    pub node_id: NodeId,
    /// Routing mode used
    pub mode: String,
    /// Time spent at this node (nanoseconds)
    pub duration_ns: u64,
    /// Hyperbolic distance to destination
    pub distance_to_dest: f64,
    /// Additional attributes
    pub attributes: HashMap<String, String>,
}

impl PacketTrace {
    pub fn new(packet_id: impl Into<String>, source: NodeId, destination: NodeId) -> Self {
        Self {
            packet_id: packet_id.into(),
            source,
            destination,
            start_time_ns: Self::now_ns(),
            hops: Vec::new(),
            delivered: false,
            failure_reason: None,
        }
    }

    fn now_ns() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }

    /// Add a hop to the trace
    pub fn add_hop(&mut self, node_id: NodeId, mode: &str, distance_to_dest: f64) {
        let now = Self::now_ns();
        let duration = if self.hops.is_empty() {
            now - self.start_time_ns
        } else {
            let last_time = self.hops.last()
                .map(|h| self.start_time_ns + h.duration_ns)
                .unwrap_or(self.start_time_ns);
            now - last_time
        };

        self.hops.push(HopTrace {
            node_id,
            mode: mode.to_string(),
            duration_ns: duration,
            distance_to_dest,
            attributes: HashMap::new(),
        });
    }

    /// Mark packet as delivered
    pub fn mark_delivered(&mut self) {
        self.delivered = true;
    }

    /// Mark packet as failed
    pub fn mark_failed(&mut self, reason: &str) {
        self.delivered = false;
        self.failure_reason = Some(reason.to_string());
    }

    /// Get total duration
    pub fn total_duration_ns(&self) -> u64 {
        Self::now_ns() - self.start_time_ns
    }

    /// Get hop count
    pub fn hop_count(&self) -> usize {
        self.hops.len()
    }

    /// Convert to OpenTelemetry span format (simplified)
    pub fn to_span_data(&self) -> SpanData {
        SpanData {
            trace_id: format!("trace-{}", self.packet_id),
            span_id: format!("span-{}", self.packet_id),
            name: format!("route {} -> {}", self.source.0, self.destination.0),
            start_time_ns: self.start_time_ns,
            end_time_ns: self.start_time_ns + self.total_duration_ns(),
            status: if self.delivered { "OK" } else { "ERROR" }.to_string(),
            attributes: vec![
                ("source".to_string(), self.source.0.clone()),
                ("destination".to_string(), self.destination.0.clone()),
                ("hop_count".to_string(), self.hop_count().to_string()),
                ("delivered".to_string(), self.delivered.to_string()),
            ],
            events: self.hops.iter().enumerate().map(|(i, hop)| SpanEvent {
                name: format!("hop_{}", i),
                timestamp_ns: self.start_time_ns + hop.duration_ns,
                attributes: vec![
                    ("node".to_string(), hop.node_id.0.clone()),
                    ("mode".to_string(), hop.mode.clone()),
                    ("distance".to_string(), format!("{:.4}", hop.distance_to_dest)),
                ],
            }).collect(),
        }
    }
}

/// Simplified span data for export
#[derive(Debug, Clone)]
pub struct SpanData {
    pub trace_id: String,
    pub span_id: String,
    pub name: String,
    pub start_time_ns: u64,
    pub end_time_ns: u64,
    pub status: String,
    pub attributes: Vec<(String, String)>,
    pub events: Vec<SpanEvent>,
}

/// Event within a span
#[derive(Debug, Clone)]
pub struct SpanEvent {
    pub name: String,
    pub timestamp_ns: u64,
    pub attributes: Vec<(String, String)>,
}

/// Telemetry collector for aggregating traces
pub struct TelemetryCollector {
    /// Active traces
    active_traces: dashmap::DashMap<String, PacketTrace>,
    /// Completed traces (ring buffer)
    completed_traces: std::sync::RwLock<Vec<PacketTrace>>,
    /// Max completed traces to keep
    max_completed: usize,
    /// Whether telemetry is enabled
    enabled: bool,
}

impl TelemetryCollector {
    pub fn new(max_completed: usize) -> Self {
        Self {
            active_traces: dashmap::DashMap::new(),
            completed_traces: std::sync::RwLock::new(Vec::new()),
            max_completed,
            enabled: true,
        }
    }

    /// Start tracing a packet
    pub fn start_trace(&self, packet_id: &str, source: NodeId, destination: NodeId) {
        if !self.enabled {
            return;
        }
        let trace = PacketTrace::new(packet_id, source, destination);
        self.active_traces.insert(packet_id.to_string(), trace);
    }

    /// Record a hop for a packet
    pub fn record_hop(&self, packet_id: &str, node_id: NodeId, mode: &str, distance: f64) {
        if !self.enabled {
            return;
        }
        if let Some(mut trace) = self.active_traces.get_mut(packet_id) {
            trace.add_hop(node_id, mode, distance);
        }
    }

    /// Complete a trace (delivered)
    pub fn complete_delivered(&self, packet_id: &str) {
        if !self.enabled {
            return;
        }
        if let Some((_, mut trace)) = self.active_traces.remove(packet_id) {
            trace.mark_delivered();
            self.store_completed(trace);
        }
    }

    /// Complete a trace (failed)
    pub fn complete_failed(&self, packet_id: &str, reason: &str) {
        if !self.enabled {
            return;
        }
        if let Some((_, mut trace)) = self.active_traces.remove(packet_id) {
            trace.mark_failed(reason);
            self.store_completed(trace);
        }
    }

    fn store_completed(&self, trace: PacketTrace) {
        if let Ok(mut completed) = self.completed_traces.write() {
            completed.push(trace);
            while completed.len() > self.max_completed {
                completed.remove(0);
            }
        }
    }

    /// Get recent completed traces
    pub fn get_recent_traces(&self, limit: usize) -> Vec<PacketTrace> {
        if let Ok(completed) = self.completed_traces.read() {
            completed.iter()
                .rev()
                .take(limit)
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Export traces as JSON for visualization
    pub fn export_json(&self, limit: usize) -> String {
        let traces = self.get_recent_traces(limit);
        serde_json::to_string_pretty(&traces.iter().map(|t| {
            serde_json::json!({
                "packet_id": t.packet_id,
                "source": t.source.0,
                "destination": t.destination.0,
                "delivered": t.delivered,
                "hop_count": t.hop_count(),
                "duration_ms": t.total_duration_ns() as f64 / 1_000_000.0,
                "hops": t.hops.iter().map(|h| {
                    serde_json::json!({
                        "node": h.node_id.0,
                        "mode": h.mode,
                        "distance": h.distance_to_dest,
                    })
                }).collect::<Vec<_>>()
            })
        }).collect::<Vec<_>>()).unwrap_or_default()
    }

    /// Get statistics
    pub fn get_stats(&self) -> TelemetryStats {
        let completed = self.completed_traces.read().ok();
        let total = completed.as_ref().map(|c| c.len()).unwrap_or(0);
        let delivered = completed.as_ref()
            .map(|c| c.iter().filter(|t| t.delivered).count())
            .unwrap_or(0);
        let avg_hops = completed.as_ref()
            .map(|c| {
                if c.is_empty() { 
                    0.0 
                } else {
                    c.iter().map(|t| t.hop_count()).sum::<usize>() as f64 / c.len() as f64
                }
            })
            .unwrap_or(0.0);

        TelemetryStats {
            active_traces: self.active_traces.len(),
            completed_traces: total,
            delivered_count: delivered,
            failed_count: total - delivered,
            avg_hop_count: avg_hops,
        }
    }

    /// Enable/disable telemetry
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

impl Default for TelemetryCollector {
    fn default() -> Self {
        Self::new(1000)
    }
}

/// Telemetry statistics
#[derive(Debug, Clone)]
pub struct TelemetryStats {
    pub active_traces: usize,
    pub completed_traces: usize,
    pub delivered_count: usize,
    pub failed_count: usize,
    pub avg_hop_count: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_trace() {
        let mut trace = PacketTrace::new("test-1", NodeId::new("a"), NodeId::new("z"));
        
        trace.add_hop(NodeId::new("a"), "Gravity", 5.0);
        trace.add_hop(NodeId::new("b"), "Gravity", 3.5);
        trace.add_hop(NodeId::new("c"), "Pressure", 2.0);
        trace.mark_delivered();
        
        assert_eq!(trace.hop_count(), 3);
        assert!(trace.delivered);
    }

    #[test]
    fn test_telemetry_collector() {
        let collector = TelemetryCollector::new(100);
        
        collector.start_trace("pkt-1", NodeId::new("src"), NodeId::new("dst"));
        collector.record_hop("pkt-1", NodeId::new("src"), "Gravity", 5.0);
        collector.record_hop("pkt-1", NodeId::new("mid"), "Gravity", 2.5);
        collector.complete_delivered("pkt-1");
        
        let stats = collector.get_stats();
        assert_eq!(stats.completed_traces, 1);
        assert_eq!(stats.delivered_count, 1);
    }
}
