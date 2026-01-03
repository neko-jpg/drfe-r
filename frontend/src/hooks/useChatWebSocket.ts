/**
 * WebSocket hook for DRFE-R Chat Application
 * 
 * Connects to the chat backend and handles:
 * - User authentication
 * - Message sending/receiving
 * - Room management
 * - User list updates
 * 
 * Requirements: 13.4
 */

import { useState, useEffect, useCallback, useRef } from 'react';
import type {
  ChatMessage,
  RoomInfo,
  UserInfo,
  ClientMessageAction,
  ChatConnectionStatus,
  MessageRoutingInfo,
} from '../types/chat';

interface UseChatWebSocketOptions {
  /** WebSocket server URL (e.g., 'ws://localhost:8080/ws/username') */
  url: string;
  /** User ID for authentication */
  userId: string;
  /** Auto-reconnect on disconnect */
  autoReconnect?: boolean;
  /** Reconnect interval in milliseconds */
  reconnectInterval?: number;
  /** Maximum reconnect attempts */
  maxReconnectAttempts?: number;
  /** Callback when a message is received */
  onMessage?: (message: ChatMessage) => void;
  /** Callback when routing info is received */
  onRoutingInfo?: (info: MessageRoutingInfo) => void;
  /** Callback when room list updates */
  onRoomListUpdate?: (rooms: RoomInfo[]) => void;
  /** Callback when user list updates */
  onUserListUpdate?: (users: UserInfo[]) => void;
  /** Callback when connection status changes */
  onStatusChange?: (status: ChatConnectionStatus) => void;
}

interface UseChatWebSocketReturn {
  /** Current connection status */
  status: ChatConnectionStatus;
  /** Connect to chat server */
  connect: () => void;
  /** Disconnect from chat server */
  disconnect: () => void;
  /** Send a direct message to another user */
  sendMessage: (recipient: string, content: string) => void;
  /** Send a message to a room */
  sendRoomMessage: (roomId: string, content: string) => void;
  /** Join a room */
  joinRoom: (roomId: string) => void;
  /** Leave a room */
  leaveRoom: (roomId: string) => void;
  /** Create a new room */
  createRoom: (roomId: string, name: string) => void;
  /** Request room list */
  listRooms: () => void;
  /** Request user list */
  listUsers: () => void;
  /** Last error message */
  error: string | null;
  /** Number of reconnect attempts */
  reconnectAttempts: number;
}

/**
 * Hook for managing WebSocket connection to DRFE-R chat backend
 */
export function useChatWebSocket(options: UseChatWebSocketOptions): UseChatWebSocketReturn {
  const {
    url,
    userId,
    autoReconnect = true,
    reconnectInterval = 3000,
    maxReconnectAttempts = 10,
    onMessage,
    onRoutingInfo,
    onRoomListUpdate,
    onUserListUpdate,
    onStatusChange,
  } = options;

  const [status, setStatus] = useState<ChatConnectionStatus>('disconnected');
  const [error, setError] = useState<string | null>(null);
  const [reconnectAttempts, setReconnectAttempts] = useState(0);

  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<number | null>(null);
  const isManualDisconnect = useRef(false);
  const connectRef = useRef<() => void>(() => {});

  // Update status and notify callback
  const updateStatus = useCallback((newStatus: ChatConnectionStatus) => {
    setStatus(newStatus);
    onStatusChange?.(newStatus);
  }, [onStatusChange]);

  // Handle incoming messages
  const handleMessage = useCallback((event: MessageEvent) => {
    try {
      const data = JSON.parse(event.data);

      switch (data.type) {
        case 'message': {
          // Extract the ChatMessage from the server response
          const message: ChatMessage = {
            id: data.id,
            sender: data.sender,
            recipient: data.recipient,
            message_type: data.message_type,
            timestamp: data.timestamp,
            room_id: data.room_id,
          };
          onMessage?.(message);
          
          // Check if this is a routing info message
          if (data.message_type?.type === 'routing_info') {
            const routingInfo: MessageRoutingInfo = {
              messageId: data.id,
              path: data.message_type.path,
              mode: data.message_type.mode,
              hops: data.message_type.hops,
            };
            onRoutingInfo?.(routingInfo);
          }
          break;
        }
        case 'room_list': {
          onRoomListUpdate?.(data.rooms);
          break;
        }
        case 'user_list': {
          onUserListUpdate?.(data.users);
          break;
        }
        case 'success': {
          // Success messages can be logged or shown as notifications
          console.log('Chat success:', data.message);
          break;
        }
        case 'error': {
          setError(data.message);
          console.error('Chat error:', data.message);
          break;
        }
        case 'pong': {
          // Keepalive response, no action needed
          break;
        }
        default:
          console.warn('Unknown chat message type:', data.type);
      }
    } catch (err) {
      console.error('Failed to parse chat message:', err);
    }
  }, [onMessage, onRoutingInfo, onRoomListUpdate, onUserListUpdate]);

  // Send a message to the server
  const send = useCallback((message: ClientMessageAction) => {
    if (wsRef.current && wsRef.current.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(message));
    } else {
      console.warn('Chat WebSocket is not connected');
    }
  }, []);

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
      // Construct the full URL with user ID
      const fullUrl = `${url}/${userId}`;
      const ws = new WebSocket(fullUrl);

      ws.onopen = () => {
        updateStatus('connected');
        setReconnectAttempts(0);
        setError(null);
        
        // Request initial room and user lists
        send({ action: 'list_rooms' });
        send({ action: 'list_users' });
      };

      ws.onmessage = handleMessage;

      ws.onerror = () => {
        setError('Chat WebSocket connection error');
        updateStatus('error');
      };

      ws.onclose = () => {
        if (!isManualDisconnect.current) {
          updateStatus('disconnected');
          
          // Attempt reconnect if enabled
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
      console.error('Failed to create chat WebSocket:', err);
      setError('Failed to create chat WebSocket connection');
      updateStatus('error');
    }
  }, [url, userId, autoReconnect, reconnectInterval, maxReconnectAttempts, handleMessage, updateStatus, send]);

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

  // Chat-specific actions
  const sendMessage = useCallback((recipient: string, content: string) => {
    send({ action: 'send_message', recipient, content });
  }, [send]);

  const sendRoomMessage = useCallback((roomId: string, content: string) => {
    send({ action: 'send_room_message', room_id: roomId, content });
  }, [send]);

  const joinRoom = useCallback((roomId: string) => {
    send({ action: 'join_room', room_id: roomId });
  }, [send]);

  const leaveRoom = useCallback((roomId: string) => {
    send({ action: 'leave_room', room_id: roomId });
  }, [send]);

  const createRoom = useCallback((roomId: string, name: string) => {
    send({ action: 'create_room', room_id: roomId, name });
  }, [send]);

  const listRooms = useCallback(() => {
    send({ action: 'list_rooms' });
  }, [send]);

  const listUsers = useCallback(() => {
    send({ action: 'list_users' });
  }, [send]);

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

  // Set up keepalive ping
  useEffect(() => {
    if (status !== 'connected') return;

    const pingInterval = setInterval(() => {
      send({ action: 'ping' });
    }, 30000); // Ping every 30 seconds

    return () => clearInterval(pingInterval);
  }, [status, send]);

  return {
    status,
    connect,
    disconnect,
    sendMessage,
    sendRoomMessage,
    joinRoom,
    leaveRoom,
    createRoom,
    listRooms,
    listUsers,
    error,
    reconnectAttempts,
  };
}

export default useChatWebSocket;
