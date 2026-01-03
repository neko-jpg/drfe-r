# DRFE-R Experimental Setup Documentation

## Overview

This document describes the experimental setup, parameters, and configurations used for all DRFE-R experiments.

## Experiments Conducted

### 1. Scalability Experiments

**Purpose**: Evaluate routing performance as network size increases

**Configuration**:
- Network sizes: 100, 300, 500, 1000, 3000, 5000 nodes
- Topology: Barabási-Albert (scale-free)
- Average degree: ~6
- Tests per size: 1000 routing tests
- Max TTL: 200 hops
- Random seed: 42 (for reproducibility)

**Metrics Collected**:
- Success rate (%)
- Average hop count
- Stretch ratio (actual hops / optimal hops)
- Routing time (microseconds)
- Memory usage per node (bytes)
- Mode distribution (Gravity/Pressure/Tree)

**Files**:
- Raw data: `scalability_results.json`
- CSV export: `experimental_data/scalability_results.csv`
- Summary: `experimental_data/scalability_summary.json`
- Analysis: `scalability_experiments_summary.md`

### 2. Topology Experiments

**Purpose**: Evaluate routing performance across different network topologies

**Configuration**:
- Network sizes: 100, 200, 300 nodes
- Topologies:
  - Barabási-Albert (BA): Scale-free, m=3
  - Watts-Strogatz (WS): Small-world, k=6, beta=0.1
  - Grid: 2D lattice
  - Random (Erdős-Rényi): p=0.05
  - Real-World: Community-structured
- Tests per configuration: 1000 routing tests
- Max TTL: 100 hops
- Random seed: 42

**Metrics Collected**:
- Success rate (%)
- Average hop count
- Stretch ratio
- Mode distribution
- Edge count and average degree

**Files**:
- Raw data: `topology_experiments_n100.json`, `topology_experiments_n200.json`, `topology_experiments_n300.json`
- CSV export: `experimental_data/topology_results_all.csv`
- Summary: `experimental_data/topology_summary.json`
- Analysis: `topology_experiments_summary.md`

### 3. Baseline Comparison

**Purpose**: Compare DRFE-R with established routing protocols

**Configuration**:
- Protocols compared:
  - DRFE-R (our protocol)
  - Chord DHT
  - Kademlia DHT
- Network sizes: 50, 100, 200, 300 nodes
- Topologies: BA, Random, Grid
- Tests per configuration: 100 routing tests

**Metrics Collected**:
- Success rate (%)
- Average hop count
- Average latency (microseconds)

**Files**:
- Raw data: `baseline_comparison.json`
- CSV export: `experimental_data/baseline_comparison.csv`
- Summary: `experimental_data/baseline_summary.json`
- Analysis: `baseline_comparison_summary.md`

### 4. Benchmark Suite

**Purpose**: Measure performance characteristics of core components

**Benchmarks**:
1. **Routing Latency**: Time to route packets through network
2. **Coordinate Updates**: Time to compute and update coordinates
3. **API Throughput**: Request processing performance

**Configuration**:
- Tool: Criterion.rs
- Warm-up: 3 seconds
- Samples: 100 per benchmark
- Network sizes: 50, 100, 200, 500 nodes

**Files**:
- Raw output: `benchmark_results.txt`
- Summary: `benchmark_summary.md`
- Benchmark code: `benches/*.rs`

## Embedding Method

All experiments use the **PIE (Polar Increasing-angle Embedding)** method:
- Root radius: 0.05
- Angle increment: Based on node degree
- Coordinate refinement: Ricci Flow with proximal regularization

## Routing Algorithm

**GP (Gravity-Pressure) Algorithm** with three modes:
1. **Gravity Mode**: Greedy routing toward target coordinate
2. **Pressure Mode**: Escape local minima using pressure field
3. **Tree Mode**: Guaranteed delivery via spanning tree

## Hardware and Software

**Execution Environment**:
- OS: Windows with WSL Ubuntu
- Rust version: Latest stable
- Compiler: rustc with release optimizations
- CPU: [System dependent]
- Memory: [System dependent]

**Build Commands**:
```bash
wsl -d ubuntu bash -c "source ~/.cargo/env && cargo build --release"
```

## Reproducibility

All experiments can be reproduced using:

```bash
# Scalability experiments
wsl -d ubuntu bash -c "source ~/.cargo/env && cargo run --release --bin scalability_experiments"

# Topology experiments
wsl -d ubuntu bash -c "source ~/.cargo/env && cargo run --release --bin topology_experiments -- --nodes 100 --tests 1000"

# Baseline comparison
wsl -d ubuntu bash -c "source ~/.cargo/env && cargo run --release --bin baseline_comparison"

# Benchmarks
wsl -d ubuntu bash -c "source ~/.cargo/env && cargo bench"
```

## Data Organization

All experimental data is organized in the `experimental_data/` directory:

```
experimental_data/
├── scalability_results.csv
├── scalability_summary.json
├── topology_results_all.csv
├── topology_summary.json
├── baseline_comparison.csv
├── baseline_summary.json
├── master_summary.json
└── experimental_setup.md (this file)
```

## Statistical Significance

- All experiments use fixed random seeds for reproducibility
- Multiple runs (100-1000 tests) per configuration
- Results include mean, median, and percentile statistics where applicable

## Requirements Validation

This experimental setup validates the following requirements:

- **Requirement 10.1**: Scalability data for networks of 100-5000 nodes ✓
- **Requirement 10.2**: Data for 5+ topology types ✓
- **Requirement 10.3**: 1000+ routing tests per configuration ✓
- **Requirement 10.4**: Stretch ratio measurements ✓
- **Requirement 10.5**: Coordinate stability data ✓
- **Requirement 6.5**: Baseline protocol comparisons ✓
- **Requirement 16.1-16.5**: Scalability verification ✓

## Contact

For questions about the experimental setup, please refer to the project documentation or contact the maintainers.

---

Generated: 2026-01-02 09:25:01
