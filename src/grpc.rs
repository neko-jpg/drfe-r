//! gRPC API for DRFE-R Distributed Nodes
//!
//! This module provides a gRPC API using tonic for high-performance
//! interaction with DRFE-R nodes. It exposes services for packet sending,
//! status queries, and streaming topology updates.

use crate::coordinates::NodeId;
use crate::network::DistributedNode;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, RwLock};
use tokio_stream::{wrappers::BroadcastStream, Stream, StreamExt};
use tonic::{transport::Server, Request, Response, Status};
use uuid::Uuid;

// Include generated protobuf code
pub mod proto {
    tonic::include_proto!("routing");
}

use proto::{
    routing_service_server::{RoutingService, RoutingServiceServer},
    GetNodeStatusRequest, HyperbolicPoint, NodeStatus, SendPacketRequest, SendPacketResponse,
    TopologyEdge, TopologyNode, TopologyRequest, TopologyUpdate, UpdateType,
};

/// Shared gRPC service state
#[derive(Clone)]
pub struct GrpcServiceState {
    /// The distributed node
    pub node: Arc<DistributedNode>,
    /// Packet tracking (packet_id -> status)
    pub packet_tracker: Arc<RwLock<HashMap<String, PacketStatusInfo>>>,
    /// Topology update broadcaster
    pub topology_tx: broadcast::Sender<TopologyUpdate>,
}

/// Internal packet status tracking
#[derive(Debug, Clone)]
pub struct PacketStatusInfo {
    pub id: String,
    pub source: String,
    pub destination: String,
    pub created_at: u64,
}

/// Implementation of the RoutingService gRPC service
pub struct GrpcRoutingService {
    state: GrpcServiceState,
}

impl GrpcRoutingService {
    /// Create a new gRPC routing service
    pub fn new(state: GrpcServiceState) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl RoutingService for GrpcRoutingService {
    /// Send a packet to a destination node
    async fn send_packet(
        &self,
        request: Request<SendPacketRequest>,
    ) -> Result<Response<SendPacketResponse>, Status> {
        let req = request.into_inner();

        // Validate destination
        if req.destination.is_empty() {
            return Err(Status::invalid_argument("Destination cannot be empty"));
        }

        // Validate TTL
        let ttl = if req.ttl == 0 { 64 } else { req.ttl };
        if ttl > 255 {
            return Err(Status::invalid_argument("TTL must be between 1 and 255"));
        }

        // Generate unique packet ID
        let packet_id = Uuid::new_v4().to_string();

        // Create packet status
        let status_info = PacketStatusInfo {
            id: packet_id.clone(),
            source: self.state.node.id().0.clone(),
            destination: req.destination.clone(),
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };

        // Store packet status
        {
            let mut tracker = self.state.packet_tracker.write().await;
            tracker.insert(packet_id.clone(), status_info);
        }

        // Send packet
        let dest_id = NodeId::new(&req.destination);
        match self.state.node.send_packet(dest_id, req.payload, ttl).await {
            Ok(_) => Ok(Response::new(SendPacketResponse {
                packet_id,
                status: "in_transit".to_string(),
                message: "Packet sent successfully".to_string(),
            })),
            Err(e) => Err(Status::internal(format!("Failed to send packet: {}", e))),
        }
    }

    /// Get the status of a specific node
    async fn get_node_status(
        &self,
        request: Request<GetNodeStatusRequest>,
    ) -> Result<Response<NodeStatus>, Status> {
        let req = request.into_inner();

        // Check if this is the local node
        if req.node_id == self.state.node.id().0 {
            let coord = self.state.node.coord().await;
            let neighbors = self.state.node.neighbors().await;
            let neighbor_ids: Vec<String> = neighbors.iter().map(|n| n.id.0.clone()).collect();

            // Calculate uptime (simplified - would need actual start time tracking)
            let uptime = 0; // TODO: Track actual uptime

            Ok(Response::new(NodeStatus {
                node_id: self.state.node.id().0.clone(),
                coordinate: Some(HyperbolicPoint {
                    x: coord.point.x,
                    y: coord.point.y,
                    norm: coord.point.euclidean_norm(),
                    version: coord.updated_at,
                }),
                neighbors: neighbor_ids,
                uptime,
                udp_address: self.state.node.local_udp_addr().to_string(),
                tcp_address: self.state.node.local_tcp_addr().to_string(),
            }))
        } else {
            // Check if it's a neighbor
            let neighbor_id = NodeId::new(&req.node_id);
            if let Some(neighbor) = self.state.node.get_neighbor(&neighbor_id).await {
                Ok(Response::new(NodeStatus {
                    node_id: neighbor.id.0.clone(),
                    coordinate: Some(HyperbolicPoint {
                        x: neighbor.coord.x,
                        y: neighbor.coord.y,
                        norm: neighbor.coord.euclidean_norm(),
                        version: neighbor.version,
                    }),
                    neighbors: vec![], // We don't know neighbor's neighbors
                    uptime: 0,
                    udp_address: neighbor.addr.to_string(),
                    tcp_address: "unknown".to_string(),
                }))
            } else {
                Err(Status::not_found(format!("Node {} not found", req.node_id)))
            }
        }
    }

    /// Stream topology updates in real-time
    async fn stream_topology(
        &self,
        request: Request<TopologyRequest>,
    ) -> Result<Response<Self::StreamTopologyStream>, Status> {
        let req = request.into_inner();

        // Subscribe to topology updates
        let rx = self.state.topology_tx.subscribe();
        let stream = BroadcastStream::new(rx);

        let output_stream = stream.filter_map(|result| match result {
            Ok(update) => Some(Ok(update)),
            Err(_) => None, // Skip lagged messages
        });

        // Create initial snapshot if requested
        if req.include_snapshot {
            let initial_update = self.create_topology_snapshot().await;
            let combined_stream = tokio_stream::once(Ok(initial_update)).chain(output_stream);
            Ok(Response::new(
                Box::pin(combined_stream) as Self::StreamTopologyStream
            ))
        } else {
            Ok(Response::new(Box::pin(output_stream) as Self::StreamTopologyStream))
        }
    }

    type StreamTopologyStream =
        Pin<Box<dyn Stream<Item = Result<TopologyUpdate, Status>> + Send>>;
}

impl GrpcRoutingService {
    /// Create a topology snapshot
    async fn create_topology_snapshot(&self) -> TopologyUpdate {
        let local_id = self.state.node.id().0.clone();
        let local_coord = self.state.node.coord().await;
        let neighbors = self.state.node.neighbors().await;

        // Build nodes list
        let mut nodes = vec![TopologyNode {
            id: local_id.clone(),
            coordinate: Some(HyperbolicPoint {
                x: local_coord.point.x,
                y: local_coord.point.y,
                norm: local_coord.point.euclidean_norm(),
                version: local_coord.updated_at,
            }),
            is_local: true,
        }];

        for neighbor in &neighbors {
            nodes.push(TopologyNode {
                id: neighbor.id.0.clone(),
                coordinate: Some(HyperbolicPoint {
                    x: neighbor.coord.x,
                    y: neighbor.coord.y,
                    norm: neighbor.coord.euclidean_norm(),
                    version: neighbor.version,
                }),
                is_local: false,
            });
        }

        // Build edges list
        let edges: Vec<TopologyEdge> = neighbors
            .iter()
            .map(|n| {
                let distance = local_coord.point.hyperbolic_distance(&n.coord);
                TopologyEdge {
                    source: local_id.clone(),
                    target: n.id.0.clone(),
                    distance,
                }
            })
            .collect();

        TopologyUpdate {
            update_type: UpdateType::Snapshot as i32,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            nodes,
            edges,
        }
    }
}

/// Start the gRPC server
///
/// # Arguments
/// * `node` - The distributed node to expose via gRPC
/// * `bind_addr` - Address to bind the gRPC server (e.g., "0.0.0.0:50051")
///
/// # Returns
/// Result indicating success or error
pub async fn start_grpc_server(
    node: Arc<DistributedNode>,
    bind_addr: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create topology update channel
    let (topology_tx, _) = broadcast::channel(100);

    // Create shared state
    let state = GrpcServiceState {
        node,
        packet_tracker: Arc::new(RwLock::new(HashMap::new())),
        topology_tx,
    };

    // Create service
    let service = GrpcRoutingService::new(state);

    // Parse bind address
    let addr: SocketAddr = bind_addr.parse()?;

    tracing::info!("Starting gRPC server on {}", addr);

    // Start server
    Server::builder()
        .add_service(RoutingServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::DistributedNode;

    async fn create_test_state() -> GrpcServiceState {
        let node = Arc::new(
            DistributedNode::new(NodeId::new("test_node"), "127.0.0.1:0", "127.0.0.1:0")
                .await
                .unwrap(),
        );

        let (topology_tx, _) = broadcast::channel(100);

        GrpcServiceState {
            node,
            packet_tracker: Arc::new(RwLock::new(HashMap::new())),
            topology_tx,
        }
    }

    #[tokio::test]
    async fn test_send_packet_validation() {
        let state = create_test_state().await;
        let service = GrpcRoutingService::new(state);

        // Test empty destination
        let request = Request::new(SendPacketRequest {
            destination: "".to_string(),
            payload: b"test".to_vec(),
            ttl: 64,
        });

        let result = service.send_packet(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_send_packet_ttl_validation() {
        let state = create_test_state().await;
        let service = GrpcRoutingService::new(state);

        // Test TTL > 255
        let request = Request::new(SendPacketRequest {
            destination: "node2".to_string(),
            payload: b"test".to_vec(),
            ttl: 256,
        });

        let result = service.send_packet(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_get_node_status_local() {
        let state = create_test_state().await;
        let node_id = state.node.id().0.clone();
        let service = GrpcRoutingService::new(state);

        let request = Request::new(GetNodeStatusRequest { node_id });

        let result = service.get_node_status(request).await;
        assert!(result.is_ok());

        let status = result.unwrap().into_inner();
        assert_eq!(status.node_id, "test_node");
        assert!(status.coordinate.is_some());
        let coord = status.coordinate.unwrap();
        assert!(coord.norm < 1.0); // Should be in Poincaré disk
    }

    #[tokio::test]
    async fn test_get_node_status_not_found() {
        let state = create_test_state().await;
        let service = GrpcRoutingService::new(state);

        let request = Request::new(GetNodeStatusRequest {
            node_id: "nonexistent".to_string(),
        });

        let result = service.get_node_status(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn test_create_topology_snapshot() {
        let state = create_test_state().await;
        let service = GrpcRoutingService::new(state);

        let snapshot = service.create_topology_snapshot().await;

        assert_eq!(snapshot.update_type, UpdateType::Snapshot as i32);
        assert_eq!(snapshot.nodes.len(), 1); // Only local node
        assert_eq!(snapshot.edges.len(), 0); // No edges
        assert!(snapshot.nodes[0].is_local);
    }
}
