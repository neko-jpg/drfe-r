//! TLS Configuration and Certificate Management for DRFE-R
//!
//! This module provides TLS encryption for all inter-node communication using rustls.
//! It handles certificate generation, validation, and TLS configuration.

use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::{ClientConfig, ServerConfig};
use std::io::{BufReader, Cursor};
use std::sync::Arc;
use thiserror::Error;

/// TLS-related errors
#[derive(Error, Debug)]
pub enum TlsError {
    #[error("Certificate generation error: {0}")]
    CertGeneration(String),
    
    #[error("Certificate parsing error: {0}")]
    CertParsing(String),
    
    #[error("TLS configuration error: {0}")]
    Configuration(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Certificate and private key pair
#[derive(Clone)]
pub struct TlsCertificate {
    /// DER-encoded certificate
    pub cert: Vec<u8>,
    /// DER-encoded private key
    pub key: Vec<u8>,
}

impl TlsCertificate {
    /// Generate a new self-signed certificate for a node
    ///
    /// # Arguments
    /// * `node_id` - The node ID to use as the certificate subject
    ///
    /// # Returns
    /// Result containing the certificate and private key
    pub fn generate_self_signed(node_id: &str) -> Result<Self, TlsError> {
        // Generate a new key pair
        let mut params = rcgen::CertificateParams::new(vec![node_id.to_string()]);
        
        // Set certificate validity period (1 year)
        params.not_before = rcgen::date_time_ymd(2024, 1, 1);
        params.not_after = rcgen::date_time_ymd(2025, 1, 1);
        
        // Set subject alternative names
        params.subject_alt_names = vec![
            rcgen::SanType::DnsName(node_id.to_string()),
        ];
        
        // Generate certificate
        let cert = rcgen::Certificate::from_params(params)
            .map_err(|e| TlsError::CertGeneration(e.to_string()))?;
        
        // Serialize certificate and key to DER format
        let cert_der = cert.serialize_der()
            .map_err(|e| TlsError::CertGeneration(e.to_string()))?;
        let key_der = cert.serialize_private_key_der();
        
        Ok(Self {
            cert: cert_der,
            key: key_der,
        })
    }
    
    /// Load certificate from PEM-encoded bytes
    pub fn from_pem(cert_pem: &[u8], key_pem: &[u8]) -> Result<Self, TlsError> {
        // Parse certificate
        let mut cert_reader = BufReader::new(Cursor::new(cert_pem));
        let certs = rustls_pemfile::certs(&mut cert_reader)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| TlsError::CertParsing(e.to_string()))?;
        
        if certs.is_empty() {
            return Err(TlsError::CertParsing("No certificates found".to_string()));
        }
        
        // Parse private key
        let mut key_reader = BufReader::new(Cursor::new(key_pem));
        let keys = rustls_pemfile::private_key(&mut key_reader)
            .map_err(|e| TlsError::CertParsing(e.to_string()))?
            .ok_or_else(|| TlsError::CertParsing("No private key found".to_string()))?;
        
        Ok(Self {
            cert: certs[0].to_vec(),
            key: keys.secret_der().to_vec(),
        })
    }
    
    /// Convert to PEM format
    pub fn to_pem(&self) -> Result<(String, String), TlsError> {
        // Convert certificate to PEM
        let cert_pem = pem::Pem::new("CERTIFICATE", self.cert.clone());
        let cert_pem_str = pem::encode(&cert_pem);
        
        // Convert key to PEM
        let key_pem = pem::Pem::new("PRIVATE KEY", self.key.clone());
        let key_pem_str = pem::encode(&key_pem);
        
        Ok((cert_pem_str, key_pem_str))
    }
}

/// TLS configuration for DRFE-R nodes
pub struct TlsConfig {
    /// Server configuration (for accepting connections)
    server_config: Arc<ServerConfig>,
    /// Client configuration (for initiating connections)
    client_config: Arc<ClientConfig>,
}

impl TlsConfig {
    /// Create a new TLS configuration with a certificate
    ///
    /// # Arguments
    /// * `certificate` - The certificate and private key to use
    ///
    /// # Returns
    /// Result containing the TLS configuration
    pub fn new(certificate: TlsCertificate) -> Result<Self, TlsError> {
        // Create server config
        let cert_der = CertificateDer::from(certificate.cert.clone());
        let key_der = PrivateKeyDer::try_from(certificate.key.clone())
            .map_err(|e| TlsError::Configuration(format!("Invalid private key: {:?}", e)))?;
        
        let server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der.clone()], key_der.clone_key())
            .map_err(|e| TlsError::Configuration(e.to_string()))?;
        
        // Create client config (accept any certificate for now - in production, use proper CA)
        let mut client_config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifier))
            .with_no_client_auth();
        
        // Enable session resumption
        client_config.resumption = rustls::client::Resumption::default();
        
        Ok(Self {
            server_config: Arc::new(server_config),
            client_config: Arc::new(client_config),
        })
    }
    
    /// Get server configuration
    pub fn server_config(&self) -> Arc<ServerConfig> {
        Arc::clone(&self.server_config)
    }
    
    /// Get client configuration
    pub fn client_config(&self) -> Arc<ClientConfig> {
        Arc::clone(&self.client_config)
    }
}

/// Custom certificate verifier that accepts any certificate
/// WARNING: This is insecure and should only be used for testing/development
/// In production, implement proper certificate validation
#[derive(Debug)]
struct NoVerifier;

impl rustls::client::danger::ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        // Accept any certificate (insecure!)
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    
    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    
    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ED25519,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_self_signed_certificate() {
        let cert = TlsCertificate::generate_self_signed("node1").unwrap();
        
        assert!(!cert.cert.is_empty());
        assert!(!cert.key.is_empty());
    }

    #[test]
    fn test_certificate_pem_roundtrip() {
        let cert = TlsCertificate::generate_self_signed("node1").unwrap();
        
        let (cert_pem, key_pem) = cert.to_pem().unwrap();
        
        assert!(cert_pem.contains("BEGIN CERTIFICATE"));
        assert!(key_pem.contains("BEGIN PRIVATE KEY"));
        
        let loaded = TlsCertificate::from_pem(cert_pem.as_bytes(), key_pem.as_bytes()).unwrap();
        
        assert_eq!(loaded.cert, cert.cert);
        assert_eq!(loaded.key, cert.key);
    }

    #[test]
    fn test_tls_config_creation() {
        let cert = TlsCertificate::generate_self_signed("node1").unwrap();
        let config = TlsConfig::new(cert).unwrap();
        
        // Verify configs are created
        assert!(Arc::strong_count(&config.server_config()) >= 1);
        assert!(Arc::strong_count(&config.client_config()) >= 1);
    }

    #[test]
    fn test_multiple_certificates() {
        let cert1 = TlsCertificate::generate_self_signed("node1").unwrap();
        let cert2 = TlsCertificate::generate_self_signed("node2").unwrap();
        
        // Certificates should be different
        assert_ne!(cert1.cert, cert2.cert);
        assert_ne!(cert1.key, cert2.key);
    }
}
