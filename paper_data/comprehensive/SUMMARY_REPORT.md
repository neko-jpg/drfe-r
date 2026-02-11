# Comprehensive Benchmark Results

Generated: 2026-02-11 21:57:57

## 1. Topology Comparison

| Topology | Nodes | PIE Stretch | TZ Stretch | Improvement |
|----------|-------|-------------|------------|-------------|
| Barabasi-Albert | 1000 | 119.57x | 1.70x | 70.3x |
| Erdos-Renyi | 1000 | 10.52x | 1.91x | 5.5x |
| Watts-Strogatz | 1000 | 3.45x | 1.70x | 2.0x |
| Grid | 961 | 7.05x | 1.02x | 6.9x |
| Tree | 1000 | 76.70x | 1.02x | 75.3x |

## 2. Scalability

| Nodes | PIE Stretch | PIE+TZ Stretch | Max Stretch | Preprocess (ms) |
|-------|-------------|----------------|-------------|-----------------|
| 100 | 3.14x | 1.44x | 4.00x | 6 |
| 500 | 45.01x | 1.69x | 4.50x | 30 |
| 1000 | 119.57x | 1.70x | 4.50x | 118 |
| 2000 | 287.45x | 1.73x | 4.50x | 611 |
| 3000 | 535.20x | 1.75x | 4.50x | 1319 |
| 5000 | 926.05x | 1.70x | 3.67x | 9430 |
| 7000 | 1241.68x | 1.66x | 4.00x | 18556 |
| 10000 | 1846.69x | 1.66x | 5.00x | 47621 |

## 3. Memory Overhead

| Nodes | TZ Entries | Memory | Bytes/Node | Landmarks |
|-------|------------|--------|------------|-----------|
| 100 | 2506 | 117.8 KB | 1206.1 | 10 |
| 500 | 27008 | 1.2 MB | 2594.2 | 23 |
| 1000 | 75859 | 3.5 MB | 3642.3 | 32 |
| 2000 | 212707 | 9.7 MB | 5105.7 | 45 |
| 3000 | 387623 | 17.7 MB | 6202.6 | 55 |
| 5000 | 814722 | 37.3 MB | 7821.8 | 71 |
| 10000 | 2266514 | 103.8 MB | 10879.6 | 100 |

## 4. Latency

| Nodes | PIE Build | TZ Build | Route Decision | Path Compute |
|-------|-----------|----------|----------------|--------------|
| 100 | 0ms | 116ms | 0.47μs | 1.31μs |
| 500 | 1ms | 24ms | 0.67μs | 1.75μs |
| 1000 | 2ms | 115ms | 1.12μs | 2.85μs |
| 2000 | 4ms | 564ms | 2.35μs | 7.18μs |
| 5000 | 18ms | 7843ms | 1.86μs | 4.48μs |

## 5. Ablation Study

| Nodes | Configuration | Success | Stretch | Max Stretch |
|-------|---------------|---------|---------|-------------|
| 500 | Gravity-only | 21.2% | 1.07x | 2.00x |
| 500 | PIE+DFS | 100.0% | 45.01x | 633.50x |
| 500 | PIE+TZ | 100.0% | 1.69x | 4.50x |
| 500 | TZ-only | 100.0% | 1.52x | 3.00x |
| 1000 | Gravity-only | 15.8% | 1.07x | 2.00x |
| 1000 | PIE+DFS | 100.0% | 119.57x | 1682.33x |
| 1000 | PIE+TZ | 100.0% | 1.70x | 4.50x |
| 1000 | TZ-only | 100.0% | 1.17x | 3.00x |
| 2000 | Gravity-only | 12.4% | 1.08x | 1.67x |
| 2000 | PIE+DFS | 100.0% | 321.30x | 1942.00x |
| 2000 | PIE+TZ | 100.0% | 1.73x | 4.00x |
| 2000 | TZ-only | 100.0% | 1.38x | 3.50x |

---
*All data saved in JSON format for further analysis.*
