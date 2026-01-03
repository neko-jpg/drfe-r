# DRFE-R Wire Protocol Specification

## Version 1.0

This document specifies the wire protocol for communication between DRFE-R distributed nodes.

## Overview

The DRFE-R wire protocol uses **MessagePack** for efficient binary serialization. All packets are self-contained and include complete routing metadata.

## Design Principles

1. **Efficiency**: MessagePack provides compact binary encoding
2. **Extensibility**: Protocol version field allows future upgrades
3. **Security**: Optional signature field for authentication
4. **Simplicity**: Single packet format for all message types

## Packet Structure

### Top-Level Packet

```rust
struct Packet {
    header: NetworkPacketHeader,
    payload: Vec<u8>,
    signature: Option<Vec<u8>>,  // Optional Ed25519 signature (64 bytes)
}
```

### Network Packet Header

```rust
struct NetworkPacketHeader {
    version: u8,                              // Protocol version (currently 1)
    packet_type: PacketType,                  // Type of packet
    source: NodeId,                           // Source node ID (String)
    destination: NodeId,                      // Destination node ID (String)
    target_coord: SerializablePoincareDiskPoint,  // Target coordinate for routing
    mode: RoutingMode,                        // Current routing mode
    ttl: u32,                                 // Time-to-live (max 255)
    timestamp: u64,                           // Unix timestamp in milliseconds
    packet_id: String,                        // Unique packet identifier
    visited: HashSet<String>,                 // Set of visited node IDs
    pressure_values: HashMap<String, f64>,    // Pressure values for visited nodes
    recovery_threshold: f64,                  // Distance threshold for mode switching
    pressure_budget: u32,                     // Remaining pressure mode steps
}
```

### Packet Types

```rust
enum PacketType {
    Data,              // Application data packet
    Heartbeat,         // Liveness check
    Discovery,         // Neighbor discovery
    CoordinateUpdate,  // Coordinate broadcast
    Ack,              // Acknowledgment
}
```

### Routing Modes

```rust
enum RoutingMode {
    Gravity,   // Greedy forwarding mode
    Pressure,  // Local minimum escape mode
    Tree,      // Spanning tree fallback mode
}
```

### Coordinate Representation

```rust
struct SerializablePoincareDiskPoint {
    x: f64,  // X coordinate in Poincaré disk
    y: f64,  // Y coordinate in Poincaré disk
}
```

## Packet Types in Detail

### 1. Data Packet

Used for routing application payloads between nodes.

**Fields:**
- `packet_type`: `Data`
- `source`: Originating node ID
- `destination`: Target node ID
- `target_coord`: Destination's hyperbolic coordinate
- `payload`: Application data (arbitrary bytes)
- `ttl`: Maximum hops allowed (typically 64-255)

**Example:**
```rust
let packet = Packet::new_data(
    NodeId::new("alice"),
    NodeId::new("bob"),
    bob_coordinate,
    b"Hello, Bob!".to_vec(),
    64
);
```

### 2. Heartbeat Packet

Used for neighbor liveness detection.

**Fields:**
- `packet_type`: `Heartbeat`
- `source`: Sending node ID
- `destination`: Neighbor node ID
- `ttl`: 1 (single hop)
- `payload`: Empty

**Frequency:** Sent every 1 second to each neighbor

**Timeout:** Node considered failed after 5 missed heartbeats (5 seconds)

**Example:**
```rust
let packet = Packet::new_heartbeat(
    NodeId::new("node1"),
    NodeId::new("node2")
);
```

### 3. Discovery Packet

Used for neighbor discovery in local network.

**Fields:**
- `packet_type`: `Discovery`
- `source`: Discovering node ID
- `destination`: `"broadcast"` (special broadcast ID)
- `target_coord`: Source node's coordinate
- `ttl`: 1 (single hop)
- `payload`: Serialized source coordinate

**Mechanism:** 
- Broadcast to local network
- Neighbors respond with their coordinates
- Node selects k nearest neighbors in hyperbolic space

**Example:**
```rust
let packet = Packet::new_discovery(
    NodeId::new("node1"),
    my_coordinate
);
```

### 4. Coordinate Update Packet

Used to broadcast coordinate changes to neighbors.

**Fields:**
- `packet_type`: `CoordinateUpdate`
- `source`: Node with updated coordinate
- `destination`: `"broadcast"`
- `target_coord`: New coordinate
- `ttl`: 1 (single hop)
- `payload`: Serialized (coordinate, version) tuple

**Trigger Conditions:**
- Periodic updates (every 60 seconds)
- Topology change detected
- Ricci flow optimization completed

**Example:**
```rust
let packet = Packet::new_coordinate_update(
    NodeId::new("node1"),
    new_coordinate,
    version_number
);
```

### 5. Acknowledgment Packet

Used to confirm packet receipt (optional, for reliability).

**Fields:**
- `packet_type`: `Ack`
- `source`: Acknowledging node
- `destination`: Original sender
- `payload`: Original packet ID

## Serialization Format

### MessagePack Encoding

All packets are serialized using MessagePack (via `rmp-serde` crate).

**Advantages:**
- Compact binary format (smaller than JSON)
- Fast serialization/deserialization
- Schema-less (self-describing)
- Wide language support

**Example:**
```rust
// Serialize
let bytes = packet.to_msgpack()?;

// Deserialize
let packet = Packet::from_msgpack(&bytes)?;
```

### Size Limits

- **Maximum packet size**: 1 MB (1,048,576 bytes)
- **Typical data packet**: 200-500 bytes
- **Heartbeat packet**: ~100 bytes
- **Discovery packet**: ~150 bytes

## Routing Metadata

### Visited Set

Tracks nodes visited during routing to prevent loops and calculate pressure.

**Format:** `HashSet<String>` of node IDs

**Usage:**
- Gravity mode: Not used
- Pressure mode: Used to calculate pressure values
- Tree mode: Used to prevent revisiting nodes during DFS

### Pressure Values

Maps node IDs to accumulated pressure values.

**Format:** `HashMap<String, f64>`

**Calculation:**
- Increment: +5.0 per visit
- Decay: ×0.95 per hop
- Used to make frequently visited nodes less attractive

### Recovery Threshold

Distance threshold for escaping local minima.

**Type:** `f64`

**Usage:**
- Set when entering Pressure/Tree mode
- Routing returns to Gravity mode when current distance < threshold
- Ensures forward progress

### Pressure Budget

Maximum steps allowed in Pressure mode before falling back to Tree mode.

**Type:** `u32`

**Default:** `N/2` where N is network size

**Purpose:** Prevent infinite loops in Pressure mode

## Security

### Packet Signing (Optional)

Packets can be signed using Ed25519 cryptography.

**Signature Field:**
- Type: `Option<Vec<u8>>`
- Size: 64 bytes (when present)
- Algorithm: Ed25519

**Process:**
1. Serialize packet (excluding signature field)
2. Sign serialized bytes with private key
3. Attach signature to packet

**Verification:**
1. Extract signature from packet
2. Serialize packet (excluding signature)
3. Verify signature using sender's public key

**Note:** Signature implementation is currently a placeholder and needs to be completed in Phase 5 (Security Implementation).

### Authentication

Node IDs can be derived from public keys:
```
NodeId = Base58(SHA256(PublicKey))
```

This ensures that only the holder of the private key can send packets from that node.

## Error Handling

### Deserialization Errors

If a packet cannot be deserialized:
1. Log error with packet source (if available)
2. Drop packet silently
3. Do not send error response (prevents amplification attacks)

### Size Limit Violations

If a packet exceeds MAX_PACKET_SIZE:
1. Reject during deserialization
2. Return error: "Packet too large: X bytes (max: 1048576)"
3. Do not process packet

### Invalid Coordinates

If target_coord is outside Poincaré disk (|z| >= 1):
1. Clamp to valid range: z' = z / (|z| + ε)
2. Log warning
3. Continue routing with clamped coordinate

### TTL Expiration

If TTL reaches 0:
1. Drop packet
2. Optionally send error response to source
3. Log routing failure

## Protocol Versioning

### Current Version: 1

**Version Field:** `u8` in packet header

**Compatibility:**
- Nodes MUST reject packets with unknown versions
- Future versions MAY add optional fields
- Breaking changes require new version number

**Version Negotiation:**
- Discovery packets include protocol version
- Nodes only communicate with compatible versions
- Version mismatch logged as warning

## Transport Layer

### Supported Transports

1. **UDP** (recommended for low latency)
   - Port: 7777 (default)
   - No reliability guarantees
   - Suitable for heartbeats and discovery

2. **TCP** (recommended for data packets)
   - Port: 7778 (default)
   - Reliable delivery
   - Connection-oriented

3. **QUIC** (future)
   - Combines UDP speed with TCP reliability
   - Built-in encryption

### Packet Framing (TCP)

For TCP, packets are framed with length prefix:

```
[4 bytes: packet length (big-endian u32)][N bytes: MessagePack packet]
```

This allows multiple packets on a single TCP stream.

## Performance Considerations

### Packet Size Optimization

- Use compact node IDs (hashes, not full names)
- Limit visited set size (e.g., max 100 nodes)
- Compress large payloads (optional)

### Batching

Multiple small packets can be batched:
```rust
struct BatchPacket {
    packets: Vec<Packet>,
}
```

### Caching

Nodes should cache:
- Neighbor coordinates (updated on CoordinateUpdate)
- Recent packet IDs (for deduplication)
- Routing decisions (for fast forwarding)

## Example Packet Flow

### Scenario: Alice sends message to Bob

1. **Alice creates data packet:**
   ```
   Packet {
     header: {
       packet_type: Data,
       source: "alice",
       destination: "bob",
       target_coord: (0.5, 0.3),
       mode: Gravity,
       ttl: 64,
       ...
     },
     payload: b"Hello, Bob!",
     signature: None
   }
   ```

2. **Alice serializes to MessagePack:**
   ```
   bytes = [0x83, 0xa6, 0x68, 0x65, 0x61, 0x64, 0x65, 0x72, ...]
   ```

3. **Alice sends to nearest neighbor (Charlie):**
   - UDP datagram to Charlie's IP:7777
   - Or TCP connection to Charlie's IP:7778

4. **Charlie receives and deserializes:**
   ```rust
   let packet = Packet::from_msgpack(&bytes)?;
   ```

5. **Charlie routes packet:**
   - Checks if destination is self (no)
   - Decrements TTL (63)
   - Finds next hop using GP algorithm (Dave)
   - Forwards to Dave

6. **Process repeats until packet reaches Bob**

7. **Bob receives packet:**
   - Checks destination (matches!)
   - Delivers payload to application
   - Optionally sends Ack to Alice

## Implementation Notes

### Rust Implementation

The reference implementation uses:
- `serde` for serialization traits
- `rmp-serde` for MessagePack encoding
- `bincode` for internal coordinate serialization

### Testing

All packet types should be tested for:
- Serialization round-trip
- Size limits
- Invalid data handling
- Version compatibility

### Future Extensions

Possible future additions:
- Packet fragmentation for large payloads
- Multicast routing
- Quality of Service (QoS) fields
- Encryption metadata
- Compression flags

## References

- MessagePack specification: https://msgpack.org/
- Ed25519 signature scheme: https://ed25519.cr.yp.to/
- DRFE-R design document: `design.md`
- DRFE-R requirements: `requirements.md`

---

**Document Version:** 1.0  
**Last Updated:** 2026-01-01  
**Status:** Draft
