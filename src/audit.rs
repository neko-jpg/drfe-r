//! Audit Logging for DRFE-R Security Events
//!
//! This module provides structured audit logging for all security-relevant events
//! in the DRFE-R system using the tracing crate.

use serde::{Deserialize, Serialize};
use std::fmt;
use tracing::{error, info, warn};

/// Security event types for audit logging
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityEventType {
    /// Authentication attempt (success or failure)
    Authentication,
    /// Packet signature verification
    SignatureVerification,
    /// Rate limit enforcement
    RateLimit,
    /// TLS connection establishment
    TlsConnection,
    /// Malicious packet detection
    MaliciousPacket,
    /// Node join/leave events
    NodeLifecycle,
    /// Coordinate update events
    CoordinateUpdate,
    /// API access events
    ApiAccess,
    /// Configuration changes
    ConfigurationChange,
}

impl fmt::Display for SecurityEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SecurityEventType::Authentication => write!(f, "AUTHENTICATION"),
            SecurityEventType::SignatureVerification => write!(f, "SIGNATURE_VERIFICATION"),
            SecurityEventType::RateLimit => write!(f, "RATE_LIMIT"),
            SecurityEventType::TlsConnection => write!(f, "TLS_CONNECTION"),
            SecurityEventType::MaliciousPacket => write!(f, "MALICIOUS_PACKET"),
            SecurityEventType::NodeLifecycle => write!(f, "NODE_LIFECYCLE"),
            SecurityEventType::CoordinateUpdate => write!(f, "COORDINATE_UPDATE"),
            SecurityEventType::ApiAccess => write!(f, "API_ACCESS"),
            SecurityEventType::ConfigurationChange => write!(f, "CONFIGURATION_CHANGE"),
        }
    }
}

/// Audit event outcome
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditOutcome {
    /// Event succeeded
    Success,
    /// Event failed
    Failure,
    /// Event was denied/rejected
    Denied,
}

impl fmt::Display for AuditOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuditOutcome::Success => write!(f, "SUCCESS"),
            AuditOutcome::Failure => write!(f, "FAILURE"),
            AuditOutcome::Denied => write!(f, "DENIED"),
        }
    }
}

/// Audit logger for security events
pub struct AuditLogger;

impl AuditLogger {
    /// Log an authentication event
    ///
    /// # Arguments
    /// * `node_id` - The node ID attempting authentication
    /// * `outcome` - Whether authentication succeeded or failed
    /// * `reason` - Optional reason for failure
    pub fn log_authentication(node_id: &str, outcome: AuditOutcome, reason: Option<&str>) {
        match outcome {
            AuditOutcome::Success => {
                info!(
                    event_type = %SecurityEventType::Authentication,
                    outcome = %outcome,
                    node_id = %node_id,
                    "Authentication successful"
                );
            }
            AuditOutcome::Failure | AuditOutcome::Denied => {
                warn!(
                    event_type = %SecurityEventType::Authentication,
                    outcome = %outcome,
                    node_id = %node_id,
                    reason = ?reason,
                    "Authentication failed"
                );
            }
        }
    }

    /// Log a signature verification event
    ///
    /// # Arguments
    /// * `packet_id` - The packet ID being verified
    /// * `source_node` - The claimed source node
    /// * `outcome` - Whether verification succeeded or failed
    /// * `reason` - Optional reason for failure
    pub fn log_signature_verification(
        packet_id: &str,
        source_node: &str,
        outcome: AuditOutcome,
        reason: Option<&str>,
    ) {
        match outcome {
            AuditOutcome::Success => {
                info!(
                    event_type = %SecurityEventType::SignatureVerification,
                    outcome = %outcome,
                    packet_id = %packet_id,
                    source_node = %source_node,
                    "Signature verification successful"
                );
            }
            AuditOutcome::Failure | AuditOutcome::Denied => {
                warn!(
                    event_type = %SecurityEventType::SignatureVerification,
                    outcome = %outcome,
                    packet_id = %packet_id,
                    source_node = %source_node,
                    reason = ?reason,
                    "Signature verification failed"
                );
            }
        }
    }

    /// Log a rate limit event
    ///
    /// # Arguments
    /// * `client_id` - The client being rate limited
    /// * `endpoint` - The API endpoint being accessed
    /// * `outcome` - Whether request was allowed or denied
    pub fn log_rate_limit(client_id: &str, endpoint: &str, outcome: AuditOutcome) {
        match outcome {
            AuditOutcome::Denied => {
                warn!(
                    event_type = %SecurityEventType::RateLimit,
                    outcome = %outcome,
                    client_id = %client_id,
                    endpoint = %endpoint,
                    "Rate limit exceeded"
                );
            }
            AuditOutcome::Success => {
                info!(
                    event_type = %SecurityEventType::RateLimit,
                    outcome = %outcome,
                    client_id = %client_id,
                    endpoint = %endpoint,
                    "Request allowed"
                );
            }
            _ => {}
        }
    }

    /// Log a TLS connection event
    ///
    /// # Arguments
    /// * `peer_addr` - The peer address
    /// * `outcome` - Whether connection succeeded or failed
    /// * `reason` - Optional reason for failure
    pub fn log_tls_connection(peer_addr: &str, outcome: AuditOutcome, reason: Option<&str>) {
        match outcome {
            AuditOutcome::Success => {
                info!(
                    event_type = %SecurityEventType::TlsConnection,
                    outcome = %outcome,
                    peer_addr = %peer_addr,
                    "TLS connection established"
                );
            }
            AuditOutcome::Failure => {
                warn!(
                    event_type = %SecurityEventType::TlsConnection,
                    outcome = %outcome,
                    peer_addr = %peer_addr,
                    reason = ?reason,
                    "TLS connection failed"
                );
            }
            _ => {}
        }
    }

    /// Log a malicious packet detection event
    ///
    /// # Arguments
    /// * `packet_id` - The packet ID
    /// * `source_node` - The claimed source node
    /// * `violation_type` - Type of violation detected
    /// * `details` - Additional details about the violation
    pub fn log_malicious_packet(
        packet_id: &str,
        source_node: &str,
        violation_type: &str,
        details: &str,
    ) {
        error!(
            event_type = %SecurityEventType::MaliciousPacket,
            outcome = %AuditOutcome::Denied,
            packet_id = %packet_id,
            source_node = %source_node,
            violation_type = %violation_type,
            details = %details,
            "Malicious packet detected and rejected"
        );
    }

    /// Log a node lifecycle event (join/leave)
    ///
    /// # Arguments
    /// * `node_id` - The node ID
    /// * `event` - The lifecycle event ("join" or "leave")
    /// * `outcome` - Whether the event succeeded
    pub fn log_node_lifecycle(node_id: &str, event: &str, outcome: AuditOutcome) {
        match outcome {
            AuditOutcome::Success => {
                info!(
                    event_type = %SecurityEventType::NodeLifecycle,
                    outcome = %outcome,
                    node_id = %node_id,
                    event = %event,
                    "Node lifecycle event"
                );
            }
            AuditOutcome::Failure => {
                warn!(
                    event_type = %SecurityEventType::NodeLifecycle,
                    outcome = %outcome,
                    node_id = %node_id,
                    event = %event,
                    "Node lifecycle event failed"
                );
            }
            _ => {}
        }
    }

    /// Log a coordinate update event
    ///
    /// # Arguments
    /// * `node_id` - The node ID
    /// * `old_version` - Previous coordinate version
    /// * `new_version` - New coordinate version
    /// * `outcome` - Whether update succeeded
    pub fn log_coordinate_update(
        node_id: &str,
        old_version: u64,
        new_version: u64,
        outcome: AuditOutcome,
    ) {
        info!(
            event_type = %SecurityEventType::CoordinateUpdate,
            outcome = %outcome,
            node_id = %node_id,
            old_version = %old_version,
            new_version = %new_version,
            "Coordinate update"
        );
    }

    /// Log an API access event
    ///
    /// # Arguments
    /// * `client_id` - The client making the request
    /// * `method` - HTTP method
    /// * `endpoint` - API endpoint
    /// * `outcome` - Whether request succeeded
    /// * `status_code` - HTTP status code
    pub fn log_api_access(
        client_id: &str,
        method: &str,
        endpoint: &str,
        outcome: AuditOutcome,
        status_code: u16,
    ) {
        match outcome {
            AuditOutcome::Success => {
                info!(
                    event_type = %SecurityEventType::ApiAccess,
                    outcome = %outcome,
                    client_id = %client_id,
                    method = %method,
                    endpoint = %endpoint,
                    status_code = %status_code,
                    "API access"
                );
            }
            AuditOutcome::Failure | AuditOutcome::Denied => {
                warn!(
                    event_type = %SecurityEventType::ApiAccess,
                    outcome = %outcome,
                    client_id = %client_id,
                    method = %method,
                    endpoint = %endpoint,
                    status_code = %status_code,
                    "API access failed or denied"
                );
            }
        }
    }

    /// Log a configuration change event
    ///
    /// # Arguments
    /// * `changed_by` - Who made the change
    /// * `setting` - What setting was changed
    /// * `old_value` - Previous value
    /// * `new_value` - New value
    pub fn log_configuration_change(
        changed_by: &str,
        setting: &str,
        old_value: &str,
        new_value: &str,
    ) {
        info!(
            event_type = %SecurityEventType::ConfigurationChange,
            outcome = %AuditOutcome::Success,
            changed_by = %changed_by,
            setting = %setting,
            old_value = %old_value,
            new_value = %new_value,
            "Configuration changed"
        );
    }
}

/// Initialize audit logging with file rotation
///
/// # Arguments
/// * `log_dir` - Directory to store log files
/// * `_max_file_size` - Maximum size of each log file in bytes (currently unused, rotation is daily)
/// * `max_files` - Maximum number of log files to keep
///
/// # Returns
/// Result indicating success or error
pub fn init_audit_logging(
    log_dir: &str,
    _max_file_size: u64,
    max_files: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    use tracing_subscriber::fmt::format::FmtSpan;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    // Create log directory if it doesn't exist
    std::fs::create_dir_all(log_dir)?;

    // Create file appender with rotation
    let file_appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("drfe-r-audit")
        .filename_suffix("log")
        .max_log_files(max_files)
        .build(log_dir)?;

    // Create non-blocking writer
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Create subscriber with JSON formatting for structured logs
    let subscriber = tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_span_events(FmtSpan::CLOSE)
                .with_writer(non_blocking),
        )
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        );

    // Try to set as global default, but don't fail if already set
    let _ = subscriber.try_init();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_event_type_display() {
        assert_eq!(
            format!("{}", SecurityEventType::Authentication),
            "AUTHENTICATION"
        );
        assert_eq!(
            format!("{}", SecurityEventType::SignatureVerification),
            "SIGNATURE_VERIFICATION"
        );
        assert_eq!(format!("{}", SecurityEventType::RateLimit), "RATE_LIMIT");
        assert_eq!(
            format!("{}", SecurityEventType::TlsConnection),
            "TLS_CONNECTION"
        );
        assert_eq!(
            format!("{}", SecurityEventType::MaliciousPacket),
            "MALICIOUS_PACKET"
        );
        assert_eq!(
            format!("{}", SecurityEventType::NodeLifecycle),
            "NODE_LIFECYCLE"
        );
        assert_eq!(
            format!("{}", SecurityEventType::CoordinateUpdate),
            "COORDINATE_UPDATE"
        );
        assert_eq!(format!("{}", SecurityEventType::ApiAccess), "API_ACCESS");
        assert_eq!(
            format!("{}", SecurityEventType::ConfigurationChange),
            "CONFIGURATION_CHANGE"
        );
    }

    #[test]
    fn test_audit_outcome_display() {
        assert_eq!(format!("{}", AuditOutcome::Success), "SUCCESS");
        assert_eq!(format!("{}", AuditOutcome::Failure), "FAILURE");
        assert_eq!(format!("{}", AuditOutcome::Denied), "DENIED");
    }

    #[test]
    fn test_audit_logger_authentication() {
        // Initialize test subscriber
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .try_init();

        // Test successful authentication
        AuditLogger::log_authentication("node1", AuditOutcome::Success, None);

        // Test failed authentication
        AuditLogger::log_authentication(
            "node2",
            AuditOutcome::Failure,
            Some("Invalid signature"),
        );
    }

    #[test]
    fn test_audit_logger_signature_verification() {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .try_init();

        AuditLogger::log_signature_verification(
            "packet123",
            "node1",
            AuditOutcome::Success,
            None,
        );

        AuditLogger::log_signature_verification(
            "packet456",
            "node2",
            AuditOutcome::Failure,
            Some("Invalid signature"),
        );
    }

    #[test]
    fn test_audit_logger_rate_limit() {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .try_init();

        AuditLogger::log_rate_limit("client1", "/api/v1/packets", AuditOutcome::Success);
        AuditLogger::log_rate_limit("client2", "/api/v1/packets", AuditOutcome::Denied);
    }

    #[test]
    fn test_audit_logger_tls_connection() {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .try_init();

        AuditLogger::log_tls_connection("192.168.1.1:7778", AuditOutcome::Success, None);
        AuditLogger::log_tls_connection(
            "192.168.1.2:7778",
            AuditOutcome::Failure,
            Some("Handshake failed"),
        );
    }

    #[test]
    fn test_audit_logger_malicious_packet() {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .try_init();

        AuditLogger::log_malicious_packet(
            "packet789",
            "node3",
            "TTL_MANIPULATION",
            "TTL exceeds maximum allowed value",
        );
    }

    #[test]
    fn test_audit_logger_node_lifecycle() {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .try_init();

        AuditLogger::log_node_lifecycle("node1", "join", AuditOutcome::Success);
        AuditLogger::log_node_lifecycle("node2", "leave", AuditOutcome::Success);
    }

    #[test]
    fn test_audit_logger_coordinate_update() {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .try_init();

        AuditLogger::log_coordinate_update("node1", 1, 2, AuditOutcome::Success);
    }

    #[test]
    fn test_audit_logger_api_access() {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .try_init();

        AuditLogger::log_api_access(
            "client1",
            "POST",
            "/api/v1/packets",
            AuditOutcome::Success,
            200,
        );
        AuditLogger::log_api_access(
            "client2",
            "GET",
            "/api/v1/nodes/node1",
            AuditOutcome::Denied,
            403,
        );
    }

    #[test]
    fn test_audit_logger_configuration_change() {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .try_init();

        AuditLogger::log_configuration_change("admin", "max_ttl", "64", "128");
    }
}
