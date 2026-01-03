/**
 * Chat Panel Component
 * 
 * Provides the main chat UI with:
 * - Message display with routing visualization
 * - Message input
 * - User/room selection
 * - Routing mode indicators for each message
 * - Educational tooltips explaining routing modes
 * 
 * Requirements: 13.2, 13.4
 */

import { useState, useRef, useEffect, useCallback } from 'react';
import type { ChatMessage, RoomInfo, UserInfo, MessageRoutingInfo } from '../types/chat';
import type { RoutingMode } from '../types';

interface ChatPanelProps {
  /** Current user ID */
  userId: string;
  /** List of messages */
  messages: ChatMessage[];
  /** List of available rooms */
  rooms: RoomInfo[];
  /** List of online users */
  users: UserInfo[];
  /** Currently selected room (null for direct messages) */
  currentRoom: string | null;
  /** Currently selected recipient for DMs */
  currentRecipient: string | null;
  /** Routing info for messages */
  routingInfo: Map<string, MessageRoutingInfo>;
  /** Callback to send a message */
  onSendMessage: (recipient: string, content: string) => void;
  /** Callback to send a room message */
  onSendRoomMessage: (roomId: string, content: string) => void;
  /** Callback to select a room */
  onSelectRoom: (roomId: string | null) => void;
  /** Callback to select a recipient */
  onSelectRecipient: (userId: string | null) => void;
  /** Callback to join a room */
  onJoinRoom: (roomId: string) => void;
  /** Callback to create a room */
  onCreateRoom: (roomId: string, name: string) => void;
  /** Callback when a routing path should be visualized */
  onVisualizeRoute?: (path: string[], modeStr?: string) => void;
  /** Connection status */
  isConnected: boolean;
}

/** Educational tooltips for routing modes */
const ROUTING_MODE_TOOLTIPS: Record<RoutingMode, { title: string; description: string }> = {
  gravity: {
    title: 'Gravity Mode',
    description: 'The message is moving toward its destination using hyperbolic distance. This is the most efficient routing mode, following the natural "gravity" of the hyperbolic space.',
  },
  pressure: {
    title: 'Pressure Mode',
    description: 'The message encountered a local minimum and is using "pressure" to escape. This happens when all neighbors are farther from the destination than the current node.',
  },
  tree: {
    title: 'Tree Mode',
    description: 'The message is using the spanning tree fallback for guaranteed delivery. This mode ensures 100% reachability but may take a longer path.',
  },
};

/** Parse routing mode from mode string (e.g., "G:3/P:1/T:0") */
function parseRoutingModes(modeStr: string): { gravity: number; pressure: number; tree: number } {
  const result = { gravity: 0, pressure: 0, tree: 0 };
  const parts = modeStr.split('/');
  
  parts.forEach(part => {
    if (part.startsWith('G:')) result.gravity = parseInt(part.slice(2)) || 0;
    if (part.startsWith('P:')) result.pressure = parseInt(part.slice(2)) || 0;
    if (part.startsWith('T:')) result.tree = parseInt(part.slice(2)) || 0;
  });
  
  return result;
}

/** Get the dominant routing mode from mode string */
function getDominantMode(modeStr: string): RoutingMode {
  const modes = parseRoutingModes(modeStr);
  if (modes.tree > 0) return 'tree';
  if (modes.pressure > 0) return 'pressure';
  return 'gravity';
}

export function ChatPanel({
  userId,
  messages,
  rooms,
  users,
  currentRoom,
  currentRecipient,
  routingInfo,
  onSendMessage,
  onSendRoomMessage,
  onSelectRoom,
  onSelectRecipient,
  onJoinRoom,
  onCreateRoom,
  onVisualizeRoute,
  isConnected,
}: ChatPanelProps) {
  const [inputValue, setInputValue] = useState('');
  const [showCreateRoom, setShowCreateRoom] = useState(false);
  const [newRoomId, setNewRoomId] = useState('');
  const [newRoomName, setNewRoomName] = useState('');
  const [activeTooltip, setActiveTooltip] = useState<string | null>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom when new messages arrive
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  // Filter messages for current conversation
  const filteredMessages = messages.filter(msg => {
    if (currentRoom) {
      return msg.room_id === currentRoom;
    }
    if (currentRecipient) {
      return (
        (msg.sender === userId && msg.recipient === currentRecipient) ||
        (msg.sender === currentRecipient && msg.recipient === userId)
      );
    }
    // Show all messages if no room/recipient selected
    return true;
  });

  // Handle sending a message
  const handleSend = useCallback(() => {
    if (!inputValue.trim()) return;

    if (currentRoom) {
      onSendRoomMessage(currentRoom, inputValue.trim());
    } else if (currentRecipient) {
      onSendMessage(currentRecipient, inputValue.trim());
    }
    setInputValue('');
  }, [inputValue, currentRoom, currentRecipient, onSendMessage, onSendRoomMessage]);

  // Handle key press (Enter to send)
  const handleKeyPress = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  }, [handleSend]);

  // Handle creating a new room
  const handleCreateRoom = useCallback(() => {
    if (newRoomId.trim() && newRoomName.trim()) {
      onCreateRoom(newRoomId.trim(), newRoomName.trim());
      setNewRoomId('');
      setNewRoomName('');
      setShowCreateRoom(false);
    }
  }, [newRoomId, newRoomName, onCreateRoom]);

  // Format timestamp
  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp);
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  };

  // Get message content text
  const getMessageContent = (msg: ChatMessage): string => {
    const msgType = msg.message_type;
    if ('content' in msgType) return msgType.content;
    if ('message' in msgType) return msgType.message;
    if ('user_id' in msgType) return `User ${msgType.user_id} joined`;
    if ('room_id' in msgType) return `Room ${msgType.room_id} created`;
    if ('path' in msgType) return `Route: ${msgType.path.join(' → ')} (${msgType.hops} hops)`;
    return '';
  };

  // Check if message is a system message
  const isSystemMessage = (msg: ChatMessage): boolean => {
    return msg.sender === 'system' || 
           msg.message_type.type === 'system' ||
           msg.message_type.type === 'user_joined' ||
           msg.message_type.type === 'user_left' ||
           msg.message_type.type === 'room_created';
  };

  // Check if message is a routing info message
  const isRoutingMessage = (msg: ChatMessage): boolean => {
    return msg.message_type.type === 'routing_info';
  };

  return (
    <div className="chat-panel">
      {/* Sidebar with rooms and users */}
      <div className="chat-sidebar">
        {/* Rooms section */}
        <div className="chat-section">
          <div className="section-header">
            <h3>Rooms</h3>
            <button 
              className="add-btn"
              onClick={() => setShowCreateRoom(!showCreateRoom)}
              title="Create Room"
            >
              +
            </button>
          </div>
          
          {showCreateRoom && (
            <div className="create-room-form">
              <input
                type="text"
                placeholder="Room ID"
                value={newRoomId}
                onChange={e => setNewRoomId(e.target.value)}
              />
              <input
                type="text"
                placeholder="Room Name"
                value={newRoomName}
                onChange={e => setNewRoomName(e.target.value)}
              />
              <div className="form-buttons">
                <button onClick={handleCreateRoom}>Create</button>
                <button onClick={() => setShowCreateRoom(false)}>Cancel</button>
              </div>
            </div>
          )}

          <ul className="room-list">
            {rooms.map(room => (
              <li
                key={room.id}
                className={`room-item ${currentRoom === room.id ? 'active' : ''}`}
                onClick={() => {
                  onSelectRoom(room.id);
                  onSelectRecipient(null);
                }}
              >
                <span className="room-icon">#</span>
                <span className="room-name">{room.name}</span>
                <span className="room-count">{room.member_count}</span>
              </li>
            ))}
            {rooms.length === 0 && (
              <li className="empty-hint">No rooms available</li>
            )}
          </ul>
        </div>

        {/* Users section */}
        <div className="chat-section">
          <h3>Online Users</h3>
          <ul className="user-list">
            {users.filter(u => u.id !== userId).map(user => (
              <li
                key={user.id}
                className={`user-item ${currentRecipient === user.id ? 'active' : ''}`}
                onClick={() => {
                  onSelectRecipient(user.id);
                  onSelectRoom(null);
                }}
              >
                <span className="user-status online">●</span>
                <span className="user-name">{user.id}</span>
              </li>
            ))}
            {users.filter(u => u.id !== userId).length === 0 && (
              <li className="empty-hint">No other users online</li>
            )}
          </ul>
        </div>
      </div>

      {/* Main chat area */}
      <div className="chat-main">
        {/* Chat header */}
        <div className="chat-header">
          {currentRoom ? (
            <>
              <span className="chat-icon">#</span>
              <span className="chat-title">{rooms.find(r => r.id === currentRoom)?.name || currentRoom}</span>
              <button 
                className="join-btn"
                onClick={() => onJoinRoom(currentRoom)}
              >
                Join
              </button>
            </>
          ) : currentRecipient ? (
            <>
              <span className="chat-icon">@</span>
              <span className="chat-title">{currentRecipient}</span>
            </>
          ) : (
            <span className="chat-title">Select a room or user to start chatting</span>
          )}
        </div>

        {/* Messages area */}
        <div className="chat-messages">
          {filteredMessages.map(msg => {
            const msgRoutingInfo = routingInfo.get(msg.id);
            const isOwn = msg.sender === userId;
            const isSystem = isSystemMessage(msg);
            const isRouting = isRoutingMessage(msg);
            const dominantMode = msgRoutingInfo ? getDominantMode(msgRoutingInfo.mode) : null;
            const modeBreakdown = msgRoutingInfo ? parseRoutingModes(msgRoutingInfo.mode) : null;

            return (
              <div
                key={msg.id}
                className={`message ${isOwn ? 'own' : ''} ${isSystem ? 'system' : ''} ${isRouting ? 'routing' : ''}`}
              >
                {!isSystem && !isRouting && (
                  <div className="message-header">
                    <span className="message-sender">{msg.sender}</span>
                    <span className="message-time">{formatTime(msg.timestamp)}</span>
                  </div>
                )}
                <div className="message-content">
                  {getMessageContent(msg)}
                </div>
                {msgRoutingInfo && (
                  <div className="message-routing-container">
                    {/* Routing Mode Indicator */}
                    <div 
                      className={`routing-mode-indicator ${dominantMode}`}
                      onMouseEnter={() => setActiveTooltip(msg.id)}
                      onMouseLeave={() => setActiveTooltip(null)}
                    >
                      <span className="mode-icon">{dominantMode === 'gravity' ? 'G' : dominantMode === 'pressure' ? 'P' : 'T'}</span>
                      <span className="mode-label">{dominantMode}</span>
                      
                      {/* Educational Tooltip */}
                      {activeTooltip === msg.id && dominantMode && (
                        <div className="routing-tooltip">
                          <div className="tooltip-header">{ROUTING_MODE_TOOLTIPS[dominantMode].title}</div>
                          <div className="tooltip-description">{ROUTING_MODE_TOOLTIPS[dominantMode].description}</div>
                          {modeBreakdown && (
                            <div className="tooltip-breakdown">
                              <div className="breakdown-title">Hop Breakdown:</div>
                              <div className="breakdown-items">
                                <span className="breakdown-item gravity">G: {modeBreakdown.gravity}</span>
                                <span className="breakdown-item pressure">P: {modeBreakdown.pressure}</span>
                                <span className="breakdown-item tree">T: {modeBreakdown.tree}</span>
                              </div>
                            </div>
                          )}
                        </div>
                      )}
                    </div>
                    
                    {/* Routing Path Visualization */}
                    <div 
                      className="message-routing"
                      onClick={() => onVisualizeRoute?.(msgRoutingInfo.path, msgRoutingInfo.mode)}
                      title="Click to animate routing path in topology view"
                    >
                      <span className="routing-label">Route:</span>
                      <span className="routing-path">
                        {msgRoutingInfo.path.map((node, idx) => (
                          <span key={idx} className="path-node">
                            {idx > 0 && <span className="path-arrow">→</span>}
                            <span className="node-id">{node.slice(0, 6)}</span>
                          </span>
                        ))}
                      </span>
                      <span className="routing-stats">
                        {msgRoutingInfo.hops} hops
                      </span>
                      <span className="visualize-hint">
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                          <circle cx="12" cy="12" r="10"/>
                          <polygon points="10,8 16,12 10,16" fill="currentColor"/>
                        </svg>
                      </span>
                    </div>
                    
                    {/* Mode Progress Bar */}
                    {modeBreakdown && (
                      <div className="routing-mode-bar">
                        {modeBreakdown.gravity > 0 && (
                          <div 
                            className="mode-segment gravity" 
                            style={{ flex: modeBreakdown.gravity }}
                            title={`Gravity: ${modeBreakdown.gravity} hops`}
                          />
                        )}
                        {modeBreakdown.pressure > 0 && (
                          <div 
                            className="mode-segment pressure" 
                            style={{ flex: modeBreakdown.pressure }}
                            title={`Pressure: ${modeBreakdown.pressure} hops`}
                          />
                        )}
                        {modeBreakdown.tree > 0 && (
                          <div 
                            className="mode-segment tree" 
                            style={{ flex: modeBreakdown.tree }}
                            title={`Tree: ${modeBreakdown.tree} hops`}
                          />
                        )}
                      </div>
                    )}
                  </div>
                )}
              </div>
            );
          })}
          <div ref={messagesEndRef} />
        </div>

        {/* Input area */}
        <div className="chat-input-area">
          <textarea
            className="chat-input"
            placeholder={
              currentRoom 
                ? `Message #${currentRoom}...` 
                : currentRecipient 
                  ? `Message @${currentRecipient}...`
                  : 'Select a room or user...'
            }
            value={inputValue}
            onChange={e => setInputValue(e.target.value)}
            onKeyPress={handleKeyPress}
            disabled={!isConnected || (!currentRoom && !currentRecipient)}
          />
          <button
            className="send-btn"
            onClick={handleSend}
            disabled={!isConnected || !inputValue.trim() || (!currentRoom && !currentRecipient)}
          >
            Send
          </button>
        </div>
      </div>
    </div>
  );
}

export default ChatPanel;
