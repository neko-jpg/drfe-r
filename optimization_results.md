# Optimization Results (Pressure Fallback + Ricci Flow)
Date: Wed Dec 31 15:09:32 JST 2025

## Baseline: Pressure Fallback (No Ricci)
DRFE-R Simulator
================

Configuration:
  Nodes:     300
  Topology:  ws
  Tests:     100
  Max TTL:   600
  Seed:      12345
  Optimize:  no

Generating network... done (1 ms)
  Nodes: 300
  Edges: 900

Running routing simulation... done

=== Simulation Results ===
Total tests:          100
Successful:           100
Failed:               0
Success rate:         100.00%
Average hops:         16.70
Avg Stretch:          3.443
Gravity hops:         525
Tree hops:            678
Pressure hops:        467
TTL failures:         0
No path failures:     0
Elapsed time:         5 ms

✓ VERIFIED: 100% routing success rate achieved
  Theory prediction (Theorem 1): Connected graph + TTL → guaranteed delivery
DRFE-R Simulator
================

Configuration:
  Nodes:     300
  Topology:  grid
  Tests:     100
  Max TTL:   600
  Seed:      12345
  Optimize:  no

Generating network... done (0 ms)
  Nodes: 300
  Edges: 565

Running routing simulation... done

=== Simulation Results ===
Total tests:          100
Successful:           100
Failed:               0
Success rate:         100.00%
Average hops:         58.84
Avg Stretch:          5.012
Gravity hops:         703
Tree hops:            1773
Pressure hops:        3408
TTL failures:         0
No path failures:     0
Elapsed time:         6 ms

✓ VERIFIED: 100% routing success rate achieved
  Theory prediction (Theorem 1): Connected graph + TTL → guaranteed delivery
## Optimized: Pressure Fallback + Ricci Flow
DRFE-R Simulator
================

Configuration:
  Nodes:     300
  Topology:  ws
  Tests:     100
  Max TTL:   600
  Seed:      12345
  Optimize:  yes

Generating network... done (1 ms)
  Nodes: 300
  Edges: 900

Running Ricci Flow optimization (30 iterations)... done (527 ms, residual stress: 1634.5626)

Running routing simulation... done

=== Simulation Results ===
Total tests:          100
Successful:           94
Failed:               6
Success rate:         94.00%
Average hops:         228.46
Avg Stretch:          47.094
Gravity hops:         136
Tree hops:            18520
Pressure hops:        2819
TTL failures:         6
No path failures:     0
Elapsed time:         12 ms

✗ VERIFICATION ISSUE: 94.0% success rate
  Check network connectivity or increase TTL
DRFE-R Simulator
================

Configuration:
  Nodes:     300
  Topology:  grid
  Tests:     100
  Max TTL:   600
  Seed:      12345
  Optimize:  yes

Generating network... done (1 ms)
  Nodes: 300
  Edges: 565

Running Ricci Flow optimization (30 iterations)... done (226 ms, residual stress: 527.4972)

Running routing simulation... done

=== Simulation Results ===
Total tests:          100
Successful:           90
Failed:               10
Success rate:         90.00%
Average hops:         246.03
Avg Stretch:          20.870
Gravity hops:         112
Tree hops:            17995
Pressure hops:        4036
TTL failures:         10
No path failures:     0
Elapsed time:         11 ms

✗ VERIFICATION ISSUE: 90.0% success rate
  Check network connectivity or increase TTL
