//! Authentication and Rate Limiting Tests
//!
//! These tests verify that authentication and rate limiting work correctly.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use base64::Engine;
use drfe_r::api::{create_router, register_auth_key, ApiState};
use drfe_r::coordinates::NodeId;
use drfe_r::network::DistributedNode;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use governor::{Quota, RateLimiter};
use nonzero_ext::nonzero;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceExt;

/// Helper to create a test node
async fn create_test_node(id: &str) -> Arc<DistributedNode> {
    Arc::new(
        DistributedNode::new(NodeId::new(id), "127.0.0.1:0", "127.0.0.1:0")
            .await
            .unwrap(),
    )
}

/// Helper to create API state with authentication enabled
async fn create_auth_api_state(node_id: &str, require_auth: bool) -> ApiState {
    let node = create_test_node(node_id).await;
    let quota = Quota::per_minute(nonzero!(100u32));
    let rate_limiter = Arc::new(RateLimiter::keyed(quota));

    ApiState {
        node,
        packet_tracker: Arc::new(RwLock::new(HashMap::new())),
        rate_limiter,
        auth_keys: Arc::new(RwLock::new(HashMap::new())),
        require_auth,
    }
}

/// Helper to generate Ed25519 key pair
fn generate_keypair() -> (SigningKey, VerifyingKey) {
    
    let mut rng = rand::thread_rng();
    let signing_key = SigningKey::from_bytes(&rand::Rng::gen(&mut rng));
    let verifying_key = signing_key.verifying_key();
    (signing_key, verifying_key)
}

/// Helper to create signed request
fn create_signed_request(
    method: &str,
    uri: &str,
    node_id: &str,
    signing_key: &SigningKey,
    body: Option<String>,
) -> Request<Body> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    // Sign the message (node_id:timestamp)
    let message = format!("{}:{}", node_id, timestamp);
    let signature = signing_key.sign(message.as_bytes());
    let signature_b64 = base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());

    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("X-Node-Id", node_id)
        .header("X-Signature", signature_b64)
        .header("X-Timestamp", timestamp.to_string());

    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }

    builder
        .body(Body::from(body.unwrap_or_default()))
        .unwrap()
}

#[tokio::test]
async fn test_unauthenticated_request_without_auth_required() {
    // When authentication is not required, requests should succeed without auth headers
    let state = create_auth_api_state("test_node", false).await;
    let app = create_router(state);

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/topology")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 200 OK
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_unauthenticated_request_with_auth_required() {
    // When authentication is required, requests without auth headers should fail
    let state = create_auth_api_state("test_node", true).await;
    let app = create_router(state);

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/topology")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 401 Unauthorized
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_authenticated_request_with_valid_signature() {
    // When authentication is required and valid signature is provided, request should succeed
    let state = create_auth_api_state("test_node", true).await;
    let (signing_key, verifying_key) = generate_keypair();

    // Register the public key
    register_auth_key(&state, "client_node", verifying_key.as_bytes())
        .await
        .unwrap();

    let app = create_router(state);

    let request = create_signed_request("GET", "/api/v1/topology", "client_node", &signing_key, None);

    let response = app.oneshot(request).await.unwrap();

    // Should return 200 OK
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_authenticated_request_with_invalid_signature() {
    // When authentication is required and invalid signature is provided, request should fail
    let state = create_auth_api_state("test_node", true).await;
    let (_signing_key, verifying_key) = generate_keypair();
    let (wrong_signing_key, _) = generate_keypair();

    // Register the public key
    register_auth_key(&state, "client_node", verifying_key.as_bytes())
        .await
        .unwrap();

    let app = create_router(state);

    // Sign with wrong key
    let request = create_signed_request(
        "GET",
        "/api/v1/topology",
        "client_node",
        &wrong_signing_key,
        None,
    );

    let response = app.oneshot(request).await.unwrap();

    // Should return 401 Unauthorized
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_authenticated_request_with_unknown_node() {
    // When authentication is required and node is not registered, request should fail
    let state = create_auth_api_state("test_node", true).await;

    let app = create_router(state);

    let (signing_key, _) = generate_keypair();
    let request = create_signed_request(
        "GET",
        "/api/v1/topology",
        "unknown_node",
        &signing_key,
        None,
    );

    let response = app.oneshot(request).await.unwrap();

    // Should return 401 Unauthorized
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_authenticated_request_with_missing_headers() {
    // When authentication is required and headers are missing, request should fail
    let state = create_auth_api_state("test_node", true).await;
    let app = create_router(state);

    // Missing X-Node-Id
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/topology")
        .header("X-Signature", "dummy")
        .header("X-Timestamp", "123456789")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_authenticated_request_with_old_timestamp() {
    // When authentication is required and timestamp is too old, request should fail
    let state = create_auth_api_state("test_node", true).await;
    let (_signing_key, verifying_key) = generate_keypair();

    // Register the public key
    register_auth_key(&state, "client_node", verifying_key.as_bytes())
        .await
        .unwrap();

    let app = create_router(state);

    // Create request with old timestamp (10 minutes ago)
    let old_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
        - 600_000; // 10 minutes

    let message = format!("client_node:{}", old_timestamp);
    let (signing_key2, _) = generate_keypair();
    let signature = signing_key2.sign(message.as_bytes());
    let signature_b64 = base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/topology")
        .header("X-Node-Id", "client_node")
        .header("X-Signature", signature_b64)
        .header("X-Timestamp", old_timestamp.to_string())
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return 401 Unauthorized
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_rate_limiting_single_client() {
    // Test that rate limiting works for a single client
    let state = create_auth_api_state("test_node", false).await;

    // Create a rate limiter with very low quota for testing (5 requests per minute)
    let quota = Quota::per_minute(nonzero!(5u32));
    let rate_limiter = Arc::new(RateLimiter::keyed(quota));

    let test_state = ApiState {
        node: state.node.clone(),
        packet_tracker: state.packet_tracker.clone(),
        rate_limiter,
        auth_keys: state.auth_keys.clone(),
        require_auth: false,
    };

    let app = create_router(test_state);

    // Make 5 requests (should all succeed)
    for i in 0..5 {
        let request = Request::builder()
            .method("GET")
            .uri("/api/v1/topology")
            .header("X-Node-Id", "client1")
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Request {} should succeed",
            i + 1
        );
    }

    // 6th request should be rate limited
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/topology")
        .header("X-Node-Id", "client1")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn test_rate_limiting_multiple_clients() {
    // Test that rate limiting is per-client
    let state = create_auth_api_state("test_node", false).await;

    // Create a rate limiter with low quota for testing (3 requests per minute)
    let quota = Quota::per_minute(nonzero!(3u32));
    let rate_limiter = Arc::new(RateLimiter::keyed(quota));

    let test_state = ApiState {
        node: state.node.clone(),
        packet_tracker: state.packet_tracker.clone(),
        rate_limiter,
        auth_keys: state.auth_keys.clone(),
        require_auth: false,
    };

    let app = create_router(test_state);

    // Client 1 makes 3 requests (should all succeed)
    for _ in 0..3 {
        let request = Request::builder()
            .method("GET")
            .uri("/api/v1/topology")
            .header("X-Node-Id", "client1")
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // Client 1's 4th request should be rate limited
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/topology")
        .header("X-Node-Id", "client1")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

    // Client 2 should still be able to make requests
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/topology")
        .header("X-Node-Id", "client2")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_register_auth_key_valid() {
    // Test registering a valid public key
    let state = create_auth_api_state("test_node", true).await;
    let (_, verifying_key) = generate_keypair();

    let result = register_auth_key(&state, "client_node", verifying_key.as_bytes()).await;

    assert!(result.is_ok());

    // Verify key was registered
    let auth_keys = state.auth_keys.read().await;
    assert!(auth_keys.contains_key("client_node"));
}

#[tokio::test]
async fn test_register_auth_key_invalid_length() {
    // Test registering a public key with invalid length
    let state = create_auth_api_state("test_node", true).await;

    let invalid_key = vec![0u8; 16]; // Wrong length (should be 32)

    let result = register_auth_key(&state, "client_node", &invalid_key).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid public key length"));
}

#[tokio::test]
async fn test_combined_auth_and_rate_limiting() {
    // Test that both authentication and rate limiting work together
    let state = create_auth_api_state("test_node", true).await;
    let (signing_key, verifying_key) = generate_keypair();

    // Register the public key
    register_auth_key(&state, "client_node", verifying_key.as_bytes())
        .await
        .unwrap();

    // Create a rate limiter with low quota for testing (2 requests per minute)
    let quota = Quota::per_minute(nonzero!(2u32));
    let rate_limiter = Arc::new(RateLimiter::keyed(quota));

    let test_state = ApiState {
        node: state.node.clone(),
        packet_tracker: state.packet_tracker.clone(),
        rate_limiter,
        auth_keys: state.auth_keys.clone(),
        require_auth: true,
    };

    let app = create_router(test_state);

    // Make 2 authenticated requests (should succeed)
    for _ in 0..2 {
        let request = create_signed_request(
            "GET",
            "/api/v1/topology",
            "client_node",
            &signing_key,
            None,
        );

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // 3rd request should be rate limited (even with valid auth)
    let request = create_signed_request(
        "GET",
        "/api/v1/topology",
        "client_node",
        &signing_key,
        None,
    );

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
}
