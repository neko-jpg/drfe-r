//! Tests for checkpoint/restore functionality
//!
//! These tests verify that DistributedNode state can be checkpointed and restored correctly.

use drfe_r::coordinates::NodeId;
use drfe_r::network::{DistributedNode, NeighborInfo, NodeCheckpoint};
use drfe_r::PoincareDiskPoint;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

/// Test checkpoint creation
#[tokio::test]
async fn test_checkpoint_creation() {
    let node = DistributedNode::new(
        NodeId::new("test_node"),
        "127.0.0.1:0",
        "127.0.0.1:0",
    )
    .await
    .unwrap();

    // Add some neighbors
    let neighbor1 = NeighborInfo::new(
        NodeId::new("neighbor1"),
        PoincareDiskPoint::new(0.3, 0.0).unwrap(),
        "127.0.0.1:8001".parse().unwrap(),
    );
    let neighbor2 = NeighborInfo::new(
        NodeId::new("neighbor2"),
        PoincareDiskPoint::new(0.0, 0.3).unwrap(),
        "127.0.0.1:8002".parse().unwrap(),
    );

    node.add_neighbor(neighbor1).await;
    node.add_neighbor(neighbor2).await;

    // Create checkpoint
    let checkpoint = node.create_checkpoint().await;

    // Verify checkpoint contents
    assert_eq!(checkpoint.node_id, "test_node");
    assert_eq!(checkpoint.neighbors.len(), 2);
    assert_eq!(checkpoint.version, NodeCheckpoint::VERSION);
    assert!(checkpoint.is_compatible());

    // Verify neighbor information
    let neighbor_ids: Vec<String> = checkpoint
        .neighbors
        .iter()
        .map(|n| n.id.clone())
        .collect();
    assert!(neighbor_ids.contains(&"neighbor1".to_string()));
    assert!(neighbor_ids.contains(&"neighbor2".to_string()));
}

/// Test checkpoint serialization to JSON
#[tokio::test]
async fn test_checkpoint_json_serialization() {
    let node = DistributedNode::new(
        NodeId::new("test_node"),
        "127.0.0.1:0",
        "127.0.0.1:0",
    )
    .await
    .unwrap();

    let checkpoint = node.create_checkpoint().await;

    // Serialize to JSON
    let json = checkpoint.to_json().expect("Failed to serialize to JSON");
    assert!(!json.is_empty());
    assert!(json.contains("test_node"));

    // Deserialize from JSON
    let restored = NodeCheckpoint::from_json(&json).expect("Failed to deserialize from JSON");

    assert_eq!(restored.node_id, checkpoint.node_id);
    assert_eq!(restored.coord_version, checkpoint.coord_version);
    assert_eq!(restored.neighbors.len(), checkpoint.neighbors.len());
    assert_eq!(restored.version, checkpoint.version);
}

/// Test checkpoint serialization to MessagePack
#[tokio::test]
async fn test_checkpoint_msgpack_serialization() {
    let node = DistributedNode::new(
        NodeId::new("test_node"),
        "127.0.0.1:0",
        "127.0.0.1:0",
    )
    .await
    .unwrap();

    let checkpoint = node.create_checkpoint().await;

    // Serialize to MessagePack
    let bytes = checkpoint
        .to_msgpack()
        .expect("Failed to serialize to MessagePack");
    assert!(!bytes.is_empty());

    // Deserialize from MessagePack
    let restored =
        NodeCheckpoint::from_msgpack(&bytes).expect("Failed to deserialize from MessagePack");

    assert_eq!(restored.node_id, checkpoint.node_id);
    assert_eq!(restored.coord_version, checkpoint.coord_version);
    assert_eq!(restored.neighbors.len(), checkpoint.neighbors.len());
}

/// Test checkpoint save and load from file
#[tokio::test]
async fn test_checkpoint_file_operations() {
    let node = DistributedNode::new(
        NodeId::new("test_node"),
        "127.0.0.1:0",
        "127.0.0.1:0",
    )
    .await
    .unwrap();

    // Add a neighbor
    let neighbor = NeighborInfo::new(
        NodeId::new("neighbor1"),
        PoincareDiskPoint::new(0.5, 0.5).unwrap(),
        "127.0.0.1:8001".parse().unwrap(),
    );
    node.add_neighbor(neighbor).await;

    // Create temporary directory
    let temp_dir = TempDir::new().unwrap();
    let checkpoint_path = temp_dir.path().join("checkpoint.json");

    // Save checkpoint
    node.save_checkpoint(&checkpoint_path)
        .await
        .expect("Failed to save checkpoint");

    // Verify file exists
    assert!(checkpoint_path.exists());

    // Load checkpoint
    let loaded = NodeCheckpoint::load_from_file(&checkpoint_path)
        .expect("Failed to load checkpoint");

    assert_eq!(loaded.node_id, "test_node");
    assert_eq!(loaded.neighbors.len(), 1);
    assert_eq!(loaded.neighbors[0].id, "neighbor1");
}

/// Test restore from checkpoint
#[tokio::test]
async fn test_restore_from_checkpoint() {
    // Create first node and checkpoint it
    let node1 = DistributedNode::new(
        NodeId::new("test_node"),
        "127.0.0.1:0",
        "127.0.0.1:0",
    )
    .await
    .unwrap();

    // Update coordinate
    let new_coord = PoincareDiskPoint::new(0.4, 0.3).unwrap();
    node1.update_coordinates(new_coord).await.unwrap();

    // Add neighbors
    let neighbor1 = NeighborInfo::new(
        NodeId::new("neighbor1"),
        PoincareDiskPoint::new(0.3, 0.0).unwrap(),
        "127.0.0.1:8001".parse().unwrap(),
    );
    let neighbor2 = NeighborInfo::new(
        NodeId::new("neighbor2"),
        PoincareDiskPoint::new(0.0, 0.3).unwrap(),
        "127.0.0.1:8002".parse().unwrap(),
    );

    node1.add_neighbor(neighbor1).await;
    node1.add_neighbor(neighbor2).await;

    // Create checkpoint
    let checkpoint = node1.create_checkpoint().await;

    // Create second node (simulating restart)
    let node2 = DistributedNode::new(
        NodeId::new("test_node"),
        "127.0.0.1:0",
        "127.0.0.1:0",
    )
    .await
    .unwrap();

    // Verify node2 starts with different state
    assert_eq!(node2.neighbors().await.len(), 0);

    // Restore from checkpoint
    node2
        .restore_from_checkpoint(&checkpoint)
        .await
        .expect("Failed to restore from checkpoint");

    // Verify state was restored
    let restored_coord = node2.coord().await;
    assert!((restored_coord.point.x - 0.4).abs() < 1e-10);
    assert!((restored_coord.point.y - 0.3).abs() < 1e-10);

    let restored_neighbors = node2.neighbors().await;
    assert_eq!(restored_neighbors.len(), 2);

    let neighbor_ids: Vec<String> = restored_neighbors
        .iter()
        .map(|n| n.id.0.clone())
        .collect();
    assert!(neighbor_ids.contains(&"neighbor1".to_string()));
    assert!(neighbor_ids.contains(&"neighbor2".to_string()));
}

/// Test restore from file after simulated crash
#[tokio::test]
async fn test_restore_after_simulated_crash() {
    let temp_dir = TempDir::new().unwrap();
    let checkpoint_path = temp_dir.path().join("checkpoint.json");

    // Phase 1: Create node, add state, save checkpoint
    {
        let node = DistributedNode::new(
            NodeId::new("crash_test_node"),
            "127.0.0.1:0",
            "127.0.0.1:0",
        )
        .await
        .unwrap();

        // Update coordinate
        let coord = PoincareDiskPoint::new(0.6, 0.2).unwrap();
        node.update_coordinates(coord).await.unwrap();

        // Add neighbors
        for i in 0..3 {
            let neighbor = NeighborInfo::new(
                NodeId::new(&format!("neighbor{}", i)),
                PoincareDiskPoint::new(0.1 * (i as f64 + 1.0), 0.1).unwrap(),
                format!("127.0.0.1:{}", 8000 + i).parse().unwrap(),
            );
            node.add_neighbor(neighbor).await;
        }

        // Save checkpoint
        node.save_checkpoint(&checkpoint_path).await.unwrap();

        // Simulate crash (node goes out of scope)
    }

    // Phase 2: Create new node and restore from checkpoint
    {
        let node = DistributedNode::new(
            NodeId::new("crash_test_node"),
            "127.0.0.1:0",
            "127.0.0.1:0",
        )
        .await
        .unwrap();

        // Restore from file
        node.restore_from_file(&checkpoint_path)
            .await
            .expect("Failed to restore from file");

        // Verify state was restored
        let coord = node.coord().await;
        assert!((coord.point.x - 0.6).abs() < 1e-10);
        assert!((coord.point.y - 0.2).abs() < 1e-10);

        let neighbors = node.neighbors().await;
        assert_eq!(neighbors.len(), 3);

        // Verify all neighbors were restored
        for i in 0..3 {
            let neighbor_id = format!("neighbor{}", i);
            assert!(neighbors.iter().any(|n| n.id.0 == neighbor_id));
        }
    }
}

/// Test checkpoint version compatibility
#[tokio::test]
async fn test_checkpoint_version_compatibility() {
    let node = DistributedNode::new(
        NodeId::new("test_node"),
        "127.0.0.1:0",
        "127.0.0.1:0",
    )
    .await
    .unwrap();

    let checkpoint = node.create_checkpoint().await;

    // Current version should be compatible
    assert!(checkpoint.is_compatible());

    // Create checkpoint with incompatible version
    let mut incompatible = checkpoint.clone();
    incompatible.version = 999;

    assert!(!incompatible.is_compatible());

    // Attempting to restore from incompatible checkpoint should fail
    let result = node.restore_from_checkpoint(&incompatible).await;
    assert!(result.is_err());
}

/// Test checkpoint age calculation
#[tokio::test]
async fn test_checkpoint_age() {
    let node = DistributedNode::new(
        NodeId::new("test_node"),
        "127.0.0.1:0",
        "127.0.0.1:0",
    )
    .await
    .unwrap();

    let checkpoint = node.create_checkpoint().await;

    // Age should be very small (just created)
    let age = checkpoint.age_seconds();
    assert!(age < 2, "Checkpoint age should be less than 2 seconds");

    // Wait a bit
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Age should have increased
    let new_age = checkpoint.age_seconds();
    assert!(new_age >= 1, "Checkpoint age should be at least 1 second");
}

/// Test restore with wrong node ID
#[tokio::test]
async fn test_restore_wrong_node_id() {
    let node1 = DistributedNode::new(
        NodeId::new("node1"),
        "127.0.0.1:0",
        "127.0.0.1:0",
    )
    .await
    .unwrap();

    let checkpoint = node1.create_checkpoint().await;

    // Try to restore to different node
    let node2 = DistributedNode::new(
        NodeId::new("node2"),
        "127.0.0.1:0",
        "127.0.0.1:0",
    )
    .await
    .unwrap();

    let result = node2.restore_from_checkpoint(&checkpoint).await;
    assert!(result.is_err(), "Should fail when restoring to wrong node");
}

/// Test periodic checkpointing
#[tokio::test]
async fn test_periodic_checkpointing() {
    let temp_dir = TempDir::new().unwrap();
    let checkpoint_dir = temp_dir.path().to_path_buf();

    let node = Arc::new(
        DistributedNode::new(
            NodeId::new("periodic_test_node"),
            "127.0.0.1:0",
            "127.0.0.1:0",
        )
        .await
        .unwrap(),
    );

    // Start periodic checkpointing with short interval
    let checkpoint_interval = Duration::from_millis(500);
    let node_clone = Arc::clone(&node);
    let handle = node_clone.start_periodic_checkpointing(checkpoint_dir.clone(), checkpoint_interval);

    // Wait for a few checkpoints to be created
    tokio::time::sleep(Duration::from_millis(1600)).await;

    // Stop checkpointing
    node.shutdown().await;
    handle.abort();

    // Verify checkpoints were created
    let checkpoints: Vec<_> = std::fs::read_dir(&checkpoint_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("checkpoint_periodic_test_node_")
        })
        .collect();

    // Should have created at least 2 checkpoints
    assert!(
        checkpoints.len() >= 2,
        "Should have created at least 2 checkpoints, found {}",
        checkpoints.len()
    );

    // Should not have more than 5 checkpoints (cleanup keeps last 5)
    assert!(
        checkpoints.len() <= 5,
        "Should not have more than 5 checkpoints, found {}",
        checkpoints.len()
    );
}

/// Test finding latest checkpoint
#[tokio::test]
async fn test_find_latest_checkpoint() {
    let temp_dir = TempDir::new().unwrap();
    let checkpoint_dir = temp_dir.path();

    let node = DistributedNode::new(
        NodeId::new("test_node"),
        "127.0.0.1:0",
        "127.0.0.1:0",
    )
    .await
    .unwrap();

    // No checkpoints initially
    assert!(node.find_latest_checkpoint(checkpoint_dir).is_none());

    // Create multiple checkpoints
    for i in 0..3 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let checkpoint_path = checkpoint_dir.join(format!("checkpoint_test_node_{}.json", i));
        node.save_checkpoint(&checkpoint_path).await.unwrap();
    }

    // Find latest checkpoint
    let latest = node
        .find_latest_checkpoint(checkpoint_dir)
        .expect("Should find latest checkpoint");

    // Verify it's the most recent one
    assert!(latest.to_string_lossy().contains("checkpoint_test_node_"));
}

/// Test restore on startup
#[tokio::test]
async fn test_restore_on_startup() {
    let temp_dir = TempDir::new().unwrap();
    let checkpoint_dir = temp_dir.path();

    // Create node and save checkpoint
    {
        let node = DistributedNode::new(
            NodeId::new("startup_test_node"),
            "127.0.0.1:0",
            "127.0.0.1:0",
        )
        .await
        .unwrap();

        let coord = PoincareDiskPoint::new(0.7, 0.1).unwrap();
        node.update_coordinates(coord).await.unwrap();

        let checkpoint_path = checkpoint_dir.join("checkpoint_startup_test_node_1.json");
        node.save_checkpoint(&checkpoint_path).await.unwrap();
    }

    // Create new node and restore on startup
    {
        let node = DistributedNode::new(
            NodeId::new("startup_test_node"),
            "127.0.0.1:0",
            "127.0.0.1:0",
        )
        .await
        .unwrap();

        let restored = node
            .restore_on_startup(checkpoint_dir)
            .await
            .expect("Failed to restore on startup");

        assert!(restored, "Should have restored from checkpoint");

        let coord = node.coord().await;
        assert!((coord.point.x - 0.7).abs() < 1e-10);
        assert!((coord.point.y - 0.1).abs() < 1e-10);
    }
}

/// Test restore on startup with no checkpoint
#[tokio::test]
async fn test_restore_on_startup_no_checkpoint() {
    let temp_dir = TempDir::new().unwrap();
    let checkpoint_dir = temp_dir.path();

    let node = DistributedNode::new(
        NodeId::new("no_checkpoint_node"),
        "127.0.0.1:0",
        "127.0.0.1:0",
    )
    .await
    .unwrap();

    let restored = node
        .restore_on_startup(checkpoint_dir)
        .await
        .expect("Should succeed even with no checkpoint");

    assert!(!restored, "Should not have restored (no checkpoint exists)");
}

/// Test checkpoint with empty neighbor list
#[tokio::test]
async fn test_checkpoint_empty_neighbors() {
    let node = DistributedNode::new(
        NodeId::new("lonely_node"),
        "127.0.0.1:0",
        "127.0.0.1:0",
    )
    .await
    .unwrap();

    let checkpoint = node.create_checkpoint().await;

    assert_eq!(checkpoint.neighbors.len(), 0);
    assert!(checkpoint.is_compatible());

    // Should be able to restore from checkpoint with no neighbors
    let node2 = DistributedNode::new(
        NodeId::new("lonely_node"),
        "127.0.0.1:0",
        "127.0.0.1:0",
    )
    .await
    .unwrap();

    node2
        .restore_from_checkpoint(&checkpoint)
        .await
        .expect("Should restore successfully");

    assert_eq!(node2.neighbors().await.len(), 0);
}

/// Test checkpoint preserves coordinate version
#[tokio::test]
async fn test_checkpoint_preserves_version() {
    let node = DistributedNode::new(
        NodeId::new("version_test_node"),
        "127.0.0.1:0",
        "127.0.0.1:0",
    )
    .await
    .unwrap();

    // Update coordinates multiple times to increment version
    for i in 0..5 {
        let coord = PoincareDiskPoint::new(0.1 * (i as f64 + 1.0), 0.1).unwrap();
        node.update_coordinates(coord).await.unwrap();
    }

    let coord_before = node.coord().await;
    let checkpoint = node.create_checkpoint().await;

    // Create new node and restore
    let node2 = DistributedNode::new(
        NodeId::new("version_test_node"),
        "127.0.0.1:0",
        "127.0.0.1:0",
    )
    .await
    .unwrap();

    node2.restore_from_checkpoint(&checkpoint).await.unwrap();

    let coord_after = node2.coord().await;

    // Version should be preserved
    assert_eq!(coord_after.updated_at, coord_before.updated_at);
    assert_eq!(coord_after.updated_at, checkpoint.coord_version);
}
