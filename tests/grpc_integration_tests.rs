//! Integration tests for gRPC API
//!
//! Tests the gRPC service endpoints including SendPacket, GetNodeStatus,
//! and StreamTopology.

use drfe_r::coordinates::NodeId;
use drfe_r::grpc::proto::routing_service_client::RoutingServiceClient;
use drfe_r::grpc::proto::{
    GetNodeStatusRequest, SendPacketRequest, TopologyRequest, UpdateType,
};
use drfe_r::grpc::{start_grpc_server, GrpcServiceState, PacketStatusInfo};
use drfe_r::network::DistributedNode;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::time::{sleep, Duration};
use tonic::Request;

/// Helper function to create a test node
async fn create_test_node(id: &str, udp_port: u16, tcp_port: u16) -> Arc<DistributedNode> {
    let udp_addr = format!("127.0.0.1:{}", udp_port);
    let tcp_addr = format!("127.0.0.1:{}", tcp_port);

    Arc::new(
        DistributedNode::new(NodeId::new(id), &udp_addr, &tcp_addr)
            .await
            .unwrap(),
    )
}

/// Helper function to start a gRPC server in the background
async fn start_test_grpc_server(
    node: Arc<DistributedNode>,
    grpc_port: u16,
) -> tokio::task::JoinHandle<()> {
    let bind_addr = format!("127.0.0.1:{}", grpc_port);

    tokio::spawn(async move {
        if let Err(e) = start_grpc_server(node, &bind_addr).await {
            eprintln!("gRPC server error: {}", e);
        }
    })
}

/// Test SendPacket RPC
#[tokio::test]
async fn test_send_packet_rpc() {
    // Create test node
    let node = create_test_node("test_node_1", 40001, 40002).await;

    // Start gRPC server
    let grpc_port = 50051;
    let _server_handle = start_test_grpc_server(node.clone(), grpc_port).await;

    // Wait for server to start
    sleep(Duration::from_millis(500)).await;

    // Create gRPC client
    let mut client = RoutingServiceClient::connect(format!("http://127.0.0.1:{}", grpc_port))
        .await
        .expect("Failed to connect to gRPC server");

    // Test valid packet send (will fail due to no neighbors, but request should be accepted)
    let request = Request::new(SendPacketRequest {
        destination: "dest_node".to_string(),
        payload: b"test payload".to_vec(),
        ttl: 64,
    });

    let response = client.send_packet(request).await;
    // Note: This will return an error because the node has no neighbors,
    // but the gRPC call itself should work (not a protocol error)
    if let Err(e) = &response {
        // Should be an Internal error (routing failed), not InvalidArgument
        assert_eq!(e.code(), tonic::Code::Internal, "Should be Internal error due to no neighbors");
    }

    // Test invalid packet send (empty destination)
    let request = Request::new(SendPacketRequest {
        destination: "".to_string(),
        payload: b"test".to_vec(),
        ttl: 64,
    });

    let response = client.send_packet(request).await;
    assert!(response.is_err(), "SendPacket with empty destination should fail");
    assert_eq!(response.unwrap_err().code(), tonic::Code::InvalidArgument);

    // Test invalid TTL
    let request = Request::new(SendPacketRequest {
        destination: "dest_node".to_string(),
        payload: b"test".to_vec(),
        ttl: 256,
    });

    let response = client.send_packet(request).await;
    assert!(response.is_err(), "SendPacket with TTL > 255 should fail");
    assert_eq!(response.unwrap_err().code(), tonic::Code::InvalidArgument);
}

/// Test GetNodeStatus RPC
#[tokio::test]
async fn test_get_node_status_rpc() {
    // Create test node
    let node = create_test_node("test_node_2", 40003, 40004).await;
    let node_id = node.id().0.clone();

    // Start gRPC server
    let grpc_port = 50052;
    let _server_handle = start_test_grpc_server(node.clone(), grpc_port).await;

    // Wait for server to start
    sleep(Duration::from_millis(500)).await;

    // Create gRPC client
    let mut client = RoutingServiceClient::connect(format!("http://127.0.0.1:{}", grpc_port))
        .await
        .expect("Failed to connect to gRPC server");

    // Test getting local node status
    let request = Request::new(GetNodeStatusRequest {
        node_id: node_id.clone(),
    });

    let response = client.get_node_status(request).await;
    assert!(response.is_ok(), "GetNodeStatus should succeed for local node");

    let status = response.unwrap().into_inner();
    assert_eq!(status.node_id, node_id);
    assert!(status.coordinate.is_some());

    let coord = status.coordinate.unwrap();
    assert!(coord.norm < 1.0, "Coordinate should be in Poincaré disk");
    assert!(!status.udp_address.is_empty());
    assert!(!status.tcp_address.is_empty());

    // Test getting non-existent node status
    let request = Request::new(GetNodeStatusRequest {
        node_id: "nonexistent_node".to_string(),
    });

    let response = client.get_node_status(request).await;
    assert!(response.is_err(), "GetNodeStatus should fail for non-existent node");
    assert_eq!(response.unwrap_err().code(), tonic::Code::NotFound);
}

/// Test StreamTopology streaming RPC
#[tokio::test]
async fn test_stream_topology_rpc() {
    // Create test node
    let node = create_test_node("test_node_3", 40005, 40006).await;

    // Start gRPC server
    let grpc_port = 50053;
    let _server_handle = start_test_grpc_server(node.clone(), grpc_port).await;

    // Wait for server to start
    sleep(Duration::from_millis(500)).await;

    // Create gRPC client
    let mut client = RoutingServiceClient::connect(format!("http://127.0.0.1:{}", grpc_port))
        .await
        .expect("Failed to connect to gRPC server");

    // Test streaming with initial snapshot
    let request = Request::new(TopologyRequest {
        include_snapshot: true,
        interval_secs: 0,
    });

    let response = client.stream_topology(request).await;
    assert!(response.is_ok(), "StreamTopology should succeed");

    let mut stream = response.unwrap().into_inner();

    // Receive first update (should be snapshot)
    let first_update = stream.message().await;
    assert!(first_update.is_ok());

    let update = first_update.unwrap();
    assert!(update.is_some());

    let topology_update = update.unwrap();
    assert_eq!(
        topology_update.update_type,
        UpdateType::Snapshot as i32,
        "First update should be a snapshot"
    );
    assert!(!topology_update.nodes.is_empty(), "Snapshot should contain nodes");
    assert_eq!(
        topology_update.nodes.len(),
        1,
        "Should have exactly one node (local)"
    );
    assert!(topology_update.nodes[0].is_local, "Node should be marked as local");

    // Test streaming without initial snapshot
    let request = Request::new(TopologyRequest {
        include_snapshot: false,
        interval_secs: 0,
    });

    let response = client.stream_topology(request).await;
    assert!(response.is_ok(), "StreamTopology without snapshot should succeed");
}

/// Test multiple concurrent gRPC clients
#[tokio::test]
async fn test_concurrent_grpc_clients() {
    // Create test node
    let node = create_test_node("test_node_4", 40007, 40008).await;

    // Start gRPC server
    let grpc_port = 50054;
    let _server_handle = start_test_grpc_server(node.clone(), grpc_port).await;

    // Wait for server to start
    sleep(Duration::from_millis(500)).await;

    // Create multiple clients
    let mut handles = vec![];

    for i in 0..5 {
        let grpc_port = grpc_port;
        let handle = tokio::spawn(async move {
            let mut client =
                RoutingServiceClient::connect(format!("http://127.0.0.1:{}", grpc_port))
                    .await
                    .expect("Failed to connect");

            let request = Request::new(SendPacketRequest {
                destination: format!("dest_{}", i),
                payload: format!("payload_{}", i).as_bytes().to_vec(),
                ttl: 64,
            });

            client.send_packet(request).await
        });

        handles.push(handle);
    }

    // Wait for all requests to complete
    // Note: These will fail due to no neighbors, but the gRPC calls should work
    for handle in handles {
        let result = handle.await.expect("Task should complete");
        // Should get Internal error (no neighbors), not a protocol error
        if let Err(e) = result {
            assert_eq!(e.code(), tonic::Code::Internal, "Should be Internal error");
        }
    }
}

/// Test gRPC error handling
#[tokio::test]
async fn test_grpc_error_handling() {
    // Create test node
    let node = create_test_node("test_node_5", 40009, 40010).await;

    // Start gRPC server
    let grpc_port = 50055;
    let _server_handle = start_test_grpc_server(node.clone(), grpc_port).await;

    // Wait for server to start
    sleep(Duration::from_millis(500)).await;

    // Create gRPC client
    let mut client = RoutingServiceClient::connect(format!("http://127.0.0.1:{}", grpc_port))
        .await
        .expect("Failed to connect to gRPC server");

    // Test various error conditions
    let test_cases = vec![
        (
            SendPacketRequest {
                destination: "".to_string(),
                payload: b"test".to_vec(),
                ttl: 64,
            },
            tonic::Code::InvalidArgument,
            "empty destination",
        ),
        (
            SendPacketRequest {
                destination: "dest".to_string(),
                payload: b"test".to_vec(),
                ttl: 0,
            },
            tonic::Code::Internal, // Will fail due to no neighbors
            "zero TTL (should default but fail on routing)",
        ),
        (
            SendPacketRequest {
                destination: "dest".to_string(),
                payload: b"test".to_vec(),
                ttl: 300,
            },
            tonic::Code::InvalidArgument,
            "TTL > 255",
        ),
    ];

    for (request, expected_code, description) in test_cases {
        let result = client.send_packet(Request::new(request)).await;

        assert!(result.is_err(), "Request should fail: {}", description);
        assert_eq!(
            result.unwrap_err().code(),
            expected_code,
            "Wrong error code for: {}",
            description
        );
    }
}

/// Test gRPC service state management
#[tokio::test]
async fn test_grpc_state_management() {
    // Create test node
    let node = create_test_node("test_node_6", 40011, 40012).await;

    // Create state manually
    let (topology_tx, _) = broadcast::channel(100);
    let state = GrpcServiceState {
        node: node.clone(),
        packet_tracker: Arc::new(RwLock::new(HashMap::new())),
        topology_tx,
    };

    // Add some packet status
    {
        let mut tracker = state.packet_tracker.write().await;
        tracker.insert(
            "test_packet_1".to_string(),
            PacketStatusInfo {
                id: "test_packet_1".to_string(),
                source: "test_node_6".to_string(),
                destination: "dest_node".to_string(),
                created_at: 1234567890,
            },
        );
    }

    // Verify state
    {
        let tracker = state.packet_tracker.read().await;
        assert_eq!(tracker.len(), 1);
        assert!(tracker.contains_key("test_packet_1"));
    }
}
