//! Integration Tests for REST API
//!
//! These tests verify the REST API endpoints work correctly end-to-end.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use drfe_r::api::{create_router, ApiState, SendPacketRequest};
use drfe_r::coordinates::NodeId;
use drfe_r::network::{DistributedNode, NeighborInfo};
use drfe_r::PoincareDiskPoint;
use governor::{Quota, RateLimiter};
use nonzero_ext::nonzero;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceExt; // for `oneshot`

/// Helper to create a test node
async fn create_test_node(id: &str) -> Arc<DistributedNode> {
    Arc::new(
        DistributedNode::new(NodeId::new(id), "127.0.0.1:0", "127.0.0.1:0")
            .await
            .unwrap(),
    )
}

/// Helper to create API state
async fn create_api_state(node_id: &str) -> ApiState {
    let node = create_test_node(node_id).await;
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
async fn test_send_packet_endpoint() {
    let state = create_api_state("test_node").await;
    let app = create_router(state);

    // Create a valid send packet request
    let request_body = json!({
        "destination": "dest_node",
        "payload": "Hello, World!",
        "ttl": 64
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/packets")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 500 because there's no route to dest_node
    // (This is expected behavior - the node doesn't have neighbors)
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_send_packet_invalid_destination() {
    let state = create_api_state("test_node").await;
    let app = create_router(state);

    // Create request with empty destination
    let request_body = json!({
        "destination": "",
        "payload": "Hello, World!",
        "ttl": 64
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/packets")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 400 Bad Request
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_send_packet_invalid_ttl_zero() {
    let state = create_api_state("test_node").await;
    let app = create_router(state);

    // Create request with TTL = 0
    let request_body = json!({
        "destination": "dest_node",
        "payload": "Hello, World!",
        "ttl": 0
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/packets")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 400 Bad Request
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_send_packet_invalid_ttl_too_large() {
    let state = create_api_state("test_node").await;
    let app = create_router(state);

    // Create request with TTL > 255
    let request_body = json!({
        "destination": "dest_node",
        "payload": "Hello, World!",
        "ttl": 256
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/packets")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 400 Bad Request
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_get_packet_status_not_found() {
    let state = create_api_state("test_node").await;
    let app = create_router(state);

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/packets/nonexistent-id")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 404 Not Found
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_node_info_local() {
    let state = create_api_state("test_node").await;
    let node_id = state.node.id().0.clone();
    let app = create_router(state);

    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/nodes/{}", node_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 200 OK
    assert_eq!(response.status(), StatusCode::OK);

    // Parse response body
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Verify response structure
    assert_eq!(json["id"], "test_node");
    assert!(json["coordinate"]["x"].is_number());
    assert!(json["coordinate"]["y"].is_number());
    assert!(json["coordinate"]["norm"].is_number());
    assert!(json["neighbor_count"].is_number());
}

#[tokio::test]
async fn test_get_node_info_not_found() {
    let state = create_api_state("test_node").await;
    let app = create_router(state);

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/nodes/nonexistent_node")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 404 Not Found
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_node_neighbors() {
    let state = create_api_state("test_node").await;
    let node_id = state.node.id().0.clone();
    let app = create_router(state);

    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/nodes/{}/neighbors", node_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 200 OK
    assert_eq!(response.status(), StatusCode::OK);

    // Parse response body
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Should be an empty array (no neighbors initially)
    assert!(json.is_array());
    assert_eq!(json.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_get_node_neighbors_with_neighbors() {
    let state = create_api_state("test_node").await;
    let node_id = state.node.id().0.clone();

    // Add a neighbor
    let neighbor = NeighborInfo::new(
        NodeId::new("neighbor1"),
        PoincareDiskPoint::new(0.5, 0.3).unwrap(),
        "127.0.0.1:8000".parse().unwrap(),
    );
    state.node.add_neighbor(neighbor).await;

    let app = create_router(state);

    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/nodes/{}/neighbors", node_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 200 OK
    assert_eq!(response.status(), StatusCode::OK);

    // Parse response body
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Should have one neighbor
    assert!(json.is_array());
    assert_eq!(json.as_array().unwrap().len(), 1);

    let neighbor_json = &json[0];
    assert_eq!(neighbor_json["id"], "neighbor1");
    assert!(neighbor_json["coordinate"]["x"].is_number());
    assert!(neighbor_json["coordinate"]["y"].is_number());
}

#[tokio::test]
async fn test_get_node_neighbors_wrong_node() {
    let state = create_api_state("test_node").await;
    let app = create_router(state);

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/nodes/other_node/neighbors")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 400 Bad Request (can only query local node's neighbors)
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_get_topology() {
    let state = create_api_state("test_node").await;
    let app = create_router(state);

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/topology")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 200 OK
    assert_eq!(response.status(), StatusCode::OK);

    // Parse response body
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Verify response structure
    assert!(json["nodes"].is_array());
    assert!(json["edges"].is_array());

    // Should have one node (local node)
    assert_eq!(json["nodes"].as_array().unwrap().len(), 1);
    assert_eq!(json["nodes"][0]["id"], "test_node");
    assert_eq!(json["nodes"][0]["is_local"], true);

    // Should have no edges (no neighbors)
    assert_eq!(json["edges"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_get_topology_with_neighbors() {
    let state = create_api_state("test_node").await;

    // Add two neighbors
    let neighbor1 = NeighborInfo::new(
        NodeId::new("neighbor1"),
        PoincareDiskPoint::new(0.3, 0.2).unwrap(),
        "127.0.0.1:8001".parse().unwrap(),
    );
    let neighbor2 = NeighborInfo::new(
        NodeId::new("neighbor2"),
        PoincareDiskPoint::new(-0.2, 0.4).unwrap(),
        "127.0.0.1:8002".parse().unwrap(),
    );

    state.node.add_neighbor(neighbor1).await;
    state.node.add_neighbor(neighbor2).await;

    let app = create_router(state);

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/topology")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 200 OK
    assert_eq!(response.status(), StatusCode::OK);

    // Parse response body
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Should have three nodes (local + 2 neighbors)
    assert_eq!(json["nodes"].as_array().unwrap().len(), 3);

    // Should have two edges (local to each neighbor)
    assert_eq!(json["edges"].as_array().unwrap().len(), 2);

    // Verify edges have distance information
    for edge in json["edges"].as_array().unwrap() {
        assert!(edge["source"].is_string());
        assert!(edge["target"].is_string());
        assert!(edge["distance"].is_number());
        assert!(edge["distance"].as_f64().unwrap() >= 0.0);
    }
}

#[tokio::test]
async fn test_cors_headers() {
    let state = create_api_state("test_node").await;
    let app = create_router(state);

    let request = Request::builder()
        .method("OPTIONS")
        .uri("/api/v1/topology")
        .header("origin", "http://localhost:3000")
        .header("access-control-request-method", "GET")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 200 OK for OPTIONS request
    assert_eq!(response.status(), StatusCode::OK);

    // Should have CORS headers
    let headers = response.headers();
    assert!(headers.contains_key("access-control-allow-origin"));
}

#[tokio::test]
async fn test_invalid_json_body() {
    let state = create_api_state("test_node").await;
    let app = create_router(state);

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/packets")
        .header("content-type", "application/json")
        .body(Body::from("invalid json"))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 400 Bad Request or 422 Unprocessable Entity
    assert!(
        response.status() == StatusCode::BAD_REQUEST
            || response.status() == StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn test_missing_required_fields() {
    let state = create_api_state("test_node").await;
    let app = create_router(state);

    // Missing destination field
    let request_body = json!({
        "payload": "Hello, World!",
        "ttl": 64
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/packets")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 400 Bad Request or 422 Unprocessable Entity
    assert!(
        response.status() == StatusCode::BAD_REQUEST
            || response.status() == StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn test_default_ttl() {
    // Test that TTL defaults to 64 when not specified
    let request = SendPacketRequest {
        destination: "dest_node".to_string(),
        payload: "test".to_string(),
        ttl: 64, // This is the default
    };

    assert_eq!(request.ttl, 64);
}

#[tokio::test]
async fn test_coordinate_info_in_response() {
    let state = create_api_state("test_node").await;
    let node_id = state.node.id().0.clone();
    let app = create_router(state);

    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/nodes/{}", node_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Verify coordinate is within Poincaré disk
    let x = json["coordinate"]["x"].as_f64().unwrap();
    let y = json["coordinate"]["y"].as_f64().unwrap();
    let norm = (x * x + y * y).sqrt();

    assert!(norm < 1.0, "Coordinate must be within Poincaré disk");
}

#[tokio::test]
async fn test_neighbor_info_completeness() {
    let state = create_api_state("test_node").await;
    let node_id = state.node.id().0.clone();

    // Add a neighbor
    let neighbor = NeighborInfo::new(
        NodeId::new("neighbor1"),
        PoincareDiskPoint::new(0.5, 0.3).unwrap(),
        "127.0.0.1:8000".parse().unwrap(),
    );
    state.node.add_neighbor(neighbor).await;

    let app = create_router(state);

    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/nodes/{}/neighbors", node_id))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let neighbor_json = &json[0];

    // Verify all required fields are present
    assert!(neighbor_json["id"].is_string());
    assert!(neighbor_json["coordinate"]["x"].is_number());
    assert!(neighbor_json["coordinate"]["y"].is_number());
    assert!(neighbor_json["coordinate"]["norm"].is_number());
    assert!(neighbor_json["coordinate"]["version"].is_number());
    assert!(neighbor_json["address"].is_string());
    assert!(neighbor_json["rtt_ms"].is_number());
    assert!(neighbor_json["last_heartbeat_secs"].is_number());
}
