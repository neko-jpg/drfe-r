//! Benchmark for API throughput
//!
//! Measures the throughput of the REST API endpoints under various loads.
//! This benchmark evaluates the performance of the API layer and helps
//! identify bottlenecks in request handling.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use drfe_r::api::{
    CoordinateInfo, DeliveryStatus, NodeInfoResponse, PacketStatus, SendPacketRequest,
    TopologyResponse,
};
use governor::{Quota, RateLimiter};
use nonzero_ext::nonzero;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;

/// Benchmark packet status serialization
fn bench_packet_status_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("packet_status_serialization");

    let status = PacketStatus {
        id: "test-packet-123".to_string(),
        status: DeliveryStatus::InTransit {
            current_node: "node_5".to_string(),
        },
        hops: 10,
        path: vec![
            "node_1".to_string(),
            "node_2".to_string(),
            "node_3".to_string(),
            "node_4".to_string(),
            "node_5".to_string(),
        ],
        created_at: 1234567890,
        source: "node_1".to_string(),
        destination: "node_10".to_string(),
    };

    group.bench_function("serialize_json", |b| {
        b.iter(|| {
            let json = serde_json::to_string(&status).unwrap();
            black_box(json);
        });
    });

    group.bench_function("deserialize_json", |b| {
        let json = serde_json::to_string(&status).unwrap();

        b.iter(|| {
            let parsed: PacketStatus = serde_json::from_str(&json).unwrap();
            black_box(parsed);
        });
    });

    group.finish();
}

/// Benchmark node info response creation
fn bench_node_info_response(c: &mut Criterion) {
    let mut group = c.benchmark_group("node_info_response");

    group.bench_function("create_response", |b| {
        b.iter(|| {
            let response = NodeInfoResponse {
                id: "test_node".to_string(),
                coordinate: CoordinateInfo {
                    x: 0.5,
                    y: 0.3,
                    norm: 0.583,
                    version: 12345,
                },
                neighbor_count: 5,
                udp_address: "127.0.0.1:8000".to_string(),
                tcp_address: "127.0.0.1:8001".to_string(),
            };

            black_box(response);
        });
    });

    group.bench_function("serialize_response", |b| {
        let response = NodeInfoResponse {
            id: "test_node".to_string(),
            coordinate: CoordinateInfo {
                x: 0.5,
                y: 0.3,
                norm: 0.583,
                version: 12345,
            },
            neighbor_count: 5,
            udp_address: "127.0.0.1:8000".to_string(),
            tcp_address: "127.0.0.1:8001".to_string(),
        };

        b.iter(|| {
            let json = serde_json::to_string(&response).unwrap();
            black_box(json);
        });
    });

    group.finish();
}

/// Benchmark topology response creation
fn bench_topology_response(c: &mut Criterion) {
    let mut group = c.benchmark_group("topology_response");

    for size in [10, 50, 100, 500].iter() {
        let n = *size;

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("create_topology", n), &n, |b, &n| {
            b.iter(|| {
                let nodes = (0..n)
                    .map(|i| drfe_r::api::TopologyNode {
                        id: format!("node_{}", i),
                        coordinate: CoordinateInfo {
                            x: (i as f64 / n as f64) * 0.8,
                            y: ((i * 7) as f64 / n as f64) * 0.8,
                            norm: 0.5,
                            version: i as u64,
                        },
                        is_local: i == 0,
                    })
                    .collect();

                let edges = (0..n - 1)
                    .map(|i| drfe_r::api::TopologyEdge {
                        source: format!("node_{}", i),
                        target: format!("node_{}", i + 1),
                        distance: 0.1,
                    })
                    .collect();

                let response = TopologyResponse { nodes, edges };

                black_box(response);
            });
        });
    }

    group.finish();
}

/// Benchmark packet tracker operations
fn bench_packet_tracker(c: &mut Criterion) {
    let mut group = c.benchmark_group("packet_tracker");
    let rt = Runtime::new().unwrap();

    group.bench_function("insert_packet", |b| {
        let tracker = Arc::new(RwLock::new(HashMap::new()));

        b.iter(|| {
            rt.block_on(async {
                let mut t = tracker.write().await;
                let status = PacketStatus {
                    id: format!("packet_{}", rand::random::<u32>()),
                    status: DeliveryStatus::Pending,
                    hops: 0,
                    path: vec!["node_1".to_string()],
                    created_at: 1234567890,
                    source: "node_1".to_string(),
                    destination: "node_2".to_string(),
                };
                t.insert(status.id.clone(), status);
                black_box(&t);
            });
        });
    });

    group.bench_function("lookup_packet", |b| {
        let tracker = Arc::new(RwLock::new(HashMap::new()));

        // Pre-populate with 1000 packets
        rt.block_on(async {
            let mut t = tracker.write().await;
            for i in 0..1000 {
                let status = PacketStatus {
                    id: format!("packet_{}", i),
                    status: DeliveryStatus::Pending,
                    hops: 0,
                    path: vec!["node_1".to_string()],
                    created_at: 1234567890,
                    source: "node_1".to_string(),
                    destination: "node_2".to_string(),
                };
                t.insert(status.id.clone(), status);
            }
        });

        b.iter(|| {
            rt.block_on(async {
                let t = tracker.read().await;
                let result = t.get("packet_500");
                black_box(result);
            });
        });
    });

    group.finish();
}

/// Benchmark rate limiter performance
fn bench_rate_limiter(c: &mut Criterion) {
    let mut group = c.benchmark_group("rate_limiter");

    group.bench_function("check_rate_limit", |b| {
        let quota = Quota::per_minute(nonzero!(10000u32));
        let limiter = RateLimiter::keyed(quota);

        b.iter(|| {
            let _ = limiter.check_key(&"test_client".to_string());
        });
    });

    group.bench_function("check_multiple_clients", |b| {
        let quota = Quota::per_minute(nonzero!(10000u32));
        let limiter = RateLimiter::keyed(quota);

        b.iter(|| {
            for i in 0..10 {
                let client = format!("client_{}", i);
                let _ = limiter.check_key(&client);
            }
        });
    });

    group.finish();
}

/// Benchmark request validation
fn bench_request_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("request_validation");

    group.bench_function("validate_send_packet", |b| {
        let request = SendPacketRequest {
            destination: "node_123".to_string(),
            payload: "Hello, world!".to_string(),
            ttl: 64,
        };

        b.iter(|| {
            // Validation logic
            let valid = !request.destination.is_empty()
                && request.ttl > 0
                && request.ttl <= 255
                && !request.payload.is_empty();

            black_box(valid);
        });
    });

    group.finish();
}

/// Benchmark concurrent packet tracking
fn bench_concurrent_tracking(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_tracking");
    let rt = Runtime::new().unwrap();

    for concurrency in [1, 5, 10, 20].iter() {
        let n = *concurrency;

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("concurrent_inserts", n), &n, |b, &n| {
            let tracker = Arc::new(RwLock::new(HashMap::new()));

            b.iter(|| {
                rt.block_on(async {
                    let mut handles = vec![];

                    for i in 0..n {
                        let tracker_clone = Arc::clone(&tracker);
                        let handle = tokio::spawn(async move {
                            let mut t = tracker_clone.write().await;
                            let status = PacketStatus {
                                id: format!("packet_{}", i),
                                status: DeliveryStatus::Pending,
                                hops: 0,
                                path: vec!["node_1".to_string()],
                                created_at: 1234567890,
                                source: "node_1".to_string(),
                                destination: "node_2".to_string(),
                            };
                            t.insert(status.id.clone(), status);
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.await.unwrap();
                    }

                    black_box(&tracker);
                });
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_packet_status_serialization,
    bench_node_info_response,
    bench_topology_response,
    bench_packet_tracker,
    bench_rate_limiter,
    bench_request_validation,
    bench_concurrent_tracking
);
criterion_main!(benches);
