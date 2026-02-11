# DRFE-R: Compact Routing with Adversarial Churn Resilience

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

A Rust implementation of hybrid compact routing combining **Poincaré Isometric Embedding (PIE)** with **Thorup-Zwick (TZ) distance oracles**, designed and evaluated for resilience under adversarial node churn.

## Motivation

Thorup-Zwick compact routing (2001) guarantees stretch ≤ 3 on static graphs, but its behavior under dynamic conditions — node failures, targeted attacks, network churn — has never been systematically measured. This project provides:

1. **The first empirical quantification of TZ routing degradation under adversarial churn** — targeted removal of 20% of nodes collapses TZ reachability to 0.7%
2. **A hybrid PIE+TZ routing scheme** with gravity-based forwarding as a TZ-free fast path
3. **A sub-100ms rebuild mechanism** that restores 100% reachability after arbitrary node removal
4. **Reproducible experiments** across 5 seeds, 4 topology types, and up to 10,000 nodes

## Key Results

### Adversarial Churn Resilience (1,000 nodes, Barabási-Albert)

| Strategy | Random 20% removal | Targeted 20% removal |
|---|---|---|
| PIE only (Gravity+Pressure+Tree) | 100.0% / stretch 134.7 | 50.4% / stretch 56.6 |
| PIE+TZ (stale table) | 9.7% / stretch 1.15 | **0.7%** / stretch 0.80 |
| **PIE+TZ+Rebuild** | **100.0% / stretch 1.71** | **100.0% / stretch 1.48** |

### Scalability (5-seed average, PIE+TZ)

| Nodes | Stretch | Max Stretch | Preprocessing |
|---|---|---|---|
| 1,000 | 1.63–1.74× | 4.0–5.5× | 85–125 ms |
| 5,000 | 1.63–1.70× | 3.5–4.5× | 3.7–9.4 s |
| 10,000 | 1.59–1.66× | 3.0–5.0× | 30–55 s |

### Ablation Study (1,000 nodes)

| Configuration | Success Rate | Stretch |
|---|---|---|
| Gravity-only | 15.8% | 1.07× |
| PIE+DFS (no TZ) | 100.0% | 119.6× |
| **PIE+TZ** | **100.0%** | **1.70×** |
| TZ-only | 100.0% | 1.17× |

> **Honest limitation:** On static graphs, TZ-only achieves better stretch (1.17×) than PIE+TZ (1.70×). The hybrid approach trades path optimality for churn resilience and reduced TZ table lookups.

### Dynamic Network (500 nodes, 20 rounds of churn)

- **With rebuild:** 100% reachability across all 20 rounds
- **Stale TZ:** degrades to ~89% on node additions, robust to edge changes
- **Rebuild cost:** 30–56 ms per round

## Architecture

```
src/
├── lib.rs                # Poincaré disk model (hyperbolic distance)
├── coordinates.rs        # Dual coordinate system (Anchor / Routing)
├── greedy_embedding.rs   # PIE: Poincaré Isometric Embedding
├── routing.rs            # Gravity → Pressure → TZ → Tree routing
├── tz_routing.rs         # Thorup-Zwick compact routing (rayon-parallelized)
├── ricci.rs              # Ollivier-Ricci flow (Sinkhorn / Forman)
├── network.rs            # Network layer
├── network_tls.rs        # TLS-encrypted transport
├── api.rs                # REST API (Axum)
├── grpc.rs               # gRPC service (Tonic)
├── chat.rs               # WebSocket P2P messaging
├── audit.rs              # Structured audit logging
├── byzantine.rs          # Byzantine fault detection
├── sybil.rs              # Sybil resistance
└── bin/
    ├── comprehensive_benchmark.rs   # Multi-seed scalability & ablation
    ├── churn_robustness.rs          # Adversarial churn experiments
    ├── dynamic_network_experiment.rs # Online topology change tests
    ├── large_topology_benchmark.rs  # 1K–10K node multi-topology tests
    ├── topology_experiments.rs      # BA / WS / Grid / Random / RealWorld
    └── simulator.rs                 # Interactive simulator

frontend/   # React + TypeScript visualization (Poincaré disk)
tests/      # Integration & property-based tests
benches/    # Criterion benchmarks (latency, throughput)
```

## Build & Run

### Prerequisites

- Rust 1.70+ (edition 2021)
- Linux or WSL recommended for benchmarks

### Build

```bash
cargo build --release
```

### Run experiments

```bash
# Comprehensive benchmark (5 seeds × 8 sizes, ~5 min)
cargo run --release --bin comprehensive_benchmark

# Adversarial churn resilience
cargo run --release --bin churn_robustness

# Dynamic network churn (20 rounds)
cargo run --release --bin dynamic_network_experiment

# Large-scale multi-topology (1K–5K nodes)
cargo run --release --bin large_topology_benchmark -- --sizes 1000,2000,5000

# Topology comparison (BA, WS, Grid, Random, RealWorld)
cargo run --release --bin topology_experiments
```

### Run tests

```bash
cargo test
```

### Output

All experiment results are saved as JSON in `paper_data/`:

```
paper_data/
├── comprehensive/
│   ├── scalability_multiseed.json   # 5-seed scalability data
│   ├── scalability_ci_report.md     # 95% confidence intervals
│   ├── ablation_results.json
│   ├── memory_results.json
│   ├── latency_results.json
│   └── SUMMARY_REPORT.md
├── churn/
│   ├── churn_robustness.json        # Targeted/random attack results
│   └── dynamic_network_results.json # 20-round churn experiment
└── real_world/
    └── large_topology_results.json  # Multi-topology benchmark
```

## Algorithm

### Routing Modes (in order of activation)

1. **Gravity**: Forward to the neighbor closest to the destination in hyperbolic space. Zero table lookups. Succeeds for ~15–35% of pairs.
2. **Pressure**: Escape local minima by exploring neighbors with a budget of N/2 hops.
3. **Thorup-Zwick**: Compact routing via √n landmarks and per-node bunches. Stretch ≤ 3 (theoretical).
4. **Tree (DFS)**: Last-resort fallback along the BFS spanning tree. Guarantees delivery but high stretch.

### TZ Rebuild Under Churn

When nodes are removed (detected via heartbeat timeout), the system:
1. Computes the surviving subgraph
2. Rebuilds the TZ table on the surviving nodes (parallelized with rayon)
3. Resumes routing with the fresh table

Measured rebuild time: **30–56 ms** for 500-node networks.

## Theoretical Background

- **Thorup-Zwick (2001)**: Compact routing with stretch ≤ 3, O(n^{1/2}) space per node
- **Kleinberg (2007)**: Geographic routing in hyperbolic space
- **Papadopoulos et al. (2010)**: Greedy forwarding in the hyperbolic plane
- **Ollivier (2009)**: Ricci curvature on metric spaces

## Citation

If you use this software, please cite:

```bibtex
@software{drfe_r_2026,
  title  = {DRFE-R: Compact Routing with Adversarial Churn Resilience},
  author = {neko-jpg},
  year   = {2026},
  url    = {https://github.com/neko-jpg/drfe-r}
}
```

## License

MIT License
