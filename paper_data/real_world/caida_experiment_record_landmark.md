# CAIDA AS-REL2 Real-World Topology Experiments (Landmark Routing)

Inputs
- Source archive: paper_data/input/caida/20251201.as-rel2.txt.bz2
- Decompressed: paper_data/input/caida/20251201.as-rel2.txt
- Edge list: paper_data/input/caida/caida_edge_list.txt (columns: AS1 AS2)

Command (WSL)
- wsl -d ubuntu bash -lc "cd '/mnt/c/dev/network test' && source ~/.cargo/env && cargo run --release --bin topology_experiments -- --nodes 100 --tests 1000 --seeds 42,43,44,45,46,47,48,49,50,51 --routing landmark --edge-list 'paper_data/input/caida/caida_edge_list.txt' --output 'paper_data/real_world/caida_topology_experiments_landmark.json' --summary-output 'paper_data/real_world/caida_topology_experiments_landmark_summary.json' | tee 'paper_data/real_world/caida_topology_experiments_landmark.log'"

Routing configuration
- Algorithm: landmark-guided greedy + bounded lookahead (depth 3)
- Landmarks: auto (2*sqrt(n), capped at 64)
- Lookahead max nodes: 5000
- Landmark weight: 1.0
- Hyperbolic weight: 0.15

Run configuration
- Nodes per synthetic topology: 100
- Tests per topology: 1000
- Seeds: 42,43,44,45,46,47,48,49,50,51
- Real-world graph size (from run): 78771 nodes, 723215 edges

Summary (mean, 95% CI from caida_topology_experiments_landmark_summary.json)
- BarabasiAlbert: success_rate=0.9941 +/- 0.0052, avg_hops=4.0618 +/- 0.2990, stretch=1.5841 +/- 0.1113
- WattsStrogatz:  success_rate=1.0000 +/- 0.0000, avg_hops=4.8736 +/- 0.2337, stretch=1.3387 +/- 0.0412
- Grid:           success_rate=1.0000 +/- 0.0000, avg_hops=6.7092 +/- 0.0766, stretch=1.0016 +/- 0.0011
- Random:         success_rate=0.9931 +/- 0.0110, avg_hops=4.8487 +/- 0.4497, stretch=1.5923 +/- 0.1536
- RealWorld:      success_rate=0.3545 +/- 0.0104, avg_hops=9.1193 +/- 0.7058, stretch=2.8815 +/- 0.2060

Notes
- Real-world success rate dropped vs baseline (0.4145 -> 0.3545). Further tuning or alternative routing needed.
- Build warnings reported unused imports; no runtime errors observed.
