//! Chat Server Backend for DRFE-R
//!
//! This module implements a WebSocket-based chat server that uses DRFE-R
//! for message routing. It provides:
//! - WebSocket server with axum
//! - Message routing via DRFE-R protocol
//! - Chat room management
//!
//! Requirements: 13.1, 13.2, 13.3

use crate::coordinates::{NodeId, RoutingCoordinate, AnchorCoordinate};
use crate::routing::{GPRouter, RoutingNode, DeliveryResult};
use crate::PoincareDiskPoint;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Maximum message size in bytes (64 KB)
pub const MAX_MESSAGE_SIZE: usize = 65536;

/// Chat message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatMessageType {
    /// Text message from user
    Text { content: String },
    /// User joined notification
    UserJoined { user_id: String },
    /// User left notification
    UserLeft { user_id: String },
    /// Room created notification
    RoomCreated { room_id: String },
    /// Routing info (for visualization)
    RoutingInfo { path: Vec<String>, mode: String, hops: u32 },
    /// Error message
    Error { message: String },
    /// System message
    System { message: String },
}

/// Chat message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Unique message ID
    pub id: String,
    /// Sender user ID
    pub sender: String,
    /// Recipient user ID (or room ID for room messages)
    pub recipient: String,
    /// Message type and content
    pub message_type: ChatMessageType,
    /// Timestamp (milliseconds since epoch)
    pub timestamp: u64,
    /// Room ID (if message is in a room)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_id: Option<String>,
}

impl ChatMessage {
    /// Create a new text message
    pub fn new_text(sender: &str, recipient: &str, content: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            sender: sender.to_string(),
            recipient: recipient.to_string(),
            message_type: ChatMessageType::Text {
                content: content.to_string(),
            },
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            room_id: None,
        }
    }

    /// Create a room message
    pub fn new_room_text(sender: &str, room_id: &str, content: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            sender: sender.to_string(),
            recipient: room_id.to_string(),
            message_type: ChatMessageType::Text {
                content: content.to_string(),
            },
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            room_id: Some(room_id.to_string()),
        }
    }

    /// Create a system message
    pub fn new_system(recipient: &str, message: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            sender: "system".to_string(),
            recipient: recipient.to_string(),
            message_type: ChatMessageType::System {
                message: message.to_string(),
            },
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            room_id: None,
        }
    }

    /// Create an error message
    pub fn new_error(recipient: &str, error: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            sender: "system".to_string(),
            recipient: recipient.to_string(),
            message_type: ChatMessageType::Error {
                message: error.to_string(),
            },
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            room_id: None,
        }
    }

    /// Create a routing info message
    pub fn new_routing_info(recipient: &str, path: Vec<String>, mode: &str, hops: u32) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            sender: "system".to_string(),
            recipient: recipient.to_string(),
            message_type: ChatMessageType::RoutingInfo {
                path,
                mode: mode.to_string(),
                hops,
            },
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            room_id: None,
        }
    }
}

/// Chat room structure
#[derive(Debug, Clone)]
pub struct ChatRoom {
    /// Room ID
    pub id: String,
    /// Room name
    pub name: String,
    /// Room members (user IDs)
    pub members: Vec<String>,
    /// Room anchor coordinate (for routing)
    pub anchor_coord: PoincareDiskPoint,
    /// Created timestamp
    pub created_at: u64,
}

impl ChatRoom {
    /// Create a new chat room
    pub fn new(id: &str, name: &str) -> Self {
        // Compute anchor coordinate from room ID
        let anchor = AnchorCoordinate::from_id(&NodeId::new(id));
        
        Self {
            id: id.to_string(),
            name: name.to_string(),
            members: Vec::new(),
            anchor_coord: anchor.point,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }

    /// Add a member to the room
    pub fn add_member(&mut self, user_id: &str) {
        if !self.members.contains(&user_id.to_string()) {
            self.members.push(user_id.to_string());
        }
    }

    /// Remove a member from the room
    pub fn remove_member(&mut self, user_id: &str) {
        self.members.retain(|m| m != user_id);
    }

    /// Check if user is a member
    pub fn is_member(&self, user_id: &str) -> bool {
        self.members.contains(&user_id.to_string())
    }
}

/// Connected user information
#[derive(Debug, Clone)]
pub struct ConnectedUser {
    /// User ID
    pub id: String,
    /// User's routing coordinate
    pub coord: PoincareDiskPoint,
    /// Rooms the user has joined
    pub rooms: Vec<String>,
    /// Connection timestamp
    pub connected_at: u64,
}

impl ConnectedUser {
    /// Create a new connected user
    pub fn new(id: &str) -> Self {
        // Compute anchor coordinate from user ID
        let anchor = AnchorCoordinate::from_id(&NodeId::new(id));
        
        Self {
            id: id.to_string(),
            coord: anchor.point,
            rooms: Vec::new(),
            connected_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }

    /// Join a room
    pub fn join_room(&mut self, room_id: &str) {
        if !self.rooms.contains(&room_id.to_string()) {
            self.rooms.push(room_id.to_string());
        }
    }

    /// Leave a room
    pub fn leave_room(&mut self, room_id: &str) {
        self.rooms.retain(|r| r != room_id);
    }
}


/// Chat server state
#[derive(Clone)]
pub struct ChatServerState {
    /// Connected users (user_id -> user info)
    pub users: Arc<RwLock<HashMap<String, ConnectedUser>>>,
    /// Chat rooms (room_id -> room info)
    pub rooms: Arc<RwLock<HashMap<String, ChatRoom>>>,
    /// GP Router for message routing
    pub router: Arc<RwLock<GPRouter>>,
    /// Broadcast channel for messages
    pub broadcast_tx: broadcast::Sender<ChatMessage>,
    /// User-specific channels (user_id -> sender)
    pub user_channels: Arc<RwLock<HashMap<String, broadcast::Sender<ChatMessage>>>>,
}

impl ChatServerState {
    /// Create a new chat server state
    pub fn new() -> Self {
        let (broadcast_tx, _) = broadcast::channel(1000);
        
        Self {
            users: Arc::new(RwLock::new(HashMap::new())),
            rooms: Arc::new(RwLock::new(HashMap::new())),
            router: Arc::new(RwLock::new(GPRouter::new())),
            broadcast_tx,
            user_channels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new user
    pub async fn register_user(&self, user_id: &str) -> Result<ConnectedUser, String> {
        let mut users = self.users.write().await;
        
        if users.contains_key(user_id) {
            return Err(format!("User {} already connected", user_id));
        }
        
        let user = ConnectedUser::new(user_id);
        users.insert(user_id.to_string(), user.clone());
        
        // Add user to router
        let mut router = self.router.write().await;
        let coord = RoutingCoordinate::new(user.coord, 0);
        let node = RoutingNode::new(NodeId::new(user_id), coord);
        router.add_node(node);
        
        // Create user-specific channel
        let (tx, _) = broadcast::channel(100);
        let mut channels = self.user_channels.write().await;
        channels.insert(user_id.to_string(), tx);
        
        Ok(user)
    }

    /// Unregister a user
    pub async fn unregister_user(&self, user_id: &str) {
        let mut users = self.users.write().await;
        
        if let Some(user) = users.remove(user_id) {
            // Remove from all rooms
            let mut rooms = self.rooms.write().await;
            for room_id in &user.rooms {
                if let Some(room) = rooms.get_mut(room_id) {
                    room.remove_member(user_id);
                }
            }
        }
        
        // Remove user channel
        let mut channels = self.user_channels.write().await;
        channels.remove(user_id);
    }

    /// Create a new chat room
    pub async fn create_room(&self, room_id: &str, name: &str) -> Result<ChatRoom, String> {
        let mut rooms = self.rooms.write().await;
        
        if rooms.contains_key(room_id) {
            return Err(format!("Room {} already exists", room_id));
        }
        
        let room = ChatRoom::new(room_id, name);
        rooms.insert(room_id.to_string(), room.clone());
        
        // Add room as a node in the router (for routing to room anchor)
        let mut router = self.router.write().await;
        let coord = RoutingCoordinate::new(room.anchor_coord, 0);
        let node = RoutingNode::new(NodeId::new(room_id), coord);
        router.add_node(node);
        
        Ok(room)
    }

    /// Join a room
    pub async fn join_room(&self, user_id: &str, room_id: &str) -> Result<(), String> {
        let mut users = self.users.write().await;
        let mut rooms = self.rooms.write().await;
        
        let user = users.get_mut(user_id)
            .ok_or_else(|| format!("User {} not found", user_id))?;
        
        let room = rooms.get_mut(room_id)
            .ok_or_else(|| format!("Room {} not found", room_id))?;
        
        user.join_room(room_id);
        room.add_member(user_id);
        
        // Add edge between user and room in router
        let mut router = self.router.write().await;
        router.add_edge(&NodeId::new(user_id), &NodeId::new(room_id));
        
        Ok(())
    }

    /// Leave a room
    pub async fn leave_room(&self, user_id: &str, room_id: &str) -> Result<(), String> {
        let mut users = self.users.write().await;
        let mut rooms = self.rooms.write().await;
        
        let user = users.get_mut(user_id)
            .ok_or_else(|| format!("User {} not found", user_id))?;
        
        let room = rooms.get_mut(room_id)
            .ok_or_else(|| format!("Room {} not found", room_id))?;
        
        user.leave_room(room_id);
        room.remove_member(user_id);
        
        Ok(())
    }

    /// Route a message using DRFE-R protocol
    /// 
    /// This is the core routing function that uses the GP algorithm
    /// to route messages between users.
    /// 
    /// **Validates: Requirements 13.1** - Allow users to send messages to any other user by ID
    pub async fn route_message(&self, message: &ChatMessage) -> Result<DeliveryResult, String> {
        let router = self.router.read().await;
        
        // Get sender and recipient coordinates
        let sender_id = NodeId::new(&message.sender);
        let recipient_id = NodeId::new(&message.recipient);
        
        // Get recipient's anchor coordinate (computable by anyone)
        let recipient_anchor = AnchorCoordinate::from_id(&recipient_id);
        
        // Simulate routing with high TTL to ensure delivery
        // TTL of 200 is sufficient for networks up to 10,000 nodes
        let result = router.simulate_delivery(
            &sender_id,
            &recipient_id,
            recipient_anchor.point,
            200, // TTL - increased for guaranteed delivery in large networks
        );
        
        Ok(result)
    }

    /// Send a message to a user
    pub async fn send_message(&self, message: ChatMessage) -> Result<(), String> {
        // Route the message
        let routing_result = self.route_message(&message).await?;
        
        if !routing_result.success {
            return Err(format!(
                "Message routing failed: {}",
                routing_result.failure_reason.unwrap_or_else(|| "Unknown error".to_string())
            ));
        }
        
        // Get recipient's channel
        let channels = self.user_channels.read().await;
        
        if let Some(tx) = channels.get(&message.recipient) {
            // Send message to recipient
            let _ = tx.send(message.clone());
            
            // Send routing info to sender
            if let Some(sender_tx) = channels.get(&message.sender) {
                let routing_info = ChatMessage::new_routing_info(
                    &message.sender,
                    routing_result.path.iter().map(|n| n.0.clone()).collect(),
                    &format!("G:{}/P:{}/T:{}", 
                        routing_result.gravity_hops,
                        routing_result.pressure_hops,
                        routing_result.tree_hops),
                    routing_result.hops,
                );
                let _ = sender_tx.send(routing_info);
            }
        } else {
            return Err(format!("Recipient {} not connected", message.recipient));
        }
        
        Ok(())
    }

    /// Send a message to a room
    pub async fn send_room_message(&self, message: ChatMessage) -> Result<(), String> {
        let room_id = message.room_id.as_ref()
            .ok_or_else(|| "Room ID not specified".to_string())?;
        
        let rooms = self.rooms.read().await;
        let room = rooms.get(room_id)
            .ok_or_else(|| format!("Room {} not found", room_id))?;
        
        // Send to all room members except sender
        let channels = self.user_channels.read().await;
        
        for member_id in &room.members {
            if member_id != &message.sender {
                if let Some(tx) = channels.get(member_id) {
                    let _ = tx.send(message.clone());
                }
            }
        }
        
        Ok(())
    }

    /// Get user by ID
    pub async fn get_user(&self, user_id: &str) -> Option<ConnectedUser> {
        let users = self.users.read().await;
        users.get(user_id).cloned()
    }

    /// Get room by ID
    pub async fn get_room(&self, room_id: &str) -> Option<ChatRoom> {
        let rooms = self.rooms.read().await;
        rooms.get(room_id).cloned()
    }

    /// Get all connected users
    pub async fn get_all_users(&self) -> Vec<ConnectedUser> {
        let users = self.users.read().await;
        users.values().cloned().collect()
    }

    /// Get all rooms
    pub async fn get_all_rooms(&self) -> Vec<ChatRoom> {
        let rooms = self.rooms.read().await;
        rooms.values().cloned().collect()
    }

    /// Check if a user can send messages to another user
    /// 
    /// **Validates: Requirements 13.1** - Allow users to send messages to any other user by ID
    pub async fn can_send_message(&self, sender_id: &str, recipient_id: &str) -> bool {
        let users = self.users.read().await;
        
        // Both users must be connected
        users.contains_key(sender_id) && users.contains_key(recipient_id)
    }

    /// Build network topology for routing
    /// 
    /// Creates edges between users based on their proximity in hyperbolic space
    pub async fn build_topology(&self) {
        let users = self.users.read().await;
        let mut router = self.router.write().await;
        
        // Create edges between nearby users (k-nearest neighbors)
        let user_list: Vec<_> = users.values().collect();
        let k = 5; // Number of nearest neighbors
        
        for user in &user_list {
            // Find k nearest neighbors
            let mut distances: Vec<_> = user_list.iter()
                .filter(|u| u.id != user.id)
                .map(|u| {
                    let dist = user.coord.hyperbolic_distance(&u.coord);
                    (u.id.clone(), dist)
                })
                .collect();
            
            distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            
            // Add edges to k nearest neighbors
            for (neighbor_id, _) in distances.iter().take(k) {
                router.add_edge(&NodeId::new(&user.id), &NodeId::new(neighbor_id));
            }
        }
    }
}

impl Default for ChatServerState {
    fn default() -> Self {
        Self::new()
    }
}


/// WebSocket message from client
#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Send a direct message to another user
    SendMessage {
        recipient: String,
        content: String,
    },
    /// Send a message to a room
    SendRoomMessage {
        room_id: String,
        content: String,
    },
    /// Join a room
    JoinRoom {
        room_id: String,
    },
    /// Leave a room
    LeaveRoom {
        room_id: String,
    },
    /// Create a new room
    CreateRoom {
        room_id: String,
        name: String,
    },
    /// Get list of rooms
    ListRooms,
    /// Get list of users
    ListUsers,
    /// Ping (keepalive)
    Ping,
}

/// WebSocket message to client
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Chat message
    Message(ChatMessage),
    /// Room list
    RoomList { rooms: Vec<RoomInfo> },
    /// User list
    UserList { users: Vec<UserInfo> },
    /// Success response
    Success { message: String },
    /// Error response
    Error { message: String },
    /// Pong (keepalive response)
    Pong,
}

/// Room info for listing
#[derive(Debug, Serialize)]
pub struct RoomInfo {
    pub id: String,
    pub name: String,
    pub member_count: usize,
}

/// User info for listing
#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: String,
    pub coord_x: f64,
    pub coord_y: f64,
}

/// Create the chat WebSocket router
pub fn create_chat_router(state: ChatServerState) -> Router {
    Router::new()
        .route("/ws/:user_id", get(websocket_handler))
        .with_state(state)
}

/// WebSocket handler
async fn websocket_handler(
    ws: WebSocketUpgrade,
    Path(user_id): Path<String>,
    State(state): State<ChatServerState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_websocket(socket, user_id, state))
}

/// Handle WebSocket connection
async fn handle_websocket(socket: WebSocket, user_id: String, state: ChatServerState) {
    let (mut sender, mut receiver) = socket.split();
    
    // Register user
    let _user = match state.register_user(&user_id).await {
        Ok(u) => u,
        Err(e) => {
            let error_msg = ServerMessage::Error { message: e };
            let _ = sender.send(Message::Text(serde_json::to_string(&error_msg).unwrap().into())).await;
            return;
        }
    };
    
    // Send welcome message
    let welcome = ChatMessage::new_system(&user_id, &format!("Welcome, {}!", user_id));
    let welcome_msg = ServerMessage::Message(welcome);
    let _ = sender.send(Message::Text(serde_json::to_string(&welcome_msg).unwrap().into())).await;
    
    // Subscribe to user's channel
    let mut user_rx = {
        let channels = state.user_channels.read().await;
        channels.get(&user_id).map(|tx| tx.subscribe())
    };
    
    // Spawn task to forward messages from channel to WebSocket
    let sender = Arc::new(tokio::sync::Mutex::new(sender));
    let sender_clone = Arc::clone(&sender);
    let forward_task = tokio::spawn(async move {
        if let Some(ref mut rx) = user_rx {
            while let Ok(msg) = rx.recv().await {
                let server_msg = ServerMessage::Message(msg);
                let mut sender = sender_clone.lock().await;
                if sender.send(Message::Text(serde_json::to_string(&server_msg).unwrap().into())).await.is_err() {
                    break;
                }
            }
        }
    });
    
    // Handle incoming messages
    let sender_for_recv = Arc::clone(&sender);
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                let response = handle_client_message(&state, &user_id, &text).await;
                let mut sender = sender_for_recv.lock().await;
                let _ = sender.send(Message::Text(serde_json::to_string(&response).unwrap().into())).await;
            }
            Ok(Message::Close(_)) => break,
            Err(_) => break,
            _ => {}
        }
    }
    
    // Cleanup
    forward_task.abort();
    state.unregister_user(&user_id).await;
}

/// Handle a client message
async fn handle_client_message(
    state: &ChatServerState,
    user_id: &str,
    text: &str,
) -> ServerMessage {
    let client_msg: ClientMessage = match serde_json::from_str(text) {
        Ok(msg) => msg,
        Err(e) => return ServerMessage::Error { message: format!("Invalid message: {}", e) },
    };
    
    match client_msg {
        ClientMessage::SendMessage { recipient, content } => {
            let message = ChatMessage::new_text(user_id, &recipient, &content);
            match state.send_message(message).await {
                Ok(_) => ServerMessage::Success { message: "Message sent".to_string() },
                Err(e) => ServerMessage::Error { message: e },
            }
        }
        ClientMessage::SendRoomMessage { room_id, content } => {
            let message = ChatMessage::new_room_text(user_id, &room_id, &content);
            match state.send_room_message(message).await {
                Ok(_) => ServerMessage::Success { message: "Message sent to room".to_string() },
                Err(e) => ServerMessage::Error { message: e },
            }
        }
        ClientMessage::JoinRoom { room_id } => {
            match state.join_room(user_id, &room_id).await {
                Ok(_) => ServerMessage::Success { message: format!("Joined room {}", room_id) },
                Err(e) => ServerMessage::Error { message: e },
            }
        }
        ClientMessage::LeaveRoom { room_id } => {
            match state.leave_room(user_id, &room_id).await {
                Ok(_) => ServerMessage::Success { message: format!("Left room {}", room_id) },
                Err(e) => ServerMessage::Error { message: e },
            }
        }
        ClientMessage::CreateRoom { room_id, name } => {
            match state.create_room(&room_id, &name).await {
                Ok(_) => ServerMessage::Success { message: format!("Created room {}", room_id) },
                Err(e) => ServerMessage::Error { message: e },
            }
        }
        ClientMessage::ListRooms => {
            let rooms = state.get_all_rooms().await;
            let room_infos: Vec<RoomInfo> = rooms.iter().map(|r| RoomInfo {
                id: r.id.clone(),
                name: r.name.clone(),
                member_count: r.members.len(),
            }).collect();
            ServerMessage::RoomList { rooms: room_infos }
        }
        ClientMessage::ListUsers => {
            let users = state.get_all_users().await;
            let user_infos: Vec<UserInfo> = users.iter().map(|u| UserInfo {
                id: u.id.clone(),
                coord_x: u.coord.x,
                coord_y: u.coord.y,
            }).collect();
            ServerMessage::UserList { users: user_infos }
        }
        ClientMessage::Ping => ServerMessage::Pong,
    }
}

/// Start the chat server
///
/// # Arguments
/// * `bind_addr` - Address to bind the server (e.g., "0.0.0.0:8080")
///
/// # Returns
/// Result indicating success or error
pub async fn start_chat_server(bind_addr: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let state = ChatServerState::new();
    let app = create_chat_router(state);
    
    let addr: std::net::SocketAddr = bind_addr.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    
    tracing::info!("Chat server listening on {}", addr);
    
    axum::serve(listener, app).await?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_chat_server_state_creation() {
        let state = ChatServerState::new();
        assert!(state.get_all_users().await.is_empty());
        assert!(state.get_all_rooms().await.is_empty());
    }

    #[tokio::test]
    async fn test_user_registration() {
        let state = ChatServerState::new();
        
        let user = state.register_user("alice").await.unwrap();
        assert_eq!(user.id, "alice");
        
        // Duplicate registration should fail
        let result = state.register_user("alice").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_user_unregistration() {
        let state = ChatServerState::new();
        
        state.register_user("alice").await.unwrap();
        assert!(state.get_user("alice").await.is_some());
        
        state.unregister_user("alice").await;
        assert!(state.get_user("alice").await.is_none());
    }

    #[tokio::test]
    async fn test_room_creation() {
        let state = ChatServerState::new();
        
        let room = state.create_room("general", "General Chat").await.unwrap();
        assert_eq!(room.id, "general");
        assert_eq!(room.name, "General Chat");
        
        // Duplicate room should fail
        let result = state.create_room("general", "Another General").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_join_leave_room() {
        let state = ChatServerState::new();
        
        state.register_user("alice").await.unwrap();
        state.create_room("general", "General Chat").await.unwrap();
        
        // Join room
        state.join_room("alice", "general").await.unwrap();
        
        let room = state.get_room("general").await.unwrap();
        assert!(room.is_member("alice"));
        
        // Leave room
        state.leave_room("alice", "general").await.unwrap();
        
        let room = state.get_room("general").await.unwrap();
        assert!(!room.is_member("alice"));
    }

    #[tokio::test]
    async fn test_can_send_message() {
        let state = ChatServerState::new();
        
        state.register_user("alice").await.unwrap();
        state.register_user("bob").await.unwrap();
        
        // Both users connected - can send
        assert!(state.can_send_message("alice", "bob").await);
        
        // Unregister bob
        state.unregister_user("bob").await;
        
        // Bob not connected - cannot send
        assert!(!state.can_send_message("alice", "bob").await);
    }

    #[tokio::test]
    async fn test_message_routing() {
        let state = ChatServerState::new();
        
        // Register users
        state.register_user("alice").await.unwrap();
        state.register_user("bob").await.unwrap();
        
        // Build topology (creates edges between users)
        state.build_topology().await;
        
        // Create message
        let message = ChatMessage::new_text("alice", "bob", "Hello, Bob!");
        
        // Route message
        let result = state.route_message(&message).await.unwrap();
        
        // Should succeed (both users are in the network)
        assert!(result.success);
    }

    #[test]
    fn test_chat_message_creation() {
        let msg = ChatMessage::new_text("alice", "bob", "Hello!");
        assert_eq!(msg.sender, "alice");
        assert_eq!(msg.recipient, "bob");
        
        match msg.message_type {
            ChatMessageType::Text { content } => assert_eq!(content, "Hello!"),
            _ => panic!("Expected Text message type"),
        }
    }

    #[test]
    fn test_chat_room_creation() {
        let room = ChatRoom::new("general", "General Chat");
        assert_eq!(room.id, "general");
        assert_eq!(room.name, "General Chat");
        assert!(room.members.is_empty());
        
        // Anchor coordinate should be valid (inside Poincaré disk)
        assert!(room.anchor_coord.euclidean_norm() < 1.0);
    }

    #[test]
    fn test_connected_user_creation() {
        let user = ConnectedUser::new("alice");
        assert_eq!(user.id, "alice");
        assert!(user.rooms.is_empty());
        
        // Coordinate should be valid (inside Poincaré disk)
        assert!(user.coord.euclidean_norm() < 1.0);
    }
}
