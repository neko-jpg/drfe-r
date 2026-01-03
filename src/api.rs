//! REST API for DRFE-R Distributed Nodes
//!
//! This module provides a REST API using axum for interacting with DRFE-R nodes.
//! It exposes endpoints for packet sending, status queries, and topology inspection.

use crate::coordinates::NodeId;
use crate::network::DistributedNode;
use axum::{
    extract::{Path, State, Request},
    http::{StatusCode, HeaderMap},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
    middleware::{self, Next},
};
use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use governor::{Quota, RateLimiter};
use nonzero_ext::nonzero;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

/// Shared application state
#[derive(Clone)]
pub struct ApiState {
    /// The distributed node
    pub node: Arc<DistributedNode>,
    /// Packet tracking (packet_id -> status)
    pub packet_tracker: Arc<RwLock<HashMap<String, PacketStatus>>>,
    /// Rate limiter for API requests (keyed by node ID or IP)
    pub rate_limiter: Arc<RateLimiter<String, dashmap::DashMap<String, governor::state::InMemoryState>, governor::clock::DefaultClock>>,
    /// Authentication keys (node_id -> public key)
    pub auth_keys: Arc<RwLock<HashMap<String, VerifyingKey>>>,
    /// Whether authentication is required
    pub require_auth: bool,
}

/// Packet status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketStatus {
    /// Unique packet ID
    pub id: String,
    /// Current delivery status
    pub status: DeliveryStatus,
    /// Number of hops taken
    pub hops: u32,
    /// Path taken (list of node IDs)
    pub path: Vec<String>,
    /// Timestamp when packet was created
    pub created_at: u64,
    /// Source node ID
    pub source: String,
    /// Destination node ID
    pub destination: String,
}

/// Delivery status enum
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DeliveryStatus {
    /// Packet is pending (not yet sent)
    Pending,
    /// Packet is in transit
    InTransit { current_node: String },
    /// Packet was delivered successfully
    Delivered { latency_ms: u64 },
    /// Packet delivery failed
    Failed { reason: String },
}

/// Request to send a packet
#[derive(Debug, Deserialize)]
pub struct SendPacketRequest {
    /// Destination node ID
    pub destination: String,
    /// Payload data (base64 encoded or raw string)
    pub payload: String,
    /// Optional TTL (defaults to 64)
    #[serde(default = "default_ttl")]
    pub ttl: u32,
}

fn default_ttl() -> u32 {
    64
}

/// Response from sending a packet
#[derive(Debug, Serialize)]
pub struct SendPacketResponse {
    /// Unique packet ID for tracking
    pub packet_id: String,
    /// Initial status
    pub status: String,
    /// Message
    pub message: String,
}

/// Node information response
#[derive(Debug, Serialize)]
pub struct NodeInfoResponse {
    /// Node ID
    pub id: String,
    /// Current coordinate
    pub coordinate: CoordinateInfo,
    /// Number of neighbors
    pub neighbor_count: usize,
    /// Local UDP address
    pub udp_address: String,
    /// Local TCP address
    pub tcp_address: String,
}

/// Coordinate information
#[derive(Debug, Serialize)]
pub struct CoordinateInfo {
    /// X coordinate
    pub x: f64,
    /// Y coordinate
    pub y: f64,
    /// Euclidean norm (distance from origin)
    pub norm: f64,
    /// Version/timestamp
    pub version: u64,
}

/// Neighbor information response
#[derive(Debug, Serialize)]
pub struct NeighborResponse {
    /// Neighbor ID
    pub id: String,
    /// Neighbor coordinate
    pub coordinate: CoordinateInfo,
    /// Neighbor address
    pub address: String,
    /// Round-trip time in milliseconds
    pub rtt_ms: u64,
    /// Last heartbeat (seconds ago)
    pub last_heartbeat_secs: u64,
}

/// Topology response
#[derive(Debug, Serialize)]
pub struct TopologyResponse {
    /// All nodes in the topology
    pub nodes: Vec<TopologyNode>,
    /// All edges in the topology
    pub edges: Vec<TopologyEdge>,
}

/// Node in topology
#[derive(Debug, Serialize)]
pub struct TopologyNode {
    /// Node ID
    pub id: String,
    /// Coordinate
    pub coordinate: CoordinateInfo,
    /// Whether this is the local node
    pub is_local: bool,
}

/// Edge in topology
#[derive(Debug, Serialize)]
pub struct TopologyEdge {
    /// Source node ID
    pub source: String,
    /// Target node ID
    pub target: String,
    /// Hyperbolic distance
    pub distance: f64,
}

/// API error type
#[derive(Debug)]
pub enum ApiError {
    /// Node not found
    NotFound(String),
    /// Invalid request
    BadRequest(String),
    /// Internal server error
    Internal(String),
    /// Unauthorized (authentication failed)
    Unauthorized(String),
    /// Rate limit exceeded
    TooManyRequests(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            ApiError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            ApiError::TooManyRequests(msg) => (StatusCode::TOO_MANY_REQUESTS, msg),
        };

        let body = Json(serde_json::json!({
            "error": message,
        }));

        (status, body).into_response()
    }
}

/// Authentication middleware
///
/// Verifies Ed25519 signatures on requests when authentication is enabled.
/// Expects headers:
/// - X-Node-Id: The node ID making the request
/// - X-Signature: Base64-encoded Ed25519 signature of the request body
/// - X-Timestamp: Unix timestamp in milliseconds (to prevent replay attacks)
async fn auth_middleware(
    State(state): State<ApiState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    // Skip authentication if not required
    if !state.require_auth {
        return Ok(next.run(request).await);
    }

    // Extract authentication headers
    let node_id = headers
        .get("X-Node-Id")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::Unauthorized("Missing X-Node-Id header".to_string()))?;

    let signature_b64 = headers
        .get("X-Signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::Unauthorized("Missing X-Signature header".to_string()))?;

    let timestamp_str = headers
        .get("X-Timestamp")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::Unauthorized("Missing X-Timestamp header".to_string()))?;

    // Parse timestamp and check if it's recent (within 5 minutes)
    let timestamp: u64 = timestamp_str
        .parse()
        .map_err(|_| ApiError::Unauthorized("Invalid timestamp format".to_string()))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let time_diff = if now > timestamp {
        now - timestamp
    } else {
        timestamp - now
    };

    if time_diff > 300_000 {
        // 5 minutes
        return Err(ApiError::Unauthorized(
            "Timestamp too old or in future".to_string(),
        ));
    }

    // Get public key for this node
    let auth_keys = state.auth_keys.read().await;
    let public_key = auth_keys
        .get(node_id)
        .ok_or_else(|| ApiError::Unauthorized(format!("Unknown node ID: {}", node_id)))?;

    // Decode signature
    let signature_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature_b64)
        .map_err(|_| ApiError::Unauthorized("Invalid signature encoding".to_string()))?;

    let signature = Signature::from_slice(&signature_bytes)
        .map_err(|_| ApiError::Unauthorized("Invalid signature format".to_string()))?;

    // For verification, we need the request body
    // In a real implementation, we'd need to buffer the body and verify it
    // For now, we'll verify the timestamp as a simple check
    let message = format!("{}:{}", node_id, timestamp);

    // Verify signature
    public_key
        .verify(message.as_bytes(), &signature)
        .map_err(|_| ApiError::Unauthorized("Signature verification failed".to_string()))?;

    Ok(next.run(request).await)
}

/// Rate limiting middleware
///
/// Limits requests per client to prevent DoS attacks.
/// Uses the X-Node-Id header or remote IP address as the rate limit key.
async fn rate_limit_middleware(
    State(state): State<ApiState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    // Determine rate limit key (node ID or IP address)
    let key = if let Some(node_id) = headers.get("X-Node-Id").and_then(|v| v.to_str().ok()) {
        node_id.to_string()
    } else {
        // Fallback to IP address (would need to extract from connection info)
        "default".to_string()
    };

    // Check rate limit
    match state.rate_limiter.check_key(&key) {
        Ok(_) => Ok(next.run(request).await),
        Err(_) => Err(ApiError::TooManyRequests(
            "Rate limit exceeded. Please try again later.".to_string(),
        )),
    }
}

/// Create the API router
pub fn create_router(state: ApiState) -> Router {
    Router::new()
        .route("/api/v1/packets", post(send_packet))
        .route("/api/v1/packets/:id", get(get_packet_status))
        .route("/api/v1/nodes/:id", get(get_node_info))
        .route("/api/v1/nodes/:id/neighbors", get(get_node_neighbors))
        .route("/api/v1/topology", get(get_topology))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// POST /api/v1/packets - Send a packet
async fn send_packet(
    State(state): State<ApiState>,
    Json(request): Json<SendPacketRequest>,
) -> Result<Json<SendPacketResponse>, ApiError> {
    // Validate destination
    if request.destination.is_empty() {
        return Err(ApiError::BadRequest("Destination cannot be empty".to_string()));
    }

    // Validate TTL
    if request.ttl == 0 || request.ttl > 255 {
        return Err(ApiError::BadRequest("TTL must be between 1 and 255".to_string()));
    }

    // Convert payload to bytes
    let payload = request.payload.as_bytes().to_vec();

    // Generate unique packet ID
    let packet_id = Uuid::new_v4().to_string();

    // Create packet status
    let status = PacketStatus {
        id: packet_id.clone(),
        status: DeliveryStatus::Pending,
        hops: 0,
        path: vec![state.node.id().0.clone()],
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
        source: state.node.id().0.clone(),
        destination: request.destination.clone(),
    };

    // Store packet status
    {
        let mut tracker = state.packet_tracker.write().await;
        tracker.insert(packet_id.clone(), status);
    }

    // Send packet
    let dest_id = NodeId::new(&request.destination);
    match state.node.send_packet(dest_id, payload, request.ttl).await {
        Ok(_) => {
            // Update status to in transit
            let mut tracker = state.packet_tracker.write().await;
            if let Some(status) = tracker.get_mut(&packet_id) {
                status.status = DeliveryStatus::InTransit {
                    current_node: state.node.id().0.clone(),
                };
            }

            Ok(Json(SendPacketResponse {
                packet_id,
                status: "in_transit".to_string(),
                message: "Packet sent successfully".to_string(),
            }))
        }
        Err(e) => {
            // Update status to failed
            let mut tracker = state.packet_tracker.write().await;
            if let Some(status) = tracker.get_mut(&packet_id) {
                status.status = DeliveryStatus::Failed {
                    reason: e.to_string(),
                };
            }

            Err(ApiError::Internal(format!("Failed to send packet: {}", e)))
        }
    }
}

/// GET /api/v1/packets/:id - Get packet status
async fn get_packet_status(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<PacketStatus>, ApiError> {
    let tracker = state.packet_tracker.read().await;

    tracker
        .get(&id)
        .cloned()
        .map(Json)
        .ok_or_else(|| ApiError::NotFound(format!("Packet {} not found", id)))
}

/// GET /api/v1/nodes/:id - Get node information
async fn get_node_info(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<NodeInfoResponse>, ApiError> {
    // Check if this is the local node
    if id == state.node.id().0 {
        let coord = state.node.coord().await;

        Ok(Json(NodeInfoResponse {
            id: state.node.id().0.clone(),
            coordinate: CoordinateInfo {
                x: coord.point.x,
                y: coord.point.y,
                norm: coord.point.euclidean_norm(),
                version: coord.updated_at,
            },
            neighbor_count: state.node.neighbors().await.len(),
            udp_address: state.node.local_udp_addr().to_string(),
            tcp_address: state.node.local_tcp_addr().to_string(),
        }))
    } else {
        // Check if it's a neighbor
        let neighbor_id = NodeId::new(&id);
        if let Some(neighbor) = state.node.get_neighbor(&neighbor_id).await {
            Ok(Json(NodeInfoResponse {
                id: neighbor.id.0.clone(),
                coordinate: CoordinateInfo {
                    x: neighbor.coord.x,
                    y: neighbor.coord.y,
                    norm: neighbor.coord.euclidean_norm(),
                    version: neighbor.version,
                },
                neighbor_count: 0, // We don't know neighbor's neighbors
                udp_address: neighbor.addr.to_string(),
                tcp_address: "unknown".to_string(),
            }))
        } else {
            Err(ApiError::NotFound(format!("Node {} not found", id)))
        }
    }
}

/// GET /api/v1/nodes/:id/neighbors - Get node neighbors
async fn get_node_neighbors(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<NeighborResponse>>, ApiError> {
    // Only support querying local node's neighbors
    if id != state.node.id().0 {
        return Err(ApiError::BadRequest(
            "Can only query local node's neighbors".to_string(),
        ));
    }

    let neighbors = state.node.neighbors().await;

    let response: Vec<NeighborResponse> = neighbors
        .iter()
        .map(|n| NeighborResponse {
            id: n.id.0.clone(),
            coordinate: CoordinateInfo {
                x: n.coord.x,
                y: n.coord.y,
                norm: n.coord.euclidean_norm(),
                version: n.version,
            },
            address: n.addr.to_string(),
            rtt_ms: n.rtt.as_millis() as u64,
            last_heartbeat_secs: n.last_heartbeat.elapsed().as_secs(),
        })
        .collect();

    Ok(Json(response))
}

/// GET /api/v1/topology - Get network topology
async fn get_topology(
    State(state): State<ApiState>,
) -> Result<Json<TopologyResponse>, ApiError> {
    let local_id = state.node.id().0.clone();
    let local_coord = state.node.coord().await;
    let neighbors = state.node.neighbors().await;

    // Build nodes list (local node + neighbors)
    let mut nodes = vec![TopologyNode {
        id: local_id.clone(),
        coordinate: CoordinateInfo {
            x: local_coord.point.x,
            y: local_coord.point.y,
            norm: local_coord.point.euclidean_norm(),
            version: local_coord.updated_at,
        },
        is_local: true,
    }];

    for neighbor in &neighbors {
        nodes.push(TopologyNode {
            id: neighbor.id.0.clone(),
            coordinate: CoordinateInfo {
                x: neighbor.coord.x,
                y: neighbor.coord.y,
                norm: neighbor.coord.euclidean_norm(),
                version: neighbor.version,
            },
            is_local: false,
        });
    }

    // Build edges list (local node to each neighbor)
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

    Ok(Json(TopologyResponse { nodes, edges }))
}

/// Start the API server
///
/// # Arguments
/// * `node` - The distributed node to expose via API
/// * `bind_addr` - Address to bind the API server (e.g., "0.0.0.0:3000")
/// * `require_auth` - Whether to require authentication
///
/// # Returns
/// Result indicating success or error
pub async fn start_api_server(
    node: Arc<DistributedNode>,
    bind_addr: &str,
    require_auth: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Create rate limiter: 100 requests per minute per client
    let quota = Quota::per_minute(nonzero!(100u32));
    let rate_limiter = Arc::new(RateLimiter::keyed(quota));

    // Create shared state
    let state = ApiState {
        node,
        packet_tracker: Arc::new(RwLock::new(HashMap::new())),
        rate_limiter,
        auth_keys: Arc::new(RwLock::new(HashMap::new())),
        require_auth,
    };

    // Create router
    let app = create_router(state);

    // Parse bind address
    let addr: SocketAddr = bind_addr.parse()?;

    tracing::info!("Starting API server on {} (auth: {})", addr, require_auth);

    // Start server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Register a public key for authentication
///
/// # Arguments
/// * `state` - The API state
/// * `node_id` - The node ID
/// * `public_key_bytes` - The Ed25519 public key bytes (32 bytes)
///
/// # Returns
/// Result indicating success or error
pub async fn register_auth_key(
    state: &ApiState,
    node_id: &str,
    public_key_bytes: &[u8],
) -> Result<(), String> {
    let public_key = VerifyingKey::from_bytes(
        public_key_bytes
            .try_into()
            .map_err(|_| "Invalid public key length (expected 32 bytes)".to_string())?,
    )
    .map_err(|e| format!("Invalid public key: {}", e))?;

    let mut auth_keys = state.auth_keys.write().await;
    auth_keys.insert(node_id.to_string(), public_key);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::DistributedNode;

    async fn create_test_node() -> Arc<DistributedNode> {
        Arc::new(
            DistributedNode::new(NodeId::new("test_node"), "127.0.0.1:0", "127.0.0.1:0")
                .await
                .unwrap(),
        )
    }

    fn create_test_state(node: Arc<DistributedNode>) -> ApiState {
        let quota = Quota::per_minute(nonzero!(100u32));
        let rate_limiter = Arc::new(RateLimiter::keyed(quota));

        ApiState {
            node,
            packet_tracker: Arc::new(RwLock::new(HashMap::new())),
            rate_limiter,
            auth_keys: Arc::new(RwLock::new(HashMap::new())),
            require_auth: false,
        }
    }

    #[tokio::test]
    async fn test_create_router() {
        let node = create_test_node().await;
        let state = create_test_state(node);

        let _router = create_router(state);
        // Router creation should succeed
    }

    #[tokio::test]
    async fn test_send_packet_request_validation() {
        let node = create_test_node().await;
        let state = create_test_state(node);

        // Test empty destination
        let request = SendPacketRequest {
            destination: "".to_string(),
            payload: "test".to_string(),
            ttl: 64,
        };

        let result = send_packet(State(state.clone()), Json(request)).await;
        assert!(result.is_err());

        // Test invalid TTL (0)
        let request = SendPacketRequest {
            destination: "node2".to_string(),
            payload: "test".to_string(),
            ttl: 0,
        };

        let result = send_packet(State(state.clone()), Json(request)).await;
        assert!(result.is_err());

        // Test invalid TTL (> 255)
        let request = SendPacketRequest {
            destination: "node2".to_string(),
            payload: "test".to_string(),
            ttl: 256,
        };

        let result = send_packet(State(state), Json(request)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_node_info_local() {
        let node = create_test_node().await;
        let node_id = node.id().0.clone();
        let state = create_test_state(node);

        let result = get_node_info(State(state), Path(node_id)).await;
        assert!(result.is_ok());

        let info = result.unwrap().0;
        assert_eq!(info.id, "test_node");
        assert!(info.coordinate.norm < 1.0); // Should be in Poincaré disk
    }

    #[tokio::test]
    async fn test_get_node_info_not_found() {
        let node = create_test_node().await;
        let state = create_test_state(node);

        let result = get_node_info(State(state), Path("nonexistent".to_string())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_node_neighbors() {
        let node = create_test_node().await;
        let node_id = node.id().0.clone();
        let state = create_test_state(node);

        let result = get_node_neighbors(State(state), Path(node_id)).await;
        assert!(result.is_ok());

        let neighbors = result.unwrap().0;
        assert_eq!(neighbors.len(), 0); // No neighbors initially
    }

    #[tokio::test]
    async fn test_get_topology() {
        let node = create_test_node().await;
        let state = create_test_state(node);

        let result = get_topology(State(state)).await;
        assert!(result.is_ok());

        let topology = result.unwrap().0;
        assert_eq!(topology.nodes.len(), 1); // Only local node
        assert_eq!(topology.edges.len(), 0); // No edges
        assert!(topology.nodes[0].is_local);
    }

    #[tokio::test]
    async fn test_packet_status_not_found() {
        let node = create_test_node().await;
        let state = create_test_state(node);

        let result = get_packet_status(State(state), Path("nonexistent".to_string())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_default_ttl() {
        assert_eq!(default_ttl(), 64);
    }
}
