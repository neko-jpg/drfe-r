# Experiment Methodology

## 1. Experimental Environment

### 1.1 Simulator Implementation
- **Language**: Rust 2021 Edition
- **Platform**: Linux (Ubuntu on WSL2)
- **Compilation**: Release mode with optimizations (`cargo build --release`)
- **Random Seed**: Fixed seed (12345) for reproducibility

### 1.2 Hardware
- Experiments run on a single machine (no distributed deployment)
- Results represent algorithmic performance, not network latency

---

## 2. Network Topology Generation

Five topology types were evaluated:

| Topology | Generation Method | Parameters |
|----------|------------------|------------|
| **Barabási-Albert (BA)** | Preferential attachment | m = 3 (edges per new node) |
| **Watts-Strogatz (WS)** | Small-world rewiring | k = 6, β = 0.1 |
| **Grid** | 2D lattice | √N × √N |
| **Line** | Linear chain | N-1 edges |
| **Lollipop** | Clique + tail | Head ratio = 0.33 |

**Scale**: N ∈ {50, 100, 200, 300} nodes per topology.

---

## 3. Embedding Strategies Compared

| Strategy | Description |
|----------|-------------|
| **PIE** | Polar Increasing-angle Embedding (greedy-guaranteed) |
| **Random** | Uniform random coordinates in Poincaré disk |
| **Ricci-Broken** | Ricci Flow with Euclidean gradient (buggy) |
| **Ricci-Fixed** | Ricci Flow with Riemannian gradient (corrected) |

---

## 4. Evaluation Metrics

### 4.1 Primary Metrics

| Metric | Definition |
|--------|-----------|
| **Success Rate** | (Delivered packets) / (Total packets) |
| **Average Hops** | Mean hop count for successful deliveries |
| **Stretch** | (Actual hops) / (Shortest path hops) |

### 4.2 Mode Distribution

| Metric | Definition |
|--------|-----------|
| **Gravity Ratio** | Hops in Gravity mode / Total hops |
| **Pressure Ratio** | Hops in Pressure mode / Total hops |
| **Tree Ratio** | Hops in Tree (DFS) mode / Total hops |

Higher Gravity ratio indicates better embedding quality.

---

## 5. Experimental Protocol

1. **Network Generation**: Create topology with fixed seed
2. **Embedding**: Apply specified embedding strategy
3. **Ricci Flow** (if applicable): 30 iterations, step size 0.05
4. **Routing Tests**: 200 random source-destination pairs per configuration
5. **TTL Setting**: 2N (twice the node count)
6. **Data Collection**: Record hops, mode transitions, success/failure

---

## 6. Statistical Considerations

- **Sample Size**: 200 routing tests per configuration
- **Total Experiments**: 5 topologies × 4 scales × 4 embeddings = 80 configurations
- **Total Routing Tests**: 80 × 200 = 16,000 individual routing simulations
- **Reproducibility**: All experiments use fixed random seed

---

## 7. Baseline Comparisons

PIE embedding serves as the primary baseline because:
1. Theoretically guarantees greedy routing success on trees
2. No additional optimization required
3. Establishes upper bound for embedding quality

---

## 8. Limitations

1. **Simulation Only**: No real network deployment
2. **Static Topology**: Node/link failures not evaluated
3. **No Traffic Load**: Packets routed independently (no congestion)
4. **Single Seed**: Results may vary with different random seeds

---

## Key Findings Summary

| Embedding | Avg Success | Avg Hops | Gravity % |
|-----------|-------------|----------|-----------|
| **PIE** | **99.7%** | **24.3** | **28.8%** |
| Random | 94.8% | 88.7 | 2.1% |
| Ricci-Broken | 93.9% | 99.0 | 1.4% |
| Ricci-Fixed | 93.6% | 91.1 | 1.7% |

*Averaged across BA, WS, Grid topologies with N ≥ 100*
