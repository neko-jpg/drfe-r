# Robustness Verification Results
Date: Wed Dec 31 14:45:42 JST 2025

## Topology: ba (N=100)
DRFE-R Simulator
================

Configuration:
  Nodes:     100
  Topology:  ba
  Tests:     100
  Max TTL:   200
  Seed:      12345
  Optimize:  no

Generating network... done (0 ms)
  Nodes: 100
  Edges: 294

Running routing simulation... done

=== Simulation Results ===
Total tests:          100
Successful:           100
Failed:               0
Success rate:         100.00%
Average hops:         35.61
Gravity hops:         230
Tree hops:            3331
Pressure hops:        0
TTL failures:         0
No path failures:     0
Elapsed time:         0 ms

✓ VERIFIED: 100% routing success rate achieved
  Theory prediction (Theorem 1): Connected graph + TTL → guaranteed delivery

## Topology: ws (N=100)
DRFE-R Simulator
================

Configuration:
  Nodes:     100
  Topology:  ws
  Tests:     100
  Max TTL:   200
  Seed:      12345
  Optimize:  no

Generating network... done (0 ms)
  Nodes: 100
  Edges: 300

Running routing simulation... done

=== Simulation Results ===
Total tests:          100
Successful:           100
Failed:               0
Success rate:         100.00%
Average hops:         15.60
Gravity hops:         438
Tree hops:            1122
Pressure hops:        0
TTL failures:         0
No path failures:     0
Elapsed time:         0 ms

✓ VERIFIED: 100% routing success rate achieved
  Theory prediction (Theorem 1): Connected graph + TTL → guaranteed delivery

## Topology: grid (N=100)
DRFE-R Simulator
================

Configuration:
  Nodes:     100
  Topology:  grid
  Tests:     100
  Max TTL:   200
  Seed:      12345
  Optimize:  no

Generating network... done (0 ms)
  Nodes: 100
  Edges: 180

Running routing simulation... done

=== Simulation Results ===
Total tests:          100
Successful:           100
Failed:               0
Success rate:         100.00%
Average hops:         29.99
Gravity hops:         530
Tree hops:            2466
Pressure hops:        3
TTL failures:         0
No path failures:     0
Elapsed time:         0 ms

✓ VERIFIED: 100% routing success rate achieved
  Theory prediction (Theorem 1): Connected graph + TTL → guaranteed delivery

## Topology: line (N=100)
DRFE-R Simulator
================

Configuration:
  Nodes:     100
  Topology:  line
  Tests:     100
  Max TTL:   300
  Seed:      12345
  Optimize:  no

Generating network... done (0 ms)
  Nodes: 100
  Edges: 99

Running routing simulation... done

=== Simulation Results ===
Total tests:          100
Successful:           100
Failed:               0
Success rate:         100.00%
Average hops:         57.44
Gravity hops:         392
Tree hops:            5352
Pressure hops:        0
TTL failures:         0
No path failures:     0
Elapsed time:         1 ms

✓ VERIFIED: 100% routing success rate achieved
  Theory prediction (Theorem 1): Connected graph + TTL → guaranteed delivery

## Topology: lollipop (N=100)
DRFE-R Simulator
================

Configuration:
  Nodes:     100
  Topology:  lollipop
  Tests:     100
  Max TTL:   300
  Seed:      12345
  Optimize:  no

Generating network... done (0 ms)
  Nodes: 100
  Edges: 595

Running routing simulation... done

=== Simulation Results ===
Total tests:          100
Successful:           100
Failed:               0
Success rate:         100.00%
Average hops:         51.35
Gravity hops:         229
Tree hops:            4906
Pressure hops:        0
TTL failures:         0
No path failures:     0
Elapsed time:         1 ms

✓ VERIFIED: 100% routing success rate achieved
  Theory prediction (Theorem 1): Connected graph + TTL → guaranteed delivery

## Topology: ba (N=300)
DRFE-R Simulator
================

Configuration:
  Nodes:     300
  Topology:  ba
  Tests:     100
  Max TTL:   600
  Seed:      12345
  Optimize:  no

Generating network... done (1 ms)
  Nodes: 300
  Edges: 894

Running routing simulation... done

=== Simulation Results ===
Total tests:          100
Successful:           100
Failed:               0
Success rate:         100.00%
Average hops:         123.04
Gravity hops:         271
Tree hops:            12033
Pressure hops:        0
TTL failures:         0
No path failures:     0
Elapsed time:         2 ms

✓ VERIFIED: 100% routing success rate achieved
  Theory prediction (Theorem 1): Connected graph + TTL → guaranteed delivery

## Topology: ws (N=300)
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
Successful:           99
Failed:               1
Success rate:         99.00%
Average hops:         67.80
Gravity hops:         563
Tree hops:            6149
Pressure hops:        0
TTL failures:         1
No path failures:     0
Elapsed time:         1 ms

○ MOSTLY VERIFIED: 99.0% success rate
  Minor failures may be due to disconnected components or TTL exhaustion

## Topology: grid (N=300)
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
Average hops:         107.10
Gravity hops:         689
Tree hops:            10021
Pressure hops:        0
TTL failures:         0
No path failures:     0
Elapsed time:         2 ms

✓ VERIFIED: 100% routing success rate achieved
  Theory prediction (Theorem 1): Connected graph + TTL → guaranteed delivery

## Topology: line (N=300)
DRFE-R Simulator
================

Configuration:
  Nodes:     300
  Topology:  line
  Tests:     100
  Max TTL:   700
  Seed:      12345
  Optimize:  no

Generating network... done (0 ms)
  Nodes: 300
  Edges: 299

Running routing simulation... done

=== Simulation Results ===
Total tests:          100
Successful:           100
Failed:               0
Success rate:         100.00%
Average hops:         164.43
Gravity hops:         405
Tree hops:            16038
Pressure hops:        0
TTL failures:         0
No path failures:     0
Elapsed time:         3 ms

✓ VERIFIED: 100% routing success rate achieved
  Theory prediction (Theorem 1): Connected graph + TTL → guaranteed delivery

