DRFE-R Simulator
================

Configuration:
  Nodes:     100
  Topology:  barabasi-albert
  Tests:     500
  Max TTL:   200
  Seed:      42
  Optimize:  no

Generating network... done (0 ms)
  Nodes: 100
  Edges: 294

Running routing simulation... done

=== Simulation Results ===
Total tests:          500
Successful:           500
Failed:               0
Success rate:         100.00%
Average hops:         43.14
Gravity hops:         1035
Tree hops:            20536
Pressure hops:        0
TTL failures:         0
No path failures:     0
Elapsed time:         5 ms

✓ VERIFIED: 100% routing success rate achieved
  Theory prediction (Theorem 1): Connected graph + TTL → guaranteed delivery
