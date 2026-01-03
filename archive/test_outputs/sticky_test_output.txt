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
Successful:           459
Failed:               41
Success rate:         91.80%
Average hops:         4.22
Gravity hops:         930
Tree hops:            1006
Pressure hops:        0
TTL failures:         41
No path failures:     0
Elapsed time:         3 ms

✗ VERIFICATION ISSUE: 91.8% success rate
  Check network connectivity or increase TTL
