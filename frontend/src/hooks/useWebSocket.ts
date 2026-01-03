/**
 * WebSocket hook for real-time routing updates
 * 
 * Connects to the DRFE-R backend and receives:
 * - Topology updates
 * - Routing events for animation
 * - Node status changes
 */

import { useState, useEffect, useCallback, useRef } from 'react';
import type { 
  ConnectionStatus, 
  RoutingEvent, 
  NetworkTopology,
  WebSocketMessage,
  TopologyUpdateMessage,
  RoutingEventMessage
} from '../types';

interface UseWebSocketOptions {
  /** WebSocket server URL (e.g., 'ws://localhost:3001/ws') */
  url: string;
  /** Auto-reconnect on disconnect */
  autoReconnect?: boolean;
  /** Reconnect interval in milliseconds */
  reconnectInterval?: number;
  /** Maximum reconnect attempts */
  maxReconnectAttempts?: number;
  /** Callback when topology updates */
  onTopologyUpdate?: (topology: NetworkTopology) => void;
  /** Callback when routing event occurs */
  onRoutingEvent?: (event: RoutingEvent) => void;
  /** Callback when connection status changes */
  onStatusChange?: (status: ConnectionStatus) => void;
}

interface UseWebSocketReturn {
  /** Current connection status */
  status: ConnectionStatus;
  /** Connect to WebSocket server */
  connect: () => void;
  /** Disconnect from WebSocket server */
  disconnect: () => void;
  /** Send a message to the server */
  send: (message: object) => void;
  /** Last error message */
  error: string | null;
  /** Number of reconnect attempts */
  reconnectAttempts: number;
}

/**
 * Hook for managing WebSocket connection to DRFE-R backend
 */
export function useWebSocket(options: UseWebSocketOptions): UseWebSocketReturn {
  const {
    url,
    autoReconnect = true,
    reconnectInterval = 3000,
    maxReconnectAttempts = 10,
    onTopologyUpdate,
    onRoutingEvent,
    onStatusChange,
  } = options;

  const [status, setStatus] = useState<ConnectionStatus>('disconnected');
  const [error, setError] = useState<string | null>(null);
  const [reconnectAttempts, setReconnectAttempts] = useState(0);

  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<number | null>(null);
  const isManualDisconnect = useRef(false);
  const connectRef = useRef<() => void>(() => {});

  // Update status and notify callback
  const updateStatus = useCallback((newStatus: ConnectionStatus) => {
    setStatus(newStatus);
    onStatusChange?.(newStatus);
  }, [onStatusChange]);

  // Handle incoming messages
  const handleMessage = useCallback((event: MessageEvent) => {
    try {
      const message = JSON.parse(event.data) as WebSocketMessage;

      switch (message.type) {
        case 'topology_update': {
          const topologyMsg = message as TopologyUpdateMessage;
          onTopologyUpdate?.(topologyMsg.topology);
          break;
        }
        case 'routing_event': {
          const routingMsg = message as RoutingEventMessage;
          onRoutingEvent?.(routingMsg.event);
          break;
        }
        case 'error': {
          console.error('WebSocket error message:', message);
          break;
        }
        default:
          console.warn('Unknown WebSocket message type:', message.type);
      }
    } catch (err) {
      console.error('Failed to parse WebSocket message:', err);
    }
  }, [onTopologyUpdate, onRoutingEvent]);

  // Connect to WebSocket server
  const connect = useCallback(() => {
    // Clean up existing connection
    if (wsRef.current) {
      wsRef.current.close();
    }

    // Clear any pending reconnect
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
      reconnectTimeoutRef.current = null;
    }

    isManualDisconnect.current = false;
    updateStatus('connecting');
    setError(null);

    try {
      const ws = new WebSocket(url);

      ws.onopen = () => {
        updateStatus('connected');
        setReconnectAttempts(0);
        setError(null);
      };

      ws.onmessage = handleMessage;

      ws.onerror = () => {
        setError('WebSocket connection error');
        updateStatus('error');
      };

      ws.onclose = () => {
        if (!isManualDisconnect.current) {
          updateStatus('disconnected');
          
          // Attempt reconnect if enabled - use ref to avoid circular dependency
          if (autoReconnect) {
            setReconnectAttempts(prev => {
              if (prev < maxReconnectAttempts) {
                reconnectTimeoutRef.current = window.setTimeout(() => {
                  connectRef.current();
                }, reconnectInterval);
                return prev + 1;
              } else {
                setError(`Failed to reconnect after ${maxReconnectAttempts} attempts`);
                return prev;
              }
            });
          }
        }
      };

      wsRef.current = ws;
    } catch (err) {
      console.error('Failed to create WebSocket:', err);
      setError('Failed to create WebSocket connection');
      updateStatus('error');
    }
  }, [url, autoReconnect, reconnectInterval, maxReconnectAttempts, handleMessage, updateStatus]);

  // Keep connectRef updated
  useEffect(() => {
    connectRef.current = connect;
  }, [connect]);

  // Disconnect from WebSocket server
  const disconnect = useCallback(() => {
    isManualDisconnect.current = true;
    
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
      reconnectTimeoutRef.current = null;
    }

    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }

    updateStatus('disconnected');
    setReconnectAttempts(0);
  }, [updateStatus]);

  // Send a message to the server
  const send = useCallback((message: object) => {
    if (wsRef.current && wsRef.current.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(message));
    } else {
      console.warn('WebSocket is not connected');
    }
  }, []);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      isManualDisconnect.current = true;
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
      }
      if (wsRef.current) {
        wsRef.current.close();
      }
    };
  }, []);

  return {
    status,
    connect,
    disconnect,
    send,
    error,
    reconnectAttempts,
  };
}

export default useWebSocket;
