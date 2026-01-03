/**
 * Chat Application Component
 * 
 * Main chat application that integrates:
 * - Chat panel for messaging
 * - Topology visualization showing routing paths
 * - Real-time routing animation
 * 
 * Requirements: 13.4
 */

import { useState, useCallback, useMemo } from 'react';
import { PoincareDisk } from './PoincareDisk';
import { ChatPanel } from './ChatPanel';
import { useChatWebSocket } from '../hooks/useChatWebSocket';
import { useRoutingAnimation } from '../hooks/useRoutingAnimation';
import type { ChatMessage, RoomInfo, UserInfo, MessageRoutingInfo } from '../types/chat';
import type { NetworkTopology, RoutingEvent, RoutingMode } from '../types';

// Chat WebSocket server URL (configurable via environment variable)
const CHAT_WS_URL = import.meta.env.VITE_CHAT_WS_URL || 'ws://localhost:8080/ws';

// Parse routing mode from mode string (e.g., "G:3/P:1/T:0")
// Defined at module level to avoid hoisting issues
function parseRoutingMode(modeStr: string, hopIndex: number): RoutingMode {
  const parts = modeStr.split('/');
  let gravityHops = 0;
  let pressureHops = 0;
  
  parts.forEach(part => {
    if (part.startsWith('G:')) gravityHops = parseInt(part.slice(2)) || 0;
    if (part.startsWith('P:')) pressureHops = parseInt(part.slice(2)) || 0;
  });

  if (hopIndex < gravityHops) return 'gravity';
  if (hopIndex < gravityHops + pressureHops) return 'pressure';
  return 'tree';
}

interface ChatAppProps {
  /** Initial user ID (can be changed) */
  initialUserId?: string;
}

export function ChatApp({ initialUserId }: ChatAppProps) {
  // User state
  const [userId, setUserId] = useState(initialUserId || '');
  const [isLoggedIn, setIsLoggedIn] = useState(false);
  const [loginInput, setLoginInput] = useState('');

  // Chat state
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [rooms, setRooms] = useState<RoomInfo[]>([]);
  const [users, setUsers] = useState<UserInfo[]>([]);
  const [currentRoom, setCurrentRoom] = useState<string | null>(null);
  const [currentRecipient, setCurrentRecipient] = useState<string | null>(null);
  const [routingInfo, setRoutingInfo] = useState<Map<string, MessageRoutingInfo>>(new Map());

  // Visualization state
  const [showVisualization, setShowVisualization] = useState(true);

  // Routing animation hook
  const {
    animations,
    activeMode,
    stats,
    addRoutingEvent,
    clearAnimations,
    resetStats,
  } = useRoutingAnimation({ animationDuration: 600, maxAnimations: 10 });

  // Handle incoming messages
  const handleMessage = useCallback((message: ChatMessage) => {
    setMessages(prev => [...prev, message]);
  }, []);

  // Handle routing info
  const handleRoutingInfo = useCallback((info: MessageRoutingInfo) => {
    setRoutingInfo(prev => {
      const newMap = new Map(prev);
      newMap.set(info.messageId, info);
      return newMap;
    });

    // Trigger routing animation
    if (info.path.length > 1) {
      // Animate each hop in the path
      info.path.forEach((node, index) => {
        if (index < info.path.length - 1) {
          setTimeout(() => {
            const mode = parseRoutingMode(info.mode, index);
            const event: RoutingEvent = {
              eventType: index === 0 ? 'packet_sent' : 'packet_hop',
              packetId: info.messageId,
              fromNode: node,
              toNode: info.path[index + 1],
              mode,
              timestamp: Date.now(),
              hops: index + 1,
            };
            addRoutingEvent(event);
          }, index * 500);
        }
      });

      // Mark as delivered
      setTimeout(() => {
        const event: RoutingEvent = {
          eventType: 'packet_delivered',
          packetId: info.messageId,
          fromNode: info.path[info.path.length - 1],
          toNode: info.path[info.path.length - 1],
          mode: 'gravity',
          timestamp: Date.now(),
          hops: info.hops,
        };
        addRoutingEvent(event);
      }, info.path.length * 500);
    }
  }, [addRoutingEvent]);

  // Handle room list update
  const handleRoomListUpdate = useCallback((newRooms: RoomInfo[]) => {
    setRooms(newRooms);
  }, []);

  // Handle user list update
  const handleUserListUpdate = useCallback((newUsers: UserInfo[]) => {
    setUsers(newUsers);
  }, []);

  // Chat WebSocket hook
  const chatWs = useChatWebSocket({
    url: CHAT_WS_URL,
    userId,
    autoReconnect: true,
    onMessage: handleMessage,
    onRoutingInfo: handleRoutingInfo,
    onRoomListUpdate: handleRoomListUpdate,
    onUserListUpdate: handleUserListUpdate,
  });

  // Build network topology from users
  const topology: NetworkTopology = useMemo(() => {
    const nodes = users.map(user => ({
      id: user.id,
      coordinate: { x: user.coord_x, y: user.coord_y },
      neighbors: [], // Will be populated based on routing
      isOnline: true,
    }));

    // Add room anchors as nodes
    rooms.forEach(room => {
      // Generate a deterministic coordinate for the room
      const hash = room.id.split('').reduce((acc, char) => acc + char.charCodeAt(0), 0);
      const angle = (hash % 360) * (Math.PI / 180);
      const radius = 0.3 + (hash % 50) / 100;
      nodes.push({
        id: room.id,
        coordinate: { x: radius * Math.cos(angle), y: radius * Math.sin(angle) },
        neighbors: [],
        isOnline: true,
      });
    });

    // Create edges based on k-nearest neighbors (simplified)
    const edges: { source: string; target: string; curvature?: number }[] = [];
    nodes.forEach((node, i) => {
      // Connect to 3 nearest neighbors
      const distances = nodes
        .map((other, j) => ({
          id: other.id,
          index: j,
          dist: Math.sqrt(
            Math.pow(node.coordinate.x - other.coordinate.x, 2) +
            Math.pow(node.coordinate.y - other.coordinate.y, 2)
          ),
        }))
        .filter(d => d.index !== i)
        .sort((a, b) => a.dist - b.dist)
        .slice(0, 3);

      distances.forEach(d => {
        // Avoid duplicate edges
        if (!edges.some(e => 
          (e.source === node.id && e.target === d.id) ||
          (e.source === d.id && e.target === node.id)
        )) {
          edges.push({ source: node.id, target: d.id, curvature: 0 });
        }
      });
    });

    return { nodes, edges };
  }, [users, rooms]);

  // Handle login
  const handleLogin = useCallback(() => {
    if (loginInput.trim()) {
      setUserId(loginInput.trim());
      setIsLoggedIn(true);
      // Connection will be established by the hook
    }
  }, [loginInput]);

  // Handle logout
  const handleLogout = useCallback(() => {
    chatWs.disconnect();
    setIsLoggedIn(false);
    setUserId('');
    setMessages([]);
    setRooms([]);
    setUsers([]);
    setCurrentRoom(null);
    setCurrentRecipient(null);
    setRoutingInfo(new Map());
    clearAnimations();
    resetStats();
  }, [chatWs, clearAnimations, resetStats]);

  // Connect when logged in
  const handleConnect = useCallback(() => {
    if (isLoggedIn && userId) {
      chatWs.connect();
    }
  }, [isLoggedIn, userId, chatWs]);

  // Handle visualizing a route
  const handleVisualizeRoute = useCallback((path: string[], modeStr?: string) => {
    // Parse mode breakdown if provided
    let gravityHops = path.length - 1;
    let pressureHops = 0;
    
    if (modeStr) {
      const parts = modeStr.split('/');
      parts.forEach(part => {
        if (part.startsWith('G:')) gravityHops = parseInt(part.slice(2)) || 0;
        if (part.startsWith('P:')) pressureHops = parseInt(part.slice(2)) || 0;
      });
    }
    
    // Trigger animation for the path with correct modes
    path.forEach((node, index) => {
      if (index < path.length - 1) {
        setTimeout(() => {
          // Determine mode for this hop
          let mode: RoutingMode = 'gravity';
          if (index >= gravityHops && index < gravityHops + pressureHops) {
            mode = 'pressure';
          } else if (index >= gravityHops + pressureHops) {
            mode = 'tree';
          }
          
          const event: RoutingEvent = {
            eventType: index === 0 ? 'packet_sent' : 'packet_hop',
            packetId: `viz-${Date.now()}`,
            fromNode: node,
            toNode: path[index + 1],
            mode,
            timestamp: Date.now(),
          };
          addRoutingEvent(event);
        }, index * 400);
      }
    });
  }, [addRoutingEvent]);

  // Login screen
  if (!isLoggedIn) {
    return (
      <div className="chat-app login-screen">
        <div className="login-container">
          <h1>DRFE-R Chat</h1>
          <p>Decentralized chat powered by hyperbolic routing</p>
          <div className="login-form">
            <input
              type="text"
              placeholder="Enter your username"
              value={loginInput}
              onChange={e => setLoginInput(e.target.value)}
              onKeyPress={e => e.key === 'Enter' && handleLogin()}
            />
            <button onClick={handleLogin} disabled={!loginInput.trim()}>
              Join Chat
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="chat-app">
      {/* Header */}
      <header className="chat-app-header">
        <div className="header-left">
          <h1>DRFE-R Chat</h1>
          <span className="user-badge">@{userId}</span>
        </div>
        <div className="header-right">
          <label className="toggle-label">
            <input
              type="checkbox"
              checked={showVisualization}
              onChange={e => setShowVisualization(e.target.checked)}
            />
            Show Topology
          </label>
          <div className="connection-status">
            <span 
              className={`status-dot ${chatWs.status}`}
              title={chatWs.status}
            />
            <span>{chatWs.status}</span>
          </div>
          {chatWs.status === 'disconnected' && (
            <button className="connect-btn" onClick={handleConnect}>
              Connect
            </button>
          )}
          <button className="logout-btn" onClick={handleLogout}>
            Logout
          </button>
        </div>
      </header>

      {/* Error display */}
      {chatWs.error && (
        <div className="error-banner">
          {chatWs.error}
        </div>
      )}

      {/* Main content */}
      <main className="chat-app-main">
        {/* Chat panel */}
        <div className={`chat-panel-container ${showVisualization ? 'with-viz' : 'full-width'}`}>
          <ChatPanel
            userId={userId}
            messages={messages}
            rooms={rooms}
            users={users}
            currentRoom={currentRoom}
            currentRecipient={currentRecipient}
            routingInfo={routingInfo}
            onSendMessage={chatWs.sendMessage}
            onSendRoomMessage={chatWs.sendRoomMessage}
            onSelectRoom={setCurrentRoom}
            onSelectRecipient={setCurrentRecipient}
            onJoinRoom={chatWs.joinRoom}
            onCreateRoom={chatWs.createRoom}
            onVisualizeRoute={handleVisualizeRoute}
            isConnected={chatWs.status === 'connected'}
          />
        </div>

        {/* Topology visualization */}
        {showVisualization && (
          <div className="visualization-panel">
            <div className="viz-header">
              <h2>Network Topology</h2>
              <div className="viz-stats">
                <span>Packets: {stats.totalPackets}</span>
                <span>Delivered: {stats.deliveredPackets}</span>
                <span>Avg Hops: {stats.averageHops.toFixed(1)}</span>
              </div>
            </div>
            <PoincareDisk
              topology={topology}
              animations={animations}
              showModeIndicator={true}
              activeMode={activeMode}
              options={{
                width: 400,
                height: 400,
                showCurvature: false,
                showLabels: true,
                showEdges: true,
                animateRouting: true,
              }}
            />
            <div className="mode-legend">
              <div className="mode-item gravity">
                <span className="mode-dot"></span>
                <span>Gravity</span>
              </div>
              <div className="mode-item pressure">
                <span className="mode-dot"></span>
                <span>Pressure</span>
              </div>
              <div className="mode-item tree">
                <span className="mode-dot"></span>
                <span>Tree</span>
              </div>
            </div>
            {/* Educational Routing Info Legend */}
            <div className="routing-info-legend">
              <h4>Routing Modes Explained</h4>
              <div className="legend-items">
                <div className="legend-item">
                  <span className="legend-dot gravity"></span>
                  <div className="legend-text">
                    <div className="legend-title">Gravity Mode</div>
                    <div className="legend-desc">Moves toward destination using hyperbolic distance</div>
                  </div>
                </div>
                <div className="legend-item">
                  <span className="legend-dot pressure"></span>
                  <div className="legend-text">
                    <div className="legend-title">Pressure Mode</div>
                    <div className="legend-desc">Escapes local minima when stuck</div>
                  </div>
                </div>
                <div className="legend-item">
                  <span className="legend-dot tree"></span>
                  <div className="legend-text">
                    <div className="legend-title">Tree Mode</div>
                    <div className="legend-desc">Fallback using spanning tree for guaranteed delivery</div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        )}
      </main>
    </div>
  );
}

export default ChatApp;
