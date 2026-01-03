//! Property-based tests for Chat Server Message Routing
//!
//! This module contains property-based tests for the chat server's message routing
//! functionality using DRFE-R protocol.
//!
//! Feature: drfe-r-completion, Property 13: Message Routing
//! Validates: Requirements 13.1

use drfe_r::chat::{ChatMessage, ChatServerState};
use proptest::prelude::*;

// ============================================================================
// Custom Strategies for Chat Testing
// ============================================================================

/// Strategy for generating valid user IDs
fn user_id_strategy() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{2,15}".prop_map(|s| s)
}

/// Strategy for generating valid room IDs
fn room_id_strategy() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_-]{2,20}".prop_map(|s| s)
}

/// Strategy for generating message content
fn message_content_strategy() -> impl Strategy<Value = String> {
    ".{1,500}".prop_map(|s| s)
}

/// Strategy for generating a list of unique user IDs
fn user_list_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<String>> {
    proptest::collection::vec(user_id_strategy(), min..=max)
        .prop_map(|users| {
            // Ensure uniqueness by appending index
            users.into_iter()
                .enumerate()
                .map(|(i, u)| format!("{}_{}", u, i))
                .collect()
        })
}

// ============================================================================
// Property Tests for Message Routing
// ============================================================================

#[cfg(test)]
mod message_routing_properties {
    use super::*;

    // Feature: drfe-r-completion, Property 13: Message Routing
    // Validates: Requirements 13.1
    // For any valid user ID in the chat application, the system must allow message sending
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        
        /// Property 13: Message Routing
        /// For any valid user ID in the chat application, the system must allow message sending
        ///
        /// This test verifies that:
        /// 1. Any registered user can send messages to any other registered user
        /// 2. Messages are routed correctly using DRFE-R protocol
        /// 3. The routing path is valid and reaches the destination
        #[test]
        fn prop_message_routing(
            users in user_list_strategy(2, 10),
            sender_idx in 0usize..10,
            recipient_idx in 0usize..10,
            content in message_content_strategy(),
        ) {
            // Run async test
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                let state = ChatServerState::new();
                
                // Register all users
                for user_id in &users {
                    let result = state.register_user(user_id).await;
                    prop_assert!(result.is_ok(), "User registration should succeed for {}", user_id);
                }
                
                // Build network topology
                state.build_topology().await;
                
                // Select sender and recipient
                let sender_idx = sender_idx % users.len();
                let recipient_idx = recipient_idx % users.len();
                
                // Skip if sender == recipient
                if sender_idx == recipient_idx {
                    return Ok(());
                }
                
                let sender = &users[sender_idx];
                let recipient = &users[recipient_idx];
                
                // Verify both users can send messages to each other
                prop_assert!(
                    state.can_send_message(sender, recipient).await,
                    "Registered users {} and {} should be able to send messages",
                    sender, recipient
                );
                
                // Create and route a message
                let message = ChatMessage::new_text(sender, recipient, &content);
                let routing_result = state.route_message(&message).await;
                
                prop_assert!(
                    routing_result.is_ok(),
                    "Message routing should succeed for {} -> {}",
                    sender, recipient
                );
                
                let result = routing_result.unwrap();
                
                // Verify routing succeeded
                prop_assert!(
                    result.success,
                    "Message from {} to {} should be delivered. Failure: {:?}",
                    sender, recipient, result.failure_reason
                );
                
                // Verify the path starts with sender and ends with recipient
                prop_assert!(
                    !result.path.is_empty(),
                    "Routing path should not be empty"
                );
                
                prop_assert_eq!(
                    &result.path.first().unwrap().0,
                    sender,
                    "Path should start with sender"
                );
                
                prop_assert_eq!(
                    &result.path.last().unwrap().0,
                    recipient,
                    "Path should end with recipient"
                );
                
                Ok(())
            })?;
        }
    }

    // Additional property: Message routing is symmetric
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]
        
        /// Property: Symmetric Message Routing
        /// If user A can send to user B, then user B can send to user A
        #[test]
        fn prop_symmetric_message_routing(
            user_a in user_id_strategy(),
            user_b in user_id_strategy(),
        ) {
            // Ensure different users
            let user_a = format!("{}_a", user_a);
            let user_b = format!("{}_b", user_b);
            
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                let state = ChatServerState::new();
                
                // Register both users
                state.register_user(&user_a).await.unwrap();
                state.register_user(&user_b).await.unwrap();
                
                // Build topology
                state.build_topology().await;
                
                // Check A -> B
                let can_a_to_b = state.can_send_message(&user_a, &user_b).await;
                
                // Check B -> A
                let can_b_to_a = state.can_send_message(&user_b, &user_a).await;
                
                prop_assert_eq!(
                    can_a_to_b, can_b_to_a,
                    "Message routing should be symmetric: A->B={}, B->A={}",
                    can_a_to_b, can_b_to_a
                );
                
                Ok(())
            })?;
        }
    }

    // Property: Unregistered users cannot send messages
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]
        
        /// Property: Unregistered User Rejection
        /// Unregistered users should not be able to send or receive messages
        #[test]
        fn prop_unregistered_user_rejection(
            registered_user in user_id_strategy(),
            unregistered_user in user_id_strategy(),
        ) {
            let registered_user = format!("{}_reg", registered_user);
            let unregistered_user = format!("{}_unreg", unregistered_user);
            
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                let state = ChatServerState::new();
                
                // Only register one user
                state.register_user(&registered_user).await.unwrap();
                
                // Unregistered user should not be able to send
                let can_send = state.can_send_message(&unregistered_user, &registered_user).await;
                prop_assert!(
                    !can_send,
                    "Unregistered user {} should not be able to send messages",
                    unregistered_user
                );
                
                // Registered user should not be able to send to unregistered
                let can_receive = state.can_send_message(&registered_user, &unregistered_user).await;
                prop_assert!(
                    !can_receive,
                    "Should not be able to send to unregistered user {}",
                    unregistered_user
                );
                
                Ok(())
            })?;
        }
    }
}

// ============================================================================
// Property Tests for Room Management
// ============================================================================

#[cfg(test)]
mod room_management_properties {
    use super::*;

    // Property: Room membership is consistent
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]
        
        /// Property: Room Membership Consistency
        /// After joining a room, a user should be a member of that room
        #[test]
        fn prop_room_membership_consistency(
            user_id in user_id_strategy(),
            room_id in room_id_strategy(),
            room_name in "[A-Za-z ]{3,30}",
        ) {
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                let state = ChatServerState::new();
                
                // Register user and create room
                state.register_user(&user_id).await.unwrap();
                state.create_room(&room_id, &room_name).await.unwrap();
                
                // Join room
                state.join_room(&user_id, &room_id).await.unwrap();
                
                // Verify membership
                let room = state.get_room(&room_id).await.unwrap();
                prop_assert!(
                    room.is_member(&user_id),
                    "User {} should be a member of room {} after joining",
                    user_id, room_id
                );
                
                // Leave room
                state.leave_room(&user_id, &room_id).await.unwrap();
                
                // Verify no longer a member
                let room = state.get_room(&room_id).await.unwrap();
                prop_assert!(
                    !room.is_member(&user_id),
                    "User {} should not be a member of room {} after leaving",
                    user_id, room_id
                );
                
                Ok(())
            })?;
        }
    }

    // Property: Room creation is idempotent (fails on duplicate)
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(30))]
        
        /// Property: Room Creation Uniqueness
        /// Creating a room with the same ID twice should fail
        #[test]
        fn prop_room_creation_uniqueness(
            room_id in room_id_strategy(),
            name1 in "[A-Za-z ]{3,20}",
            name2 in "[A-Za-z ]{3,20}",
        ) {
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                let state = ChatServerState::new();
                
                // First creation should succeed
                let result1 = state.create_room(&room_id, &name1).await;
                prop_assert!(result1.is_ok(), "First room creation should succeed");
                
                // Second creation with same ID should fail
                let result2 = state.create_room(&room_id, &name2).await;
                prop_assert!(result2.is_err(), "Duplicate room creation should fail");
                
                Ok(())
            })?;
        }
    }
}

// ============================================================================
// Property Tests for User Registration
// ============================================================================

#[cfg(test)]
mod user_registration_properties {
    use super::*;

    // Property: User registration is idempotent (fails on duplicate)
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]
        
        /// Property: User Registration Uniqueness
        /// Registering the same user ID twice should fail
        #[test]
        fn prop_user_registration_uniqueness(
            user_id in user_id_strategy(),
        ) {
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                let state = ChatServerState::new();
                
                // First registration should succeed
                let result1 = state.register_user(&user_id).await;
                prop_assert!(result1.is_ok(), "First registration should succeed");
                
                // Second registration should fail
                let result2 = state.register_user(&user_id).await;
                prop_assert!(result2.is_err(), "Duplicate registration should fail");
                
                Ok(())
            })?;
        }
    }

    // Property: User coordinates are valid Poincaré disk points
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        
        /// Property: User Coordinate Validity
        /// All user coordinates should be valid points in the Poincaré disk
        #[test]
        fn prop_user_coordinate_validity(
            user_id in user_id_strategy(),
        ) {
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                let state = ChatServerState::new();
                
                let user = state.register_user(&user_id).await.unwrap();
                
                // Coordinate should be inside the Poincaré disk
                let norm = user.coord.euclidean_norm();
                prop_assert!(
                    norm < 1.0,
                    "User coordinate should be inside Poincaré disk: norm = {}",
                    norm
                );
                
                // Coordinate should be near the boundary (anchor coordinates are at r ≈ 0.95)
                prop_assert!(
                    norm > 0.9,
                    "User coordinate should be near boundary: norm = {}",
                    norm
                );
                
                Ok(())
            })?;
        }
    }

    // Property: Unregistration removes user completely
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]
        
        /// Property: Complete User Removal
        /// After unregistration, user should be completely removed from the system
        #[test]
        fn prop_complete_user_removal(
            user_id in user_id_strategy(),
            room_id in room_id_strategy(),
        ) {
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                let state = ChatServerState::new();
                
                // Register user and create room
                state.register_user(&user_id).await.unwrap();
                state.create_room(&room_id, "Test Room").await.unwrap();
                state.join_room(&user_id, &room_id).await.unwrap();
                
                // Verify user exists
                prop_assert!(state.get_user(&user_id).await.is_some());
                
                // Unregister user
                state.unregister_user(&user_id).await;
                
                // Verify user is gone
                prop_assert!(
                    state.get_user(&user_id).await.is_none(),
                    "User should be removed after unregistration"
                );
                
                // Verify user is removed from room
                let room = state.get_room(&room_id).await.unwrap();
                prop_assert!(
                    !room.is_member(&user_id),
                    "User should be removed from rooms after unregistration"
                );
                
                Ok(())
            })?;
        }
    }
}
