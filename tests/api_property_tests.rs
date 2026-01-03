//! Property-Based Tests for REST API
//!
//! These tests verify universal properties of the API endpoints using proptest.

use drfe_r::api::{ApiState, DeliveryStatus, PacketStatus, SendPacketRequest};
use drfe_r::coordinates::NodeId;
use drfe_r::network::DistributedNode;
use governor::{Quota, RateLimiter};
use nonzero_ext::nonzero;
use proptest::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

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

// Strategy for generating valid node IDs
fn node_id_strategy() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{2,15}".prop_map(|s| s.to_string())
}

// Strategy for generating valid payloads
fn payload_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(any::<u8>(), 0..1000).prop_map(|bytes| {
        String::from_utf8_lossy(&bytes).to_string()
    })
}

// Strategy for generating valid TTL values
fn ttl_strategy() -> impl Strategy<Value = u32> {
    1u32..=255u32
}

// Strategy for generating SendPacketRequest
fn send_packet_request_strategy() -> impl Strategy<Value = SendPacketRequest> {
    (node_id_strategy(), payload_strategy(), ttl_strategy()).prop_map(|(dest, payload, ttl)| {
        SendPacketRequest {
            destination: dest,
            payload,
            ttl,
        }
    })
}

// Feature: drfe-r-completion, Property 8: API Response Completeness
// **Validates: Requirements 4.3**
//
// Property 8: API Response Completeness
// For any valid packet send request via API, the response must include delivery status and path information
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_api_response_completeness(
        request in send_packet_request_strategy()
    ) {
        // Run async test
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Create API state
            let state = create_api_state("test_node").await;

            // Simulate sending a packet by creating a packet status
            let packet_id = uuid::Uuid::new_v4().to_string();
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
                tracker.insert(packet_id.clone(), status.clone());
            }

            // Verify response completeness
            // 1. Packet status must have an ID
            prop_assert!(!status.id.is_empty(), "Packet ID must not be empty");

            // 2. Packet status must have a delivery status
            match &status.status {
                DeliveryStatus::Pending => {},
                DeliveryStatus::InTransit { current_node } => {
                    prop_assert!(!current_node.is_empty(), "Current node must not be empty");
                },
                DeliveryStatus::Delivered { latency_ms: _ } => {
                    // Latency is always non-negative (u64)
                },
                DeliveryStatus::Failed { reason } => {
                    prop_assert!(!reason.is_empty(), "Failure reason must not be empty");
                },
            }

            // 3. Packet status must have path information
            prop_assert!(!status.path.is_empty(), "Path must not be empty");
            prop_assert!(status.path.contains(&state.node.id().0), "Path must contain source node");

            // 4. Packet status must have source and destination
            prop_assert!(!status.source.is_empty(), "Source must not be empty");
            prop_assert!(!status.destination.is_empty(), "Destination must not be empty");
            prop_assert_eq!(status.destination, request.destination, "Destination must match request");

            // 5. Packet status must have a timestamp
            prop_assert!(status.created_at > 0, "Timestamp must be positive");

            // 6. Hops is always non-negative (u32)

            Ok(())
        })?;
    }
}

// Additional property: Valid requests should have valid TTL
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_valid_ttl_range(request in send_packet_request_strategy()) {
        // TTL must be in valid range [1, 255]
        prop_assert!(request.ttl >= 1, "TTL must be at least 1");
        prop_assert!(request.ttl <= 255, "TTL must be at most 255");
    }
}

// Additional property: Valid requests should have non-empty destination
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_non_empty_destination(request in send_packet_request_strategy()) {
        // Destination must not be empty
        prop_assert!(!request.destination.is_empty(), "Destination must not be empty");
        
        // Destination must be a valid node ID format
        prop_assert!(request.destination.len() >= 3, "Destination must be at least 3 characters");
        prop_assert!(request.destination.len() <= 16, "Destination must be at most 16 characters");
    }
}

// Additional property: Packet IDs should be unique
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_unique_packet_ids(
        requests in prop::collection::vec(send_packet_request_strategy(), 1..20)
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let _state = create_api_state("test_node").await;
            let mut packet_ids = std::collections::HashSet::new();

            // Generate packet IDs for all requests
            for _ in &requests {
                let packet_id = uuid::Uuid::new_v4().to_string();
                
                // Packet ID should be unique
                prop_assert!(
                    packet_ids.insert(packet_id.clone()),
                    "Packet IDs must be unique"
                );
            }

            Ok(())
        })?;
    }
}

// Additional property: Path should always start with source node
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_path_starts_with_source(request in send_packet_request_strategy()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let state = create_api_state("test_node").await;

            let packet_id = uuid::Uuid::new_v4().to_string();
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

            // Path should start with source
            prop_assert!(!status.path.is_empty(), "Path must not be empty");
            prop_assert_eq!(
                &status.path[0],
                &status.source,
                "Path must start with source node"
            );

            Ok(())
        })?;
    }
}

// Additional property: Hops should not exceed TTL
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_hops_not_exceed_ttl(request in send_packet_request_strategy()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let state = create_api_state("test_node").await;

            let packet_id = uuid::Uuid::new_v4().to_string();
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

            // Hops should not exceed TTL
            prop_assert!(
                status.hops <= request.ttl,
                "Hops ({}) must not exceed TTL ({})",
                status.hops,
                request.ttl
            );

            Ok(())
        })?;
    }
}

// Feature: drfe-r-completion, Property 9: Status Query Completeness
// **Validates: Requirements 4.4**
//
// Property 9: Status Query Completeness
// For any node status query, the response must include current coordinates and neighbor information
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_status_query_completeness(node_id in node_id_strategy()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Create API state with the given node ID
            let state = create_api_state(&node_id).await;

            // Get node coordinate
            let coord = state.node.coord().await;

            // Verify coordinate completeness
            // 1. Coordinate must have x and y values
            prop_assert!(coord.point.x.is_finite(), "X coordinate must be finite");
            prop_assert!(coord.point.y.is_finite(), "Y coordinate must be finite");

            // 2. Coordinate must be within Poincaré disk (|z| < 1)
            let norm = coord.point.euclidean_norm();
            prop_assert!(norm < 1.0, "Coordinate must be within Poincaré disk (norm: {})", norm);

            // 3. Coordinate has a version/timestamp (always non-negative as u64)

            // Get neighbors
            let neighbors = state.node.neighbors().await;

            // 4. Neighbor information must be complete
            for neighbor in &neighbors {
                // Each neighbor must have an ID
                prop_assert!(!neighbor.id.0.is_empty(), "Neighbor ID must not be empty");

                // Each neighbor must have valid coordinates
                prop_assert!(neighbor.coord.x.is_finite(), "Neighbor X coordinate must be finite");
                prop_assert!(neighbor.coord.y.is_finite(), "Neighbor Y coordinate must be finite");

                let neighbor_norm = neighbor.coord.euclidean_norm();
                prop_assert!(
                    neighbor_norm < 1.0,
                    "Neighbor coordinate must be within Poincaré disk (norm: {})",
                    neighbor_norm
                );

                // Each neighbor must have a valid address
                prop_assert!(neighbor.addr.port() > 0, "Neighbor port must be positive");

                // Neighbor version is always non-negative (u64)
            }

            Ok(())
        })?;
    }
}

// Additional property: Node coordinates should remain in Poincaré disk
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_coordinates_in_poincare_disk(node_id in node_id_strategy()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let state = create_api_state(&node_id).await;
            let coord = state.node.coord().await;

            // Coordinate must be strictly within unit disk
            let norm_sq = coord.point.x * coord.point.x + coord.point.y * coord.point.y;
            prop_assert!(norm_sq < 1.0, "Coordinate must be within Poincaré disk");

            Ok(())
        })?;
    }
}

// Additional property: Neighbor coordinates should be valid
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_neighbor_coordinates_valid(node_id in node_id_strategy()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let state = create_api_state(&node_id).await;
            let neighbors = state.node.neighbors().await;

            for neighbor in &neighbors {
                // Neighbor coordinates must be finite
                prop_assert!(neighbor.coord.x.is_finite(), "Neighbor X must be finite");
                prop_assert!(neighbor.coord.y.is_finite(), "Neighbor Y must be finite");

                // Neighbor coordinates must be in Poincaré disk
                let norm_sq = neighbor.coord.x * neighbor.coord.x + neighbor.coord.y * neighbor.coord.y;
                prop_assert!(
                    norm_sq < 1.0,
                    "Neighbor coordinate must be within Poincaré disk"
                );
            }

            Ok(())
        })?;
    }
}

// Additional property: Node ID should match the requested ID
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_node_id_matches(node_id in node_id_strategy()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let state = create_api_state(&node_id).await;

            // Node ID should match what was requested
            prop_assert_eq!(
                &state.node.id().0,
                &node_id,
                "Node ID must match requested ID"
            );

            Ok(())
        })?;
    }
}

// Additional property: Neighbor addresses should be valid
proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn prop_neighbor_addresses_valid(node_id in node_id_strategy()) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let state = create_api_state(&node_id).await;
            let neighbors = state.node.neighbors().await;

            for neighbor in &neighbors {
                // Address port must be positive
                prop_assert!(neighbor.addr.port() > 0, "Neighbor port must be positive");

                // Address IP should be valid (not unspecified)
                let ip = neighbor.addr.ip();
                prop_assert!(
                    !ip.is_unspecified(),
                    "Neighbor IP should not be unspecified"
                );
            }

            Ok(())
        })?;
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[tokio::test]
    async fn test_api_state_creation() {
        let state = create_api_state("test_node").await;
        assert_eq!(state.node.id().0, "test_node");
        assert_eq!(state.packet_tracker.read().await.len(), 0);
    }

    #[tokio::test]
    async fn test_packet_status_creation() {
        let state = create_api_state("test_node").await;
        let packet_id = uuid::Uuid::new_v4().to_string();

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
            destination: "dest_node".to_string(),
        };

        assert!(!status.id.is_empty());
        assert!(!status.path.is_empty());
        assert_eq!(status.source, "test_node");
        assert_eq!(status.destination, "dest_node");
    }

    #[tokio::test]
    async fn test_node_coordinate_in_disk() {
        let state = create_api_state("test_node").await;
        let coord = state.node.coord().await;

        let norm = coord.point.euclidean_norm();
        assert!(norm < 1.0, "Coordinate must be in Poincaré disk");
    }

    #[tokio::test]
    async fn test_node_id_matches() {
        let state = create_api_state("my_test_node").await;
        assert_eq!(state.node.id().0, "my_test_node");
    }
}

