# Comprehensive Benchmark Results

Generated: 2026-01-04 10:32:09

## 1. Topology Comparison

| Topology | Nodes | PIE Stretch | TZ Stretch | Improvement |
|----------|-------|-------------|------------|-------------|
| Barabasi-Albert | 1000 | 119.57x | 1.37x | 87.5x |
| Erdos-Renyi | 1000 | 10.52x | 1.58x | 6.6x |
| Watts-Strogatz | 1000 | 5.20x | 1.58x | 3.3x |
| Grid | 961 | 4.92x | 0.80x | 6.1x |
| Tree | 1000 | 13.37x | 0.74x | 18.0x |

## 2. Scalability

| Nodes | PIE Stretch | PIE+TZ Stretch | Max Stretch | Preprocess (ms) |
|-------|-------------|----------------|-------------|-----------------|
| 100 | 3.14x | 1.30x | 3.50x | 4 |
| 500 | 45.01x | 1.40x | 4.00x | 116 |
| 1000 | 119.57x | 1.37x | 3.50x | 461 |
| 2000 | 287.45x | 1.37x | 3.50x | 2140 |
| 3000 | 535.20x | 1.35x | 3.50x | 4692 |
| 5000 | 926.05x | 1.29x | 3.00x | 15203 |
| 7000 | 1241.68x | 1.25x | 3.50x | 41289 |
| 10000 | 1846.69x | 1.25x | 4.00x | 122342 |

## 3. Memory Overhead

| Nodes | TZ Entries | Memory | Bytes/Node | Landmarks |
|-------|------------|--------|------------|-----------|
| 100 | 526 | 25.0 KB | 255.7 | 10 |
| 500 | 4054 | 190.8 KB | 390.7 | 23 |
| 1000 | 11923 | 559.9 KB | 573.3 | 32 |
| 2000 | 32797 | 1.5 MB | 787.8 | 45 |
| 3000 | 57733 | 2.6 MB | 924.3 | 55 |
| 5000 | 104864 | 4.8 MB | 1007.1 | 71 |
| 10000 | 266714 | 12.2 MB | 1280.5 | 100 |

## 4. Latency

| Nodes | PIE Build | TZ Build | Route Decision | Path Compute |
|-------|-----------|----------|----------------|--------------|
| 100 | 0ms | 4ms | 0.45μs | 0.96μs |
| 500 | 0ms | 115ms | 0.52μs | 1.14μs |
| 1000 | 1ms | 433ms | 0.80μs | 1.47μs |
| 2000 | 3ms | 1992ms | 1.46μs | 1.91μs |
| 5000 | 10ms | 13819ms | 1.32μs | 2.50μs |

## 5. Ablation Study

| Nodes | Configuration | Success | Stretch | Max Stretch |
|-------|---------------|---------|---------|-------------|
| 500 | Gravity-only | 21.2% | 1.07x | 2.00x |
| 500 | PIE+DFS | 100.0% | 45.01x | 633.50x |
| 500 | PIE+TZ | 100.0% | 1.40x | 4.00x |
| 500 | TZ-only | 100.0% | 0.90x | 2.00x |
| 1000 | Gravity-only | 15.8% | 1.07x | 2.00x |
| 1000 | PIE+DFS | 100.0% | 119.57x | 1682.33x |
| 1000 | PIE+TZ | 100.0% | 1.37x | 3.50x |
| 1000 | TZ-only | 100.0% | 0.86x | 2.00x |
| 2000 | Gravity-only | 12.4% | 1.08x | 1.67x |
| 2000 | PIE+DFS | 100.0% | 321.30x | 1942.00x |
| 2000 | PIE+TZ | 100.0% | 1.35x | 3.50x |
| 2000 | TZ-only | 100.0% | 0.81x | 1.50x |

---
*All data saved in JSON format for further analysis.*
