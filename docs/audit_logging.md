# Audit Logging in DRFE-R

## Overview

DRFE-R includes comprehensive audit logging for all security-relevant events using the `tracing` crate. Logs are structured in JSON format and support automatic rotation.

## Features

- **Structured Logging**: All logs are in JSON format for easy parsing and analysis
- **Log Rotation**: Automatic daily rotation with configurable retention
- **Security Events**: Comprehensive coverage of authentication, authorization, and security events
- **Non-blocking I/O**: Asynchronous log writing for minimal performance impact

## Initialization

Initialize audit logging at application startup:

```rust
use drfe_r::audit::init_audit_logging;

// Initialize with log directory, max file size, and max files to keep
init_audit_logging("./logs", 10_000_000, 5)?;
```

Parameters:
- `log_dir`: Directory to store log files
- `max_file_size`: Maximum size per log file (currently unused, rotation is daily)
- `max_files`: Maximum number of log files to retain

## Security Event Types

The following security events are logged:

### 1. Authentication Events

```rust
use drfe_r::audit::{AuditLogger, AuditOutcome};

// Successful authentication
AuditLogger::log_authentication("node1", AuditOutcome::Success, None);

// Failed authentication
AuditLogger::log_authentication(
    "node2",
    AuditOutcome::Failure,
    Some("Invalid signature")
);
```

### 2. Signature Verification

```rust
// Successful verification
AuditLogger::log_signature_verification(
    "packet123",
    "node1",
    AuditOutcome::Success,
    None
);

// Failed verification
AuditLogger::log_signature_verification(
    "packet456",
    "node2",
    AuditOutcome::Failure,
    Some("Signature mismatch")
);
```

### 3. Rate Limiting

```rust
// Request allowed
AuditLogger::log_rate_limit("client1", "/api/v1/packets", AuditOutcome::Success);

// Request denied (rate limit exceeded)
AuditLogger::log_rate_limit("client2", "/api/v1/packets", AuditOutcome::Denied);
```

### 4. TLS Connections

```rust
// Successful connection
AuditLogger::log_tls_connection("192.168.1.1:7778", AuditOutcome::Success, None);

// Failed connection
AuditLogger::log_tls_connection(
    "192.168.1.2:7778",
    AuditOutcome::Failure,
    Some("Handshake timeout")
);
```

### 5. Malicious Packet Detection

```rust
AuditLogger::log_malicious_packet(
    "packet789",
    "node3",
    "TTL_MANIPULATION",
    "TTL value exceeds maximum allowed (300 > 255)"
);
```

### 6. Node Lifecycle Events

```rust
// Node join
AuditLogger::log_node_lifecycle("node1", "join", AuditOutcome::Success);

// Node leave
AuditLogger::log_node_lifecycle("node2", "leave", AuditOutcome::Success);
```

### 7. Coordinate Updates

```rust
AuditLogger::log_coordinate_update("node1", 1, 2, AuditOutcome::Success);
```

### 8. API Access

```rust
// Successful API access
AuditLogger::log_api_access(
    "client1",
    "POST",
    "/api/v1/packets",
    AuditOutcome::Success,
    200
);

// Denied API access
AuditLogger::log_api_access(
    "client2",
    "GET",
    "/api/v1/nodes/node1",
    AuditOutcome::Denied,
    403
);
```

### 9. Configuration Changes

```rust
AuditLogger::log_configuration_change("admin", "max_ttl", "64", "128");
```

## Log Format

Logs are written in JSON format with the following structure:

```json
{
  "timestamp": "2026-01-01T12:00:00.000Z",
  "level": "INFO",
  "event_type": "AUTHENTICATION",
  "outcome": "SUCCESS",
  "node_id": "node1",
  "message": "Authentication successful"
}
```

## Log Rotation

Logs are automatically rotated daily. The rotation configuration:
- **Rotation**: Daily (at midnight)
- **Filename pattern**: `drfe-r-audit.YYYY-MM-DD.log`
- **Retention**: Configurable (default: 5 files)

## Integration Examples

### In Network Layer

```rust
// When verifying packet signatures
if packet.verify_signature(&public_key) {
    AuditLogger::log_signature_verification(
        &packet.header.packet_id,
        &packet.header.source.0,
        AuditOutcome::Success,
        None
    );
} else {
    AuditLogger::log_signature_verification(
        &packet.header.packet_id,
        &packet.header.source.0,
        AuditOutcome::Failure,
        Some("Invalid signature")
    );
}
```

### In API Layer

```rust
// Log API access
AuditLogger::log_api_access(
    &client_id,
    "POST",
    "/api/v1/packets",
    if result.is_ok() { AuditOutcome::Success } else { AuditOutcome::Failure },
    status_code
);
```

### In TLS Layer

```rust
// Log TLS connection establishment
match tls_stream.await {
    Ok(_) => {
        AuditLogger::log_tls_connection(
            &peer_addr.to_string(),
            AuditOutcome::Success,
            None
        );
    }
    Err(e) => {
        AuditLogger::log_tls_connection(
            &peer_addr.to_string(),
            AuditOutcome::Failure,
            Some(&e.to_string())
        );
    }
}
```

## Monitoring and Analysis

### Viewing Logs

```bash
# View latest log file
tail -f logs/drfe-r-audit.*.log

# Parse JSON logs with jq
cat logs/drfe-r-audit.*.log | jq '.event_type, .outcome, .message'

# Filter by event type
cat logs/drfe-r-audit.*.log | jq 'select(.event_type == "AUTHENTICATION")'

# Count failed events
cat logs/drfe-r-audit.*.log | jq 'select(.outcome == "FAILURE")' | wc -l
```

### Security Monitoring

Monitor for suspicious activity:

```bash
# Failed authentication attempts
cat logs/*.log | jq 'select(.event_type == "AUTHENTICATION" and .outcome == "FAILURE")'

# Malicious packet detections
cat logs/*.log | jq 'select(.event_type == "MALICIOUS_PACKET")'

# Rate limit violations
cat logs/*.log | jq 'select(.event_type == "RATE_LIMIT" and .outcome == "DENIED")'
```

## Performance Considerations

- Audit logging uses non-blocking I/O to minimize performance impact
- Logs are buffered and written asynchronously
- JSON formatting adds minimal overhead
- Log rotation prevents disk space issues

## Best Practices

1. **Initialize Early**: Call `init_audit_logging()` at application startup
2. **Log All Security Events**: Use appropriate log functions for all security-relevant operations
3. **Include Context**: Provide detailed reason strings for failures
4. **Monitor Regularly**: Set up automated monitoring for security events
5. **Rotate Logs**: Configure appropriate retention based on compliance requirements
6. **Secure Log Files**: Ensure log directory has appropriate permissions

## Compliance

The audit logging system is designed to meet security compliance requirements:

- **Requirement 14.5**: Provides audit logs for security monitoring
- **Structured Format**: JSON format enables automated analysis
- **Comprehensive Coverage**: All security-relevant events are logged
- **Tamper Resistance**: Logs are append-only with rotation
- **Retention**: Configurable retention period for compliance

## Testing

Run audit logging tests:

```bash
cargo test --test audit_logging_tests
```

All tests verify:
- Log file creation
- Log format and content
- Event type coverage
- Concurrent logging
- Log rotation
- Directory creation
