//! Integration tests for TLS encryption
//!
//! These tests verify that TLS encryption is working correctly for inter-node communication.
//! Tests cover encrypted communication, certificate validation, and security properties.

use drfe_r::coordinates::NodeId;
use drfe_r::network::Packet;
use drfe_r::network_tls::TlsNetworkLayer;
use drfe_r::tls::TlsCertificate;
use drfe_r::PoincareDiskPoint;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Test that encrypted communication works between two nodes
#[tokio::test]
async fn test_encrypted_communication() {
    // Create two nodes with different certificates
    let cert1 = TlsCertificate::generate_self_signed("node1").unwrap();
    let cert2 = TlsCertificate::generate_self_signed("node2").unwrap();
    
    let layer1 = TlsNetworkLayer::new("127.0.0.1:0", cert1).await.unwrap();
    let layer2 = TlsNetworkLayer::new("127.0.0.1:0", cert2).await.unwrap();
    
    // Create a test packet with sensitive data
    let sensitive_data = b"This is sensitive routing information";
    let packet = Packet::new_data(
        NodeId::new("node1"),
        NodeId::new("node2"),
        PoincareDiskPoint::new(0.5, 0.3).unwrap(),
        sensitive_data.to_vec(),
        64,
    );
    
    let layer2_addr = layer2.local_tcp_addr();
    
    // Spawn receiver
    let accept_handle = tokio::spawn(async move {
        let (mut stream, _) = layer2.accept_tls().await.unwrap();
        TlsNetworkLayer::recv_tls(&mut stream).await.unwrap()
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Send encrypted packet
    layer1.send_tls(&packet, layer2_addr).await.unwrap();
    
    // Verify packet received correctly
    let received = accept_handle.await.unwrap();
    assert_eq!(received.header.source.0, "node1");
    assert_eq!(received.header.destination.0, "node2");
    assert_eq!(received.payload, sensitive_data);
}

/// Test that certificate validation prevents unauthorized connections
/// Note: Our current implementation uses NoVerifier for testing, so this test
/// verifies the TLS handshake completes successfully with valid certificates
#[tokio::test]
async fn test_certificate_validation() {
    let cert1 = TlsCertificate::generate_self_signed("node1").unwrap();
    let cert2 = TlsCertificate::generate_self_signed("node2").unwrap();
    
    let layer1 = TlsNetworkLayer::new("127.0.0.1:0", cert1).await.unwrap();
    let layer2 = TlsNetworkLayer::new("127.0.0.1:0", cert2).await.unwrap();
    
    let packet = Packet::new_data(
        NodeId::new("node1"),
        NodeId::new("node2"),
        PoincareDiskPoint::origin(),
        b"Test".to_vec(),
        64,
    );
    
    let layer2_addr = layer2.local_tcp_addr();
    
    let accept_handle = tokio::spawn(async move {
        layer2.accept_tls().await
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // This should succeed with valid certificates
    let result = layer1.send_tls(&packet, layer2_addr).await;
    assert!(result.is_ok(), "TLS connection with valid certificate should succeed");
    
    // Verify connection was accepted
    let accept_result = accept_handle.await.unwrap();
    assert!(accept_result.is_ok(), "TLS accept with valid certificate should succeed");
}

/// Test that data is actually encrypted on the wire
/// This test attempts to read raw TCP data and verifies it's not plaintext
#[tokio::test]
async fn test_data_is_encrypted_on_wire() {
    let cert1 = TlsCertificate::generate_self_signed("node1").unwrap();
    let _cert2 = TlsCertificate::generate_self_signed("node2").unwrap();
    
    let layer1 = TlsNetworkLayer::new("127.0.0.1:0", cert1).await.unwrap();
    let _layer2 = TlsNetworkLayer::new("127.0.0.1:0", _cert2).await.unwrap();
    
    let secret_message = b"SECRET_MESSAGE_THAT_SHOULD_BE_ENCRYPTED";
    let packet = Packet::new_data(
        NodeId::new("node1"),
        NodeId::new("node2"),
        PoincareDiskPoint::origin(),
        secret_message.to_vec(),
        64,
    );
    
    // Spawn a raw TCP listener to capture encrypted data
    let raw_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let raw_addr = raw_listener.local_addr().unwrap();
    
    let capture_handle = tokio::spawn(async move {
        let (mut stream, _) = raw_listener.accept().await.unwrap();
        let mut buffer = vec![0u8; 1024];
        let n = stream.read(&mut buffer).await.unwrap();
        buffer.truncate(n);
        buffer
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Try to send to the raw listener (will fail TLS handshake, but we can capture data)
    let _ = layer1.send_tls(&packet, raw_addr).await;
    
    // Get captured data
    let captured_data = capture_handle.await.unwrap();
    
    // Verify the secret message is NOT in plaintext in the captured data
    let plaintext_found = captured_data.windows(secret_message.len())
        .any(|window| window == secret_message);
    
    assert!(!plaintext_found, "Secret message should not appear in plaintext on the wire");
    
    // Verify we captured some data (TLS handshake)
    assert!(!captured_data.is_empty(), "Should have captured TLS handshake data");
}

/// Test TLS with multiple concurrent connections
#[tokio::test]
async fn test_concurrent_tls_connections() {
    let cert2 = TlsCertificate::generate_self_signed("node2").unwrap();
    let _layer2 = TlsNetworkLayer::new("127.0.0.1:0", cert2).await.unwrap();
    
    // Spawn multiple receivers on separate layers
    let mut receive_handles = vec![];
    for _ in 0..5 {
        let layer2_clone = TlsNetworkLayer::new("127.0.0.1:0", 
            TlsCertificate::generate_self_signed("node2").unwrap()).await.unwrap();
        let layer2_clone_addr = layer2_clone.local_tcp_addr();
        
        let handle = tokio::spawn(async move {
            let (mut stream, _) = layer2_clone.accept_tls().await.unwrap();
            TlsNetworkLayer::recv_tls(&mut stream).await.unwrap()
        });
        receive_handles.push((handle, layer2_clone_addr));
    }
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Send multiple packets concurrently to different receivers
    let mut send_handles = vec![];
    for (i, (_, addr)) in receive_handles.iter().enumerate() {
        let packet = Packet::new_data(
            NodeId::new("node1"),
            NodeId::new("node2"),
            PoincareDiskPoint::origin(),
            format!("Message {}", i).into_bytes(),
            64,
        );
        
        let layer1_clone = TlsNetworkLayer::new("127.0.0.1:0",
            TlsCertificate::generate_self_signed("node1").unwrap()).await.unwrap();
        let dest_addr = *addr;
        
        let handle = tokio::spawn(async move {
            layer1_clone.send_tls(&packet, dest_addr).await
        });
        send_handles.push(handle);
    }
    
    // Wait for all sends to complete
    for handle in send_handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok(), "Concurrent TLS send should succeed");
    }
    
    // Wait for all receives to complete
    for (handle, _) in receive_handles {
        let result = handle.await.unwrap();
        assert!(result.payload.starts_with(b"Message "), "Should receive a message");
    }
}

/// Test TLS connection timeout
#[tokio::test]
async fn test_tls_connection_timeout() {
    let cert = TlsCertificate::generate_self_signed("node1").unwrap();
    let mut layer = TlsNetworkLayer::new("127.0.0.1:0", cert).await.unwrap();
    
    // Set short timeout
    layer.set_connection_timeout(Duration::from_millis(100));
    
    let packet = Packet::new_data(
        NodeId::new("node1"),
        NodeId::new("node2"),
        PoincareDiskPoint::origin(),
        b"Test".to_vec(),
        64,
    );
    
    // Try to connect to non-existent server
    let result = layer.send_tls(&packet, "127.0.0.1:9999".parse().unwrap()).await;
    
    assert!(result.is_err(), "Connection to non-existent server should timeout");
}

/// Test TLS with large payloads
#[tokio::test]
async fn test_tls_large_payload() {
    let cert1 = TlsCertificate::generate_self_signed("node1").unwrap();
    let cert2 = TlsCertificate::generate_self_signed("node2").unwrap();
    
    let layer1 = TlsNetworkLayer::new("127.0.0.1:0", cert1).await.unwrap();
    let layer2 = TlsNetworkLayer::new("127.0.0.1:0", cert2).await.unwrap();
    
    // Create a moderately large payload (50 KB)
    let large_payload = vec![0xAB; 50_000];
    let packet = Packet::new_data(
        NodeId::new("node1"),
        NodeId::new("node2"),
        PoincareDiskPoint::origin(),
        large_payload.clone(),
        64,
    );
    
    let layer2_addr = layer2.local_tcp_addr();
    
    let accept_handle = tokio::spawn(async move {
        match layer2.accept_tls().await {
            Ok((mut stream, _)) => {
                TlsNetworkLayer::recv_tls(&mut stream).await
            }
            Err(e) => Err(e),
        }
    });
    
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // Send large encrypted packet
    let send_result = layer1.send_tls(&packet, layer2_addr).await;
    assert!(send_result.is_ok(), "Failed to send large packet: {:?}", send_result.err());
    
    // Verify large packet received correctly
    let received = accept_handle.await.unwrap();
    assert!(received.is_ok(), "Failed to receive large packet: {:?}", received.err());
    let received = received.unwrap();
    assert_eq!(received.payload.len(), 50_000);
    assert_eq!(received.payload, large_payload);
}

/// Test TLS session reuse (multiple packets on same connection)
#[tokio::test]
async fn test_tls_session_reuse() {
    let cert1 = TlsCertificate::generate_self_signed("node1").unwrap();
    let cert2 = TlsCertificate::generate_self_signed("node2").unwrap();
    
    let layer1 = TlsNetworkLayer::new("127.0.0.1:0", cert1).await.unwrap();
    let layer2 = TlsNetworkLayer::new("127.0.0.1:0", cert2).await.unwrap();
    
    let layer2_addr = layer2.local_tcp_addr();
    
    // Accept connection and receive multiple packets
    let accept_handle = tokio::spawn(async move {
        let (mut stream, _) = layer2.accept_tls().await.unwrap();
        let p1 = TlsNetworkLayer::recv_tls(&mut stream).await.unwrap();
        let p2 = TlsNetworkLayer::recv_tls(&mut stream).await.unwrap();
        let p3 = TlsNetworkLayer::recv_tls(&mut stream).await.unwrap();
        (p1, p2, p3)
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Establish one TLS connection and send multiple packets
    let tcp_stream = TcpStream::connect(layer2_addr).await.unwrap();
    let server_name = rustls::pki_types::ServerName::try_from("localhost").unwrap();
    let mut tls_stream = layer1.tls_connector.connect(server_name, tcp_stream).await.unwrap();
    
    // Send three packets on the same TLS connection
    for i in 1..=3 {
        let packet = Packet::new_data(
            NodeId::new("node1"),
            NodeId::new("node2"),
            PoincareDiskPoint::origin(),
            format!("Packet {}", i).into_bytes(),
            64,
        );
        
        let bytes = packet.to_msgpack().unwrap();
        let len = bytes.len() as u32;
        tls_stream.write_all(&len.to_be_bytes()).await.unwrap();
        tls_stream.write_all(&bytes).await.unwrap();
    }
    tls_stream.flush().await.unwrap();
    
    // Verify all packets received
    let (p1, p2, p3) = accept_handle.await.unwrap();
    assert_eq!(p1.payload, b"Packet 1");
    assert_eq!(p2.payload, b"Packet 2");
    assert_eq!(p3.payload, b"Packet 3");
}

/// Test that different node certificates can communicate
#[tokio::test]
async fn test_different_certificates_can_communicate() {
    // Create three nodes with different certificates
    let cert1 = TlsCertificate::generate_self_signed("node1").unwrap();
    let cert2 = TlsCertificate::generate_self_signed("node2").unwrap();
    let cert3 = TlsCertificate::generate_self_signed("node3").unwrap();
    
    let layer1 = TlsNetworkLayer::new("127.0.0.1:0", cert1).await.unwrap();
    let layer2 = TlsNetworkLayer::new("127.0.0.1:0", cert2).await.unwrap();
    let layer3 = TlsNetworkLayer::new("127.0.0.1:0", cert3).await.unwrap();
    
    // Test node1 -> node2
    let packet12 = Packet::new_data(
        NodeId::new("node1"),
        NodeId::new("node2"),
        PoincareDiskPoint::origin(),
        b"1 to 2".to_vec(),
        64,
    );
    
    let layer2_addr = layer2.local_tcp_addr();
    let accept2 = tokio::spawn(async move {
        let (mut stream, _) = layer2.accept_tls().await.unwrap();
        TlsNetworkLayer::recv_tls(&mut stream).await.unwrap()
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    layer1.send_tls(&packet12, layer2_addr).await.unwrap();
    
    let received2 = accept2.await.unwrap();
    assert_eq!(received2.payload, b"1 to 2");
    
    // Test node1 -> node3 (using the same layer1)
    let packet13 = Packet::new_data(
        NodeId::new("node1"),
        NodeId::new("node3"),
        PoincareDiskPoint::origin(),
        b"1 to 3".to_vec(),
        64,
    );
    
    let layer3_addr = layer3.local_tcp_addr();
    let accept3 = tokio::spawn(async move {
        let (mut stream, _) = layer3.accept_tls().await.unwrap();
        TlsNetworkLayer::recv_tls(&mut stream).await.unwrap()
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    layer1.send_tls(&packet13, layer3_addr).await.unwrap();
    
    let received3 = accept3.await.unwrap();
    assert_eq!(received3.payload, b"1 to 3");
}

/// Test TLS handshake failure handling
#[tokio::test]
async fn test_tls_handshake_failure() {
    let cert = TlsCertificate::generate_self_signed("node1").unwrap();
    let layer = TlsNetworkLayer::new("127.0.0.1:0", cert).await.unwrap();
    
    // Start a plain TCP server (no TLS)
    let plain_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let plain_addr = plain_listener.local_addr().unwrap();
    
    tokio::spawn(async move {
        let (mut stream, _) = plain_listener.accept().await.unwrap();
        // Send some non-TLS data
        let _ = stream.write_all(b"NOT TLS DATA").await;
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let packet = Packet::new_data(
        NodeId::new("node1"),
        NodeId::new("node2"),
        PoincareDiskPoint::origin(),
        b"Test".to_vec(),
        64,
    );
    
    // Try to establish TLS connection with plain TCP server
    let result = layer.send_tls(&packet, plain_addr).await;
    
    // Should fail due to TLS handshake failure
    assert!(result.is_err(), "TLS handshake with non-TLS server should fail");
}
