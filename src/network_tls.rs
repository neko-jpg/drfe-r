//! TLS-enabled Network Layer for DRFE-R
//!
//! This module extends the NetworkLayer with TLS encryption for secure inter-node communication.

use crate::network::{NetworkError, Packet, MAX_PACKET_SIZE};
use crate::tls::{TlsCertificate, TlsConfig};
use rustls::pki_types::ServerName;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, TlsConnector};

/// TLS-enabled network layer for secure communication
pub struct TlsNetworkLayer {
    /// TCP listener for incoming TLS connections
    tcp_listener: Arc<TcpListener>,
    /// TLS acceptor for server-side connections
    tls_acceptor: TlsAcceptor,
    /// TLS connector for client-side connections
    pub tls_connector: TlsConnector,
    /// Connection timeout duration
    connection_timeout: Duration,
    /// Local TCP address
    local_tcp_addr: SocketAddr,
}

impl TlsNetworkLayer {
    /// Create a new TLS-enabled network layer
    ///
    /// # Arguments
    /// * `tcp_addr` - Address to bind TCP listener (e.g., "0.0.0.0:7778")
    /// * `certificate` - TLS certificate and private key
    ///
    /// # Returns
    /// Result containing the TlsNetworkLayer or an error
    pub async fn new(tcp_addr: &str, certificate: TlsCertificate) -> Result<Self, NetworkError> {
        // Bind TCP listener
        let tcp_listener = TcpListener::bind(tcp_addr).await?;
        let local_tcp_addr = tcp_listener.local_addr()?;
        
        // Create TLS configuration
        let tls_config = TlsConfig::new(certificate)
            .map_err(|e| NetworkError::Serialization(e.to_string()))?;
        
        // Create TLS acceptor and connector
        let tls_acceptor = TlsAcceptor::from(tls_config.server_config());
        let tls_connector = TlsConnector::from(tls_config.client_config());
        
        Ok(Self {
            tcp_listener: Arc::new(tcp_listener),
            tls_acceptor,
            tls_connector,
            connection_timeout: Duration::from_secs(30),
            local_tcp_addr,
        })
    }
    
    /// Get local TCP address
    pub fn local_tcp_addr(&self) -> SocketAddr {
        self.local_tcp_addr
    }
    
    /// Send a packet using TLS-encrypted TCP
    ///
    /// # Arguments
    /// * `packet` - The packet to send
    /// * `dest_addr` - Destination socket address
    ///
    /// # Returns
    /// Result indicating success or error
    pub async fn send_tls(&self, packet: &Packet, dest_addr: SocketAddr) -> Result<(), NetworkError> {
        let bytes = packet.to_msgpack()
            .map_err(NetworkError::Serialization)?;
        
        // Connect to destination with TLS
        let tcp_stream = tokio::time::timeout(
            self.connection_timeout,
            TcpStream::connect(dest_addr),
        )
        .await
        .map_err(|_| NetworkError::Timeout)??;
        
        // Perform TLS handshake
        let server_name = ServerName::try_from("localhost")
            .map_err(|e| NetworkError::Serialization(format!("Invalid server name: {:?}", e)))?;
        
        let mut tls_stream = self.tls_connector.connect(server_name, tcp_stream).await
            .map_err(|e| NetworkError::Serialization(format!("TLS handshake failed: {}", e)))?;
        
        // Send length prefix (4 bytes, big-endian)
        let len = bytes.len() as u32;
        let len_bytes = len.to_be_bytes();
        
        tls_stream.write_all(&len_bytes).await?;
        tls_stream.write_all(&bytes).await?;
        tls_stream.flush().await?;
        
        Ok(())
    }
    
    /// Receive a packet from a TLS stream
    ///
    /// # Arguments
    /// * `stream` - TLS stream to receive from
    ///
    /// # Returns
    /// Result containing the packet or error
    pub async fn recv_tls(stream: &mut tokio_rustls::server::TlsStream<TcpStream>) -> Result<Packet, NetworkError> {
        // Read length prefix (4 bytes, big-endian)
        let mut len_bytes = [0u8; 4];
        stream.read_exact(&mut len_bytes).await?;
        let len = u32::from_be_bytes(len_bytes) as usize;
        
        // Validate length
        if len > MAX_PACKET_SIZE {
            return Err(NetworkError::InvalidPacket(format!(
                "Packet too large: {} bytes (max: {})",
                len, MAX_PACKET_SIZE
            )));
        }
        
        // Read packet data
        let mut buffer = vec![0u8; len];
        stream.read_exact(&mut buffer).await?;
        
        // Deserialize packet
        let packet = Packet::from_msgpack(&buffer)
            .map_err(NetworkError::InvalidPacket)?;
        
        Ok(packet)
    }
    
    /// Accept incoming TLS connections
    ///
    /// # Returns
    /// Result containing (TLS stream, peer address) or error
    pub async fn accept_tls(&self) -> Result<(tokio_rustls::server::TlsStream<TcpStream>, SocketAddr), NetworkError> {
        let (stream, addr) = self.tcp_listener.accept().await?;
        
        // Perform TLS handshake
        let tls_stream = self.tls_acceptor.accept(stream).await
            .map_err(|e| NetworkError::Serialization(format!("TLS accept failed: {}", e)))?;
        
        Ok((tls_stream, addr))
    }
    
    /// Set connection timeout
    pub fn set_connection_timeout(&mut self, timeout: Duration) {
        self.connection_timeout = timeout;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordinates::NodeId;
    use crate::PoincareDiskPoint;

    #[tokio::test]
    async fn test_tls_network_layer_creation() {
        let cert = TlsCertificate::generate_self_signed("node1").unwrap();
        let result = TlsNetworkLayer::new("127.0.0.1:0", cert).await;
        
        assert!(result.is_ok());
        let layer = result.unwrap();
        assert!(layer.local_tcp_addr().port() > 0);
    }

    #[tokio::test]
    async fn test_tls_send_receive() {
        // Create two TLS network layers
        let cert1 = TlsCertificate::generate_self_signed("node1").unwrap();
        let cert2 = TlsCertificate::generate_self_signed("node2").unwrap();
        
        let layer1 = TlsNetworkLayer::new("127.0.0.1:0", cert1).await.unwrap();
        let layer2 = TlsNetworkLayer::new("127.0.0.1:0", cert2).await.unwrap();
        
        // Create a test packet
        let packet = Packet::new_data(
            NodeId::new("node1"),
            NodeId::new("node2"),
            PoincareDiskPoint::new(0.5, 0.3).unwrap(),
            b"Hello, TLS!".to_vec(),
            64,
        );
        
        // Spawn a task to accept connection on layer2
        let layer2_tcp_addr = layer2.local_tcp_addr();
        let accept_handle = tokio::spawn(async move {
            layer2.accept_tls().await
        });
        
        // Give the accept task time to start
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Send from layer1 to layer2
        layer1.send_tls(&packet, layer2_tcp_addr).await.unwrap();
        
        // Accept connection and receive packet
        let (mut stream, _) = accept_handle.await.unwrap().unwrap();
        let received_packet = TlsNetworkLayer::recv_tls(&mut stream).await.unwrap();
        
        assert_eq!(received_packet.header.source.0, "node1");
        assert_eq!(received_packet.header.destination.0, "node2");
        assert_eq!(received_packet.payload, b"Hello, TLS!");
    }

    #[tokio::test]
    async fn test_tls_multiple_packets() {
        let cert1 = TlsCertificate::generate_self_signed("node1").unwrap();
        let cert2 = TlsCertificate::generate_self_signed("node2").unwrap();
        
        let layer1 = TlsNetworkLayer::new("127.0.0.1:0", cert1).await.unwrap();
        let layer2 = TlsNetworkLayer::new("127.0.0.1:0", cert2).await.unwrap();
        
        let layer2_tcp_addr = layer2.local_tcp_addr();
        
        // Spawn accept task
        let accept_handle = tokio::spawn(async move {
            let (mut stream, _) = layer2.accept_tls().await.unwrap();
            let p1 = TlsNetworkLayer::recv_tls(&mut stream).await.unwrap();
            let p2 = TlsNetworkLayer::recv_tls(&mut stream).await.unwrap();
            (p1, p2)
        });
        
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Connect once and send two packets
        let tcp_stream = TcpStream::connect(layer2_tcp_addr).await.unwrap();
        let server_name = ServerName::try_from("localhost").unwrap();
        let mut tls_stream = layer1.tls_connector.connect(server_name, tcp_stream).await.unwrap();
        
        // Send two packets on the same TLS connection
        let packet1 = Packet::new_data(
            NodeId::new("node1"),
            NodeId::new("node2"),
            PoincareDiskPoint::origin(),
            b"First".to_vec(),
            64,
        );
        let packet2 = Packet::new_data(
            NodeId::new("node1"),
            NodeId::new("node2"),
            PoincareDiskPoint::origin(),
            b"Second".to_vec(),
            64,
        );
        
        // Send packet 1
        let bytes1 = packet1.to_msgpack().unwrap();
        let len1 = bytes1.len() as u32;
        tls_stream.write_all(&len1.to_be_bytes()).await.unwrap();
        tls_stream.write_all(&bytes1).await.unwrap();
        
        // Send packet 2
        let bytes2 = packet2.to_msgpack().unwrap();
        let len2 = bytes2.len() as u32;
        tls_stream.write_all(&len2.to_be_bytes()).await.unwrap();
        tls_stream.write_all(&bytes2).await.unwrap();
        
        tls_stream.flush().await.unwrap();
        
        // Verify both packets received
        let (p1, p2) = accept_handle.await.unwrap();
        assert_eq!(p1.payload, b"First");
        assert_eq!(p2.payload, b"Second");
    }

    #[tokio::test]
    async fn test_tls_connection_timeout() {
        let cert = TlsCertificate::generate_self_signed("node1").unwrap();
        let layer = TlsNetworkLayer::new("127.0.0.1:0", cert).await.unwrap();
        
        // Try to connect to non-existent server
        let packet = Packet::new_data(
            NodeId::new("node1"),
            NodeId::new("node2"),
            PoincareDiskPoint::origin(),
            b"Test".to_vec(),
            64,
        );
        
        // This should timeout (no server listening on this port)
        let result = layer.send_tls(&packet, "127.0.0.1:9999".parse().unwrap()).await;
        
        // Should get either timeout or connection refused
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tls_large_packet() {
        let cert1 = TlsCertificate::generate_self_signed("node1").unwrap();
        let cert2 = TlsCertificate::generate_self_signed("node2").unwrap();
        
        let layer1 = TlsNetworkLayer::new("127.0.0.1:0", cert1).await.unwrap();
        let layer2 = TlsNetworkLayer::new("127.0.0.1:0", cert2).await.unwrap();
        
        // Create a large packet (but within limits)
        let large_payload = vec![0u8; 100_000];
        let packet = Packet::new_data(
            NodeId::new("node1"),
            NodeId::new("node2"),
            PoincareDiskPoint::origin(),
            large_payload.clone(),
            64,
        );
        
        let layer2_tcp_addr = layer2.local_tcp_addr();
        
        // Spawn accept task
        let accept_handle = tokio::spawn(async move {
            layer2.accept_tls().await
        });
        
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Send large packet
        layer1.send_tls(&packet, layer2_tcp_addr).await.unwrap();
        
        // Receive and verify
        let (mut stream, _) = accept_handle.await.unwrap().unwrap();
        let received = TlsNetworkLayer::recv_tls(&mut stream).await.unwrap();
        
        assert_eq!(received.payload.len(), 100_000);
        assert_eq!(received.payload, large_payload);
    }

    #[tokio::test]
    async fn test_tls_handshake_with_different_certs() {
        // Test that nodes with different certificates can communicate
        let cert1 = TlsCertificate::generate_self_signed("node1").unwrap();
        let cert2 = TlsCertificate::generate_self_signed("node2").unwrap();
        let cert3 = TlsCertificate::generate_self_signed("node3").unwrap();
        
        let layer1 = TlsNetworkLayer::new("127.0.0.1:0", cert1).await.unwrap();
        let layer2 = TlsNetworkLayer::new("127.0.0.1:0", cert2).await.unwrap();
        let layer3 = TlsNetworkLayer::new("127.0.0.1:0", cert3).await.unwrap();
        
        // Node1 -> Node2
        let packet12 = Packet::new_data(
            NodeId::new("node1"),
            NodeId::new("node2"),
            PoincareDiskPoint::origin(),
            b"1 to 2".to_vec(),
            64,
        );
        
        let layer2_addr = layer2.local_tcp_addr();
        let accept2 = tokio::spawn(async move {
            layer2.accept_tls().await
        });
        
        tokio::time::sleep(Duration::from_millis(100)).await;
        layer1.send_tls(&packet12, layer2_addr).await.unwrap();
        
        let (mut stream2, _) = accept2.await.unwrap().unwrap();
        let received2 = TlsNetworkLayer::recv_tls(&mut stream2).await.unwrap();
        assert_eq!(received2.payload, b"1 to 2");
        
        // Node1 -> Node3
        let packet13 = Packet::new_data(
            NodeId::new("node1"),
            NodeId::new("node3"),
            PoincareDiskPoint::origin(),
            b"1 to 3".to_vec(),
            64,
        );
        
        let layer3_addr = layer3.local_tcp_addr();
        let accept3 = tokio::spawn(async move {
            layer3.accept_tls().await
        });
        
        tokio::time::sleep(Duration::from_millis(100)).await;
        layer1.send_tls(&packet13, layer3_addr).await.unwrap();
        
        let (mut stream3, _) = accept3.await.unwrap().unwrap();
        let received3 = TlsNetworkLayer::recv_tls(&mut stream3).await.unwrap();
        assert_eq!(received3.payload, b"1 to 3");
    }
}
