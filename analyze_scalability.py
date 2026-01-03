#!/usr/bin/env python3
"""
Analyze and visualize DRFE-R scalability experiment results.

This script reads the scalability_results.json file and generates
analysis and visualizations for the research paper.
"""

import json
import sys

def load_results(filename='scalability_results.json'):
    """Load experiment results from JSON file."""
    try:
        with open(filename, 'r') as f:
            return json.load(f)
    except FileNotFoundError:
        print(f"Error: {filename} not found")
        sys.exit(1)
    except json.JSONDecodeError:
        print(f"Error: Invalid JSON in {filename}")
        sys.exit(1)

def print_summary_table(results):
    """Print a formatted summary table."""
    print("\n=== Scalability Summary Table ===\n")
    print(f"{'Size':<8} {'Success%':<10} {'Avg Hops':<10} {'Stretch':<10} {'Time(μs)':<12} {'Memory(MB)':<12}")
    print("-" * 72)
    
    for r in results:
        print(f"{r['network_size']:<8} "
              f"{r['success_rate']*100:<10.2f} "
              f"{r['avg_hops']:<10.2f} "
              f"{r['avg_stretch']:<10.3f} "
              f"{r['avg_routing_time_us']:<12.2f} "
              f"{r['total_memory_mb']:<12.2f}")

def print_complexity_analysis(results):
    """Print complexity analysis."""
    print("\n=== Complexity Analysis ===\n")
    
    print("Routing Complexity (O(k) per hop):")
    print(f"{'Size':<8} {'Avg Hops':<12} {'Avg Degree':<12} {'Ratio':<10}")
    print("-" * 42)
    for r in results:
        ratio = r['avg_hops'] / r['avg_degree'] if r['avg_degree'] > 0 else 0
        print(f"{r['network_size']:<8} {r['avg_hops']:<12.2f} {r['avg_degree']:<12.2f} {ratio:<10.2f}")
    
    print("\nMemory Complexity (O(k) per node):")
    print(f"{'Size':<8} {'Mem/Node':<12} {'Avg Degree':<12} {'Bytes/Neighbor':<15}")
    print("-" * 47)
    for r in results:
        bytes_per_neighbor = r['memory_per_node_bytes'] / r['avg_degree'] if r['avg_degree'] > 0 else 0
        print(f"{r['network_size']:<8} {r['memory_per_node_bytes']:<12} "
              f"{r['avg_degree']:<12.2f} {bytes_per_neighbor:<15.1f}")
    
    print("\nEmbedding Complexity (O(|E|)):")
    print(f"{'Size':<8} {'Time(ms)':<12} {'Edges':<10} {'ms/edge':<12}")
    print("-" * 42)
    for r in results:
        print(f"{r['network_size']:<8} {r['embedding_time_ms']:<12} "
              f"{r['num_edges']:<10} {r['embedding_complexity_per_edge']:<12.6f}")

def print_mode_distribution(results):
    """Print routing mode distribution."""
    print("\n=== Routing Mode Distribution ===\n")
    print(f"{'Size':<8} {'Gravity%':<12} {'Pressure%':<12} {'Tree%':<12}")
    print("-" * 44)
    
    for r in results:
        total_hops = r['gravity_hops'] + r['pressure_hops'] + r['tree_hops']
        if total_hops > 0:
            gravity_pct = (r['gravity_hops'] / total_hops) * 100
            pressure_pct = (r['pressure_hops'] / total_hops) * 100
            tree_pct = (r['tree_hops'] / total_hops) * 100
        else:
            gravity_pct = pressure_pct = tree_pct = 0
        
        print(f"{r['network_size']:<8} {gravity_pct:<12.1f} {pressure_pct:<12.1f} {tree_pct:<12.1f}")

def print_hop_statistics(results):
    """Print hop count statistics."""
    print("\n=== Hop Count Statistics ===\n")
    print(f"{'Size':<8} {'Avg':<10} {'Median':<10} {'P95':<10} {'Max':<10}")
    print("-" * 48)
    
    for r in results:
        print(f"{r['network_size']:<8} "
              f"{r['avg_hops']:<10.2f} "
              f"{r['median_hops']:<10} "
              f"{r['p95_hops']:<10} "
              f"{r['max_hops']:<10}")

def generate_latex_table(results):
    """Generate LaTeX table for paper."""
    print("\n=== LaTeX Table for Paper ===\n")
    print("\\begin{table}[h]")
    print("\\centering")
    print("\\caption{Scalability Experiment Results}")
    print("\\label{tab:scalability}")
    print("\\begin{tabular}{rrrrrr}")
    print("\\hline")
    print("Size & Success & Avg Hops & Stretch & Time ($\\mu$s) & Memory (MB) \\\\")
    print("\\hline")
    
    for r in results:
        print(f"{r['network_size']} & "
              f"{r['success_rate']*100:.1f}\\% & "
              f"{r['avg_hops']:.2f} & "
              f"{r['avg_stretch']:.2f} & "
              f"{r['avg_routing_time_us']:.1f} & "
              f"{r['total_memory_mb']:.2f} \\\\")
    
    print("\\hline")
    print("\\end{tabular}")
    print("\\end{table}")

def main():
    """Main analysis function."""
    print("DRFE-R Scalability Analysis")
    print("=" * 50)
    
    # Load results
    data = load_results()
    results = data['results']
    
    print(f"\nExperiment Timestamp: {data['timestamp']}")
    print(f"Network Sizes: {data['config']['network_sizes']}")
    print(f"Tests per Size: {data['config']['num_routing_tests']}")
    print(f"Max TTL: {data['config']['max_ttl']}")
    print(f"Random Seed: {data['config']['seed']}")
    
    # Print various analyses
    print_summary_table(results)
    print_complexity_analysis(results)
    print_mode_distribution(results)
    print_hop_statistics(results)
    generate_latex_table(results)
    
    # Print key insights
    print("\n=== Key Insights ===\n")
    
    # Success rate trend
    success_rates = [r['success_rate'] for r in results]
    print(f"Success Rate Range: {min(success_rates)*100:.1f}% - {max(success_rates)*100:.1f}%")
    
    # Stretch ratio trend
    stretches = [r['avg_stretch'] for r in results]
    print(f"Stretch Ratio Range: {min(stretches):.2f}x - {max(stretches):.2f}x")
    
    # Memory efficiency
    total_memory = sum(r['total_memory_mb'] for r in results)
    print(f"Total Memory Across All Experiments: {total_memory:.2f} MB")
    
    # Routing time scaling
    first_time = results[0]['avg_routing_time_us']
    last_time = results[-1]['avg_routing_time_us']
    time_ratio = last_time / first_time
    size_ratio = results[-1]['network_size'] / results[0]['network_size']
    print(f"Routing Time Scaling: {time_ratio:.2f}x for {size_ratio:.0f}x network size")
    print(f"  (Sub-linear scaling: {time_ratio/size_ratio:.2f})")
    
    print("\n✓ Analysis complete!")

if __name__ == '__main__':
    main()
