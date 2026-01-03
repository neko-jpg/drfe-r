/**
 * Type definitions for DRFE-R Chat Application
 * Requirements: 13.4
 */

/**
 * Chat message types from the server
 */
export type ChatMessageType =
  | { type: 'text'; content: string }
  | { type: 'user_joined'; user_id: string }
  | { type: 'user_left'; user_id: string }
  | { type: 'room_created'; room_id: string }
  | { type: 'routing_info'; path: string[]; mode: string; hops: number }
  | { type: 'error'; message: string }
  | { type: 'system'; message: string };

/**
 * Chat message structure
 */
export interface ChatMessage {
  id: string;
  sender: string;
  recipient: string;
  message_type: ChatMessageType;
  timestamp: number;
  room_id?: string;
}

/**
 * Room information
 */
export interface RoomInfo {
  id: string;
  name: string;
  member_count: number;
}

/**
 * User information
 */
export interface UserInfo {
  id: string;
  coord_x: number;
  coord_y: number;
}

/**
 * Client message types (sent to server)
 */
export type ClientMessageAction =
  | { action: 'send_message'; recipient: string; content: string }
  | { action: 'send_room_message'; room_id: string; content: string }
  | { action: 'join_room'; room_id: string }
  | { action: 'leave_room'; room_id: string }
  | { action: 'create_room'; room_id: string; name: string }
  | { action: 'list_rooms' }
  | { action: 'list_users' }
  | { action: 'ping' };

/**
 * Server message types (received from server)
 */
export type ServerMessage =
  | { type: 'message'; } & ChatMessage
  | { type: 'room_list'; rooms: RoomInfo[] }
  | { type: 'user_list'; users: UserInfo[] }
  | { type: 'success'; message: string }
  | { type: 'error'; message: string }
  | { type: 'pong' };

/**
 * Chat connection status
 */
export type ChatConnectionStatus = 'disconnected' | 'connecting' | 'connected' | 'error';

/**
 * Chat state for the application
 */
export interface ChatState {
  userId: string | null;
  messages: ChatMessage[];
  rooms: RoomInfo[];
  users: UserInfo[];
  currentRoom: string | null;
  connectionStatus: ChatConnectionStatus;
}

/**
 * Routing visualization data for a message
 */
export interface MessageRoutingInfo {
  messageId: string;
  path: string[];
  mode: string;
  hops: number;
}
