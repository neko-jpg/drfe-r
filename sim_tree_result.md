DRFE-R Simulator
================

Configuration:
  Nodes:     100
  Topology:  barabasi-albert
  Tests:     500
  Max TTL:   50
  Seed:      42
  Optimize:  no

Generating network... done (0 ms)
  Nodes: 100
  Edges: 294

Running routing simulation... done

=== Simulation Results ===
Total tests:          500
Successful:           465
Failed:               35
Success rate:         93.00%
Average hops:         3.04
Gravity hops:         1177
Tree hops:            236
Pressure hops:        0
TTL failures:         35
No path failures:     0
Elapsed time:         2 ms

✗ VERIFICATION ISSUE: 93.0% success rate
  Check network connectivity or increase TTL
