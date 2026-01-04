# CAIDA AS-REL2 Real-World Topology Experiments

Inputs
- Source archive: paper_data/input/caida/20251201.as-rel2.txt.bz2
- Decompressed: paper_data/input/caida/20251201.as-rel2.txt
- Edge list: paper_data/input/caida/caida_edge_list.txt (columns: AS1 AS2)

Command (WSL)
- wsl -d ubuntu bash -lc "cd '/mnt/c/dev/network test' && source ~/.cargo/env && cargo run --release --bin topology_experiments -- --nodes 100 --tests 1000 --seeds 42,43,44,45,46,47,48,49,50,51 --edge-list 'paper_data/input/caida/caida_edge_list.txt' --output 'paper_data/real_world/caida_topology_experiments.json' --summary-output 'paper_data/real_world/caida_topology_experiments_summary.json' | tee 'paper_data/real_world/caida_topology_experiments.log'"

Run configuration
- Nodes per synthetic topology: 100
- Tests per topology: 1000
- Seeds: 42,43,44,45,46,47,48,49,50,51
- Real-world graph size (from run): 78771 nodes, 723215 edges

Summary (mean, 95% CI from caida_topology_experiments_summary.json)
- BarabasiAlbert: success_rate=0.9786 +/- 0.0051, avg_hops=5.1457 +/- 0.2728, stretch=2.0166 +/- 0.0983
- WattsStrogatz:  success_rate=0.9989 +/- 0.0011, avg_hops=6.3114 +/- 0.6266, stretch=1.7385 +/- 0.1897
- Grid:           success_rate=0.9897 +/- 0.0100, avg_hops=12.1571 +/- 0.8774, stretch=1.8219 +/- 0.1343
- Random:         success_rate=0.9954 +/- 0.0089, avg_hops=5.1467 +/- 0.2295, stretch=1.6855 +/- 0.0669
- RealWorld:      success_rate=0.4145 +/- 0.0138, avg_hops=12.5652 +/- 0.4585, stretch=3.6764 +/- 0.1308

Notes
- Build warnings reported unused imports; no runtime errors observed.
