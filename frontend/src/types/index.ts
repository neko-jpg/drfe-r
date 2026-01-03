/**
 * Type definitions for DRFE-R Visualization Dashboard
 */

/**
 * A point in the Poincaré disk model of hyperbolic space
 * Coordinates must satisfy x² + y² < 1
 */
export interface PoincareDiskPoint {
  x: number;
  y: number;
}

/**
 * A node in the DRFE-R network
 */
export interface NetworkNode {
  id: string;
  coordinate: PoincareDiskPoint;
  neighbors: string[];
  isOnline: boolean;
}

/**
 * An edge in the network topology
 */
export interface NetworkEdge {
  source: string;
  target: string;
  curvature?: number;
}

/**
 * The complete network topology
 */
export interface NetworkTopology {
  nodes: NetworkNode[];
  edges: NetworkEdge[];
}

/**
 * Routing mode used by the GP algorithm
 */
export type RoutingMode = 'gravity' | 'pressure' | 'tree';

/**
 * A packet being routed through the network
 */
export interface RoutingPacket {
  id: string;
  source: string;
  destination: string;
  currentNode: string;
  path: string[];
  mode: RoutingMode;
  ttl: number;
  status: 'pending' | 'in_transit' | 'delivered' | 'failed';
}

/**
 * Statistics for a node
 */
export interface NodeStats {
  packetsRouted: number;
  packetsDelivered: number;
  averageHops: number;
  uptime: number;
}

/**
 * Canvas rendering options
 */
export interface RenderOptions {
  width: number;
  height: number;
  showEdges: boolean;
  showLabels: boolean;
  showCurvature: boolean;
  animateRouting: boolean;
}

/**
 * WebSocket connection status
 */
export type ConnectionStatus = 'connecting' | 'connected' | 'disconnected' | 'error';

/**
 * WebSocket message types from the backend
 */
export type WebSocketMessageType = 
  | 'topology_update'
  | 'routing_event'
  | 'node_status'
  | 'error';

/**
 * Base WebSocket message structure
 */
export interface WebSocketMessage {
  type: WebSocketMessageType;
  timestamp: number;
}

/**
 * Topology update message from WebSocket
 */
export interface TopologyUpdateMessage extends WebSocketMessage {
  type: 'topology_update';
  topology: NetworkTopology;
}

/**
 * Routing event message from WebSocket
 */
export interface RoutingEventMessage extends WebSocketMessage {
  type: 'routing_event';
  event: RoutingEvent;
}

/**
 * Routing event types
 */
export type RoutingEventType = 
  | 'packet_sent'
  | 'packet_hop'
  | 'packet_delivered'
  | 'packet_failed'
  | 'mode_change';

/**
 * A routing event for animation
 */
export interface RoutingEvent {
  eventType: RoutingEventType;
  packetId: string;
  fromNode: string;
  toNode: string;
  mode: RoutingMode;
  timestamp: number;
  hops?: number;
  ttl?: number;
}

/**
 * Active packet animation state
 */
export interface PacketAnimation {
  id: string;
  fromNode: string;
  toNode: string;
  mode: RoutingMode;
  progress: number; // 0 to 1
  startTime: number;
  duration: number; // milliseconds
}

/**
 * Routing statistics for display
 */
export interface RoutingStats {
  totalPackets: number;
  deliveredPackets: number;
  failedPackets: number;
  averageHops: number;
  modeBreakdown: {
    gravity: number;
    pressure: number;
    tree: number;
  };
}

/**
 * Per-node routing statistics
 */
export interface NodeRoutingStats {
  packetsRouted: number;
  packetsOriginated: number;
  packetsReceived: number;
  lastActivity: number | null;
  modeUsage: {
    gravity: number;
    pressure: number;
    tree: number;
  };
}

/**
 * Extended node information for inspection panel
 */
export interface NodeInspectionData {
  node: NetworkNode;
  stats: NodeRoutingStats;
  hyperbolicDistance: number | null; // Distance from origin
  neighborDistances: { id: string; distance: number }[];
}

// Re-export chat types
export * from './chat';
