#!/usr/bin/env python3
"""
Generate LaTeX tables and analysis from ablation study results.
For DRFE-R paper publication.
"""

import csv
import json
from pathlib import Path

def load_csv_data(csv_path):
    """Load ablation summary CSV."""
    data = []
    with open(csv_path, 'r') as f:
        reader = csv.DictReader(f)
        for row in reader:
            data.append({
                'topology': row['topology'],
                'n': int(row['n']),
                'embedding': row['embedding'],
                'success_rate': float(row['success_rate']),
                'avg_hops': float(row['avg_hops']),
                'stretch': float(row['stretch']),
                'gravity_ratio': float(row['gravity_ratio']),
                'pressure_ratio': float(row['pressure_ratio']),
                'tree_ratio': float(row['tree_ratio']),
            })
    return data

def generate_latex_success_table(data):
    """Generate LaTeX table for success rates by topology and embedding."""
    
    # Group by topology and n
    topologies = ['ba', 'ws', 'grid', 'line', 'lollipop']
    embeddings = ['PIE', 'Random', 'Ricci-Broken', 'Ricci-Fixed']
    scales = [50, 100, 200, 300]
    
    latex = r"""\begin{table}[htbp]
\centering
\caption{Routing Success Rate by Topology and Embedding Strategy}
\label{tab:success-rate}
\begin{tabular}{llcccc}
\toprule
Topology & N & PIE & Random & Ricci-Broken & Ricci-Fixed \\
\midrule
"""
    
    for topo in topologies:
        for n in scales:
            row_data = [d for d in data if d['topology'] == topo and d['n'] == n]
            if not row_data:
                continue
            
            topo_label = topo.upper() if topo in ['ba', 'ws'] else topo.capitalize()
            line = f"{topo_label} & {n}"
            
            for emb in embeddings:
                match = [d for d in row_data if d['embedding'] == emb]
                if match:
                    sr = match[0]['success_rate']
                    # Bold if 100%
                    if sr >= 0.999:
                        line += f" & \\textbf{{{sr*100:.1f}\\%}}"
                    else:
                        line += f" & {sr*100:.1f}\\%"
                else:
                    line += " & --"
            
            line += r" \\"
            if n == 300:
                line += r"\midrule"
            latex += line + "\n"
    
    latex += r"""\bottomrule
\end{tabular}
\end{table}
"""
    return latex

def generate_latex_hops_table(data):
    """Generate LaTeX table for average hops."""
    
    topologies = ['ba', 'ws', 'grid']  # Only main topologies for this table
    embeddings = ['PIE', 'Random', 'Ricci-Broken', 'Ricci-Fixed']
    scales = [100, 200, 300]
    
    latex = r"""\begin{table}[htbp]
\centering
\caption{Average Hop Count by Topology and Embedding Strategy}
\label{tab:avg-hops}
\begin{tabular}{llcccc}
\toprule
Topology & N & PIE & Random & Ricci-Broken & Ricci-Fixed \\
\midrule
"""
    
    for topo in topologies:
        for n in scales:
            row_data = [d for d in data if d['topology'] == topo and d['n'] == n]
            if not row_data:
                continue
            
            topo_label = topo.upper() if topo in ['ba', 'ws'] else topo.capitalize()
            line = f"{topo_label} & {n}"
            
            # Find minimum hops for this row
            hops_values = [d['avg_hops'] for d in row_data if d['success_rate'] > 0.8]
            min_hops = min(hops_values) if hops_values else 0
            
            for emb in embeddings:
                match = [d for d in row_data if d['embedding'] == emb]
                if match:
                    hops = match[0]['avg_hops']
                    if abs(hops - min_hops) < 0.01:
                        line += f" & \\textbf{{{hops:.1f}}}"
                    else:
                        line += f" & {hops:.1f}"
                else:
                    line += " & --"
            
            line += r" \\"
            if n == 300 and topo != 'grid':
                line += r"\midrule"
            latex += line + "\n"
    
    latex += r"""\bottomrule
\end{tabular}
\end{table}
"""
    return latex

def generate_hop_breakdown_table(data):
    """Generate table showing Gravity/Pressure/Tree breakdown."""
    
    topologies = ['ba', 'ws', 'grid']
    scale = 300  # Focus on largest scale
    
    latex = r"""\begin{table}[htbp]
\centering
\caption{Routing Mode Distribution (N=300)}
\label{tab:mode-distribution}
\begin{tabular}{llccc}
\toprule
Topology & Embedding & Gravity \% & Pressure \% & Tree \% \\
\midrule
"""
    
    for topo in topologies:
        row_data = [d for d in data if d['topology'] == topo and d['n'] == scale]
        if not row_data:
            continue
            
        topo_label = topo.upper() if topo in ['ba', 'ws'] else topo.capitalize()
        first = True
        
        for d in sorted(row_data, key=lambda x: x['embedding']):
            if first:
                line = f"{topo_label} & {d['embedding']}"
                first = False
            else:
                line = f" & {d['embedding']}"
            
            gravity = d['gravity_ratio'] * 100
            pressure = d['pressure_ratio'] * 100
            tree = d['tree_ratio'] * 100
            
            line += f" & {gravity:.1f}\\% & {pressure:.1f}\\% & {tree:.1f}\\% \\\\"
            latex += line + "\n"
        
        latex += r"\midrule" + "\n"
    
    latex += r"""\bottomrule
\end{tabular}
\end{table}
"""
    return latex

def generate_summary_analysis(data):
    """Generate summary statistics for paper."""
    
    summary = {
        'PIE': {'success': [], 'hops': [], 'gravity': []},
        'Random': {'success': [], 'hops': [], 'gravity': []},
        'Ricci-Broken': {'success': [], 'hops': [], 'gravity': []},
        'Ricci-Fixed': {'success': [], 'hops': [], 'gravity': []},
    }
    
    # Focus on realistic topologies
    realistic = ['ba', 'ws', 'grid']
    
    for d in data:
        if d['topology'] in realistic and d['n'] >= 100:
            emb = d['embedding']
            if emb in summary:
                summary[emb]['success'].append(d['success_rate'])
                summary[emb]['hops'].append(d['avg_hops'])
                summary[emb]['gravity'].append(d['gravity_ratio'])
    
    analysis = "## Summary Analysis for Paper\n\n"
    analysis += "| Embedding | Avg Success | Avg Hops | Avg Gravity % |\n"
    analysis += "|-----------|------------|----------|---------------|\n"
    
    for emb in ['PIE', 'Random', 'Ricci-Broken', 'Ricci-Fixed']:
        s = summary[emb]
        avg_success = sum(s['success']) / len(s['success']) if s['success'] else 0
        avg_hops = sum(s['hops']) / len(s['hops']) if s['hops'] else 0
        avg_gravity = sum(s['gravity']) / len(s['gravity']) if s['gravity'] else 0
        
        analysis += f"| {emb} | {avg_success*100:.1f}% | {avg_hops:.1f} | {avg_gravity*100:.1f}% |\n"
    
    return analysis

def main():
    base_path = Path("paper_data")
    csv_path = base_path / "ablation" / "ablation_summary.csv"
    
    if not csv_path.exists():
        print(f"Error: {csv_path} not found")
        return
    
    data = load_csv_data(csv_path)
    print(f"Loaded {len(data)} experiment results")
    
    # Generate LaTeX tables
    tables_dir = base_path / "tables"
    tables_dir.mkdir(exist_ok=True)
    
    success_table = generate_latex_success_table(data)
    with open(tables_dir / "success_rate_table.tex", 'w') as f:
        f.write(success_table)
    print("✓ Generated success_rate_table.tex")
    
    hops_table = generate_latex_hops_table(data)
    with open(tables_dir / "avg_hops_table.tex", 'w') as f:
        f.write(hops_table)
    print("✓ Generated avg_hops_table.tex")
    
    mode_table = generate_hop_breakdown_table(data)
    with open(tables_dir / "mode_distribution_table.tex", 'w') as f:
        f.write(mode_table)
    print("✓ Generated mode_distribution_table.tex")
    
    # Generate hop analysis
    hop_dir = base_path / "hop_analysis"
    hop_dir.mkdir(exist_ok=True)
    
    summary = generate_summary_analysis(data)
    with open(hop_dir / "summary_analysis.md", 'w') as f:
        f.write(summary)
    print("✓ Generated summary_analysis.md")
    
    # Generate CSV for hop ratios
    with open(hop_dir / "hop_ratios.csv", 'w', newline='') as f:
        writer = csv.writer(f)
        writer.writerow(['topology', 'n', 'embedding', 'gravity_pct', 'pressure_pct', 'tree_pct'])
        for d in data:
            writer.writerow([
                d['topology'], d['n'], d['embedding'],
                f"{d['gravity_ratio']*100:.1f}",
                f"{d['pressure_ratio']*100:.1f}",
                f"{d['tree_ratio']*100:.1f}"
            ])
    print("✓ Generated hop_ratios.csv")
    
    print("\nAll paper materials generated successfully!")

if __name__ == "__main__":
    main()
