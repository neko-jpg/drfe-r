# DRFE-R Paper Data Directory

This directory contains all experimental data and generated materials for the DRFE-R paper.

## Directory Structure

```
paper_data/
├── ablation/          # Ablation study results (JSON/CSV)
├── hop_analysis/      # Hop count breakdown data
├── visualization/     # Embedding coordinates for visualization
├── figures/           # Publication-quality figures
└── tables/            # LaTeX tables
```

## Data Files

### Ablation Study (`ablation/`)
- `ablation_results.json` - Complete experiment results
- `ablation_summary.csv` - Aggregated statistics

### Hop Analysis (`hop_analysis/`)
- `hop_breakdown.json` - Per-test mode breakdown
- `hop_ratios.csv` - Gravity/Pressure/Tree percentages

### Visualization (`visualization/`)
- `coordinates_pie.json` - PIE embedding coordinates
- `coordinates_ricci.json` - Ricci Flow optimized coordinates
- `coordinates_random.json` - Random embedding coordinates

## Generated: 2026-01-03
