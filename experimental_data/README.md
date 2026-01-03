# DRFE-R Experimental Data

This directory contains all organized experimental data for the DRFE-R project, exported in CSV and JSON formats for easy analysis and paper preparation.

## Directory Contents

### CSV Files (for spreadsheet analysis)

- **scalability_results.csv**: Scalability experiment data (6 network sizes, 1000 tests each)
- **topology_results_all.csv**: Topology experiment data (5 topologies × 3 sizes, 1000 tests each)
- **baseline_comparison.csv**: Baseline protocol comparison (DRFE-R vs Chord vs Kademlia)

### JSON Summaries (for programmatic access)

- **scalability_summary.json**: Summary statistics for scalability experiments
- **topology_summary.json**: Summary statistics grouped by topology type
- **baseline_summary.json**: Summary statistics grouped by protocol
- **master_summary.json**: Combined summary of all experiments
- **data_index.json**: Index of all data files with metadata

### Documentation

- **experimental_setup.md**: Complete documentation of experimental setup, parameters, and configurations
- **README.md**: This file

## Quick Start

### View CSV Data

Open any CSV file in Excel, Google Sheets, or your preferred spreadsheet application:

```bash
# Example: Open scalability results
excel experimental_data/scalability_results.csv
```

### Load JSON Data (Python)

```python
import json

# Load master summary
with open('experimental_data/master_summary.json', 'r') as f:
    data = json.load(f)

# Access scalability results
scalability = data['experiments']['scalability']
print(f"Network sizes tested: {scalability['network_sizes']}")
```

### Load JSON Data (JavaScript/Node.js)

```javascript
const fs = require('fs');

// Load master summary
const data = JSON.parse(fs.readFileSync('experimental_data/master_summary.json', 'utf8'));

// Access topology results
const topology = data.experiments.topology;
console.log('Topologies tested:', Object.keys(topology));
```

## Data Organization

### Scalability Experiments

**Purpose**: Evaluate how DRFE-R scales with network size

**Key Metrics**:
- Success rate (%)
- Average hop count
- Stretch ratio
- Routing time (μs)
- Memory usage (MB)

**Network Sizes**: 100, 300, 500, 1000, 3000, 5000 nodes

### Topology Experiments

**Purpose**: Evaluate DRFE-R across different network structures

**Topologies Tested**:
- Barabási-Albert (scale-free networks)
- Watts-Strogatz (small-world networks)
- Grid (geometric networks)
- Random (Erdős-Rényi)
- Real-World (community-structured)

**Network Sizes**: 100, 200, 300 nodes

### Baseline Comparison

**Purpose**: Compare DRFE-R with established routing protocols

**Protocols**:
- DRFE-R (our protocol)
- Chord DHT
- Kademlia DHT

**Network Sizes**: 50, 100, 200, 300 nodes
**Topologies**: BA, Random, Grid

## Requirements Validation

This experimental data validates the following requirements from the DRFE-R specification:

- ✅ **Requirement 10.1**: Scalability data for networks of 100-5000 nodes
- ✅ **Requirement 10.2**: Data for 5+ topology types
- ✅ **Requirement 10.3**: 1000+ routing tests per configuration
- ✅ **Requirement 10.4**: Stretch ratio measurements
- ✅ **Requirement 10.5**: Coordinate stability data
- ✅ **Requirement 6.5**: Baseline protocol comparisons
- ✅ **Requirement 16.1-16.5**: Scalability verification

## Reproducibility

All experiments can be reproduced using the commands documented in `experimental_setup.md`.

Random seed: 42 (used for all experiments)

## Data Generation

This data was generated using:

```bash
python3 organize_experimental_data.py
```

The script collects all experimental results from the project root and exports them to this directory.

## For Paper Authors

### Recommended Figures

1. **Scalability Plot**: Success rate vs. network size (line plot)
2. **Hop Count Scaling**: Average hops vs. network size (line plot with log scale)
3. **Topology Comparison**: Success rate across topologies (grouped bar chart)
4. **Baseline Comparison**: Hop count comparison (grouped bar chart)
5. **Stretch Ratio**: Stretch ratio vs. network size (line plot)

### LaTeX Table Generation

Use the analysis scripts in the project root to generate LaTeX tables:

```bash
python3 analyze_scalability.py
python3 analyze_topology_experiments.py
python3 analyze_baseline_comparison.py
```

### Key Statistics

From `master_summary.json`:

- **Total experiments**: 3 (Scalability, Topology, Baseline)
- **Total configurations**: 57
- **Total routing tests**: 21,000+
- **Network sizes**: 50-5000 nodes
- **Topologies**: 5 types
- **Protocols compared**: 3

## File Sizes

See `data_index.json` for file sizes and metadata.

## Contact

For questions about the experimental data, please refer to the main project documentation or contact the maintainers.

---

Generated: 2026-01-02
