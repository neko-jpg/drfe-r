#!/usr/bin/env python3
"""
Analyze baseline comparison results and generate summary report.
"""

import json
import sys
from collections import defaultdict

def load_results(filename):
    """Load results from JSON file."""
    with open(filename, 'r') as f:
        data = json.load(f)
    return data['results']

def analyze_by_protocol(results):
    """Analyze results grouped by protocol."""
    by_protocol = defaultdict(list)
    
    for result in results:
        by_protocol[result['protocol']].append(result)
    
    print("=" * 80)
    print("ANALYSIS BY PROTOCOL")
    print("=" * 80)
    print()
    
    for protocol in sorted(by_protocol.keys()):
        protocol_results = by_protocol[protocol]
        
        avg_success = sum(r['success_rate'] for r in protocol_results) / len(protocol_results)
        avg_hops = sum(r['avg_hops'] for r in protocol_results) / len(protocol_results)
        avg_latency = sum(r['avg_latency_us'] for r in protocol_results) / len(protocol_results)
        
        print(f"{protocol}:")
        print(f"  Average Success Rate: {avg_success * 100:.2f}%")
        print(f"  Average Hop Count: {avg_hops:.2f}")
        print(f"  Average Latency: {avg_latency:.2f} μs")
        print()

def analyze_by_topology(results):
    """Analyze results grouped by topology."""
    by_topology = defaultdict(lambda: defaultdict(list))
    
    for result in results:
        by_topology[result['topology']][result['protocol']].append(result)
    
    print("=" * 80)
    print("ANALYSIS BY TOPOLOGY")
    print("=" * 80)
    print()
    
    for topology in sorted(by_topology.keys()):
        print(f"{topology.upper()} Topology:")
        print()
        
        for protocol in sorted(by_topology[topology].keys()):
            protocol_results = by_topology[topology][protocol]
            
            avg_success = sum(r['success_rate'] for r in protocol_results) / len(protocol_results)
            avg_hops = sum(r['avg_hops'] for r in protocol_results) / len(protocol_results)
            avg_latency = sum(r['avg_latency_us'] for r in protocol_results) / len(protocol_results)
            
            print(f"  {protocol}:")
            print(f"    Success Rate: {avg_success * 100:.2f}%")
            print(f"    Avg Hops: {avg_hops:.2f}")
            print(f"    Avg Latency: {avg_latency:.2f} μs")
        print()

def analyze_scalability(results):
    """Analyze scalability trends."""
    by_size = defaultdict(lambda: defaultdict(list))
    
    for result in results:
        by_size[result['network_size']][result['protocol']].append(result)
    
    print("=" * 80)
    print("SCALABILITY ANALYSIS")
    print("=" * 80)
    print()
    
    print(f"{'Size':<10} {'Protocol':<12} {'Success %':<12} {'Avg Hops':<12} {'Avg Latency(μs)':<15}")
    print("-" * 80)
    
    for size in sorted(by_size.keys()):
        for protocol in sorted(by_size[size].keys()):
            protocol_results = by_size[size][protocol]
            
            avg_success = sum(r['success_rate'] for r in protocol_results) / len(protocol_results)
            avg_hops = sum(r['avg_hops'] for r in protocol_results) / len(protocol_results)
            avg_latency = sum(r['avg_latency_us'] for r in protocol_results) / len(protocol_results)
            
            print(f"{size:<10} {protocol:<12} {avg_success * 100:<12.2f} {avg_hops:<12.2f} {avg_latency:<15.2f}")
    print()

def generate_comparison_summary(results):
    """Generate a comparison summary."""
    print("=" * 80)
    print("COMPARISON SUMMARY")
    print("=" * 80)
    print()
    
    # Group by protocol
    by_protocol = defaultdict(list)
    for result in results:
        by_protocol[result['protocol']].append(result)
    
    # Calculate overall metrics
    print("Overall Performance (averaged across all tests):")
    print()
    print(f"{'Protocol':<12} {'Success %':<12} {'Avg Hops':<12} {'Avg Latency(μs)':<15}")
    print("-" * 60)
    
    for protocol in sorted(by_protocol.keys()):
        protocol_results = by_protocol[protocol]
        
        avg_success = sum(r['success_rate'] for r in protocol_results) / len(protocol_results)
        avg_hops = sum(r['avg_hops'] for r in protocol_results) / len(protocol_results)
        avg_latency = sum(r['avg_latency_us'] for r in protocol_results) / len(protocol_results)
        
        print(f"{protocol:<12} {avg_success * 100:<12.2f} {avg_hops:<12.2f} {avg_latency:<15.2f}")
    
    print()
    print("Key Findings:")
    print()
    
    # Find best in each category
    all_by_protocol = {}
    for protocol in by_protocol.keys():
        protocol_results = by_protocol[protocol]
        all_by_protocol[protocol] = {
            'success': sum(r['success_rate'] for r in protocol_results) / len(protocol_results),
            'hops': sum(r['avg_hops'] for r in protocol_results) / len(protocol_results),
            'latency': sum(r['avg_latency_us'] for r in protocol_results) / len(protocol_results)
        }
    
    best_success = max(all_by_protocol.items(), key=lambda x: x[1]['success'])
    best_hops = min(all_by_protocol.items(), key=lambda x: x[1]['hops'])
    best_latency = min(all_by_protocol.items(), key=lambda x: x[1]['latency'])
    
    print(f"1. Highest Success Rate: {best_success[0]} ({best_success[1]['success'] * 100:.2f}%)")
    print(f"2. Lowest Hop Count: {best_hops[0]} ({best_hops[1]['hops']:.2f} hops)")
    print(f"3. Lowest Latency: {best_latency[0]} ({best_latency[1]['latency']:.2f} μs)")
    print()
    
    # DRFE-R specific analysis
    if 'DRFE-R' in all_by_protocol:
        drfer = all_by_protocol['DRFE-R']
        print("DRFE-R Performance:")
        print(f"  - Success Rate: {drfer['success'] * 100:.2f}%")
        print(f"  - Average Hops: {drfer['hops']:.2f}")
        print(f"  - Average Latency: {drfer['latency']:.2f} μs")
        print()
        
        # Compare with others
        if 'Chord' in all_by_protocol:
            chord = all_by_protocol['Chord']
            hop_diff = ((drfer['hops'] - chord['hops']) / chord['hops']) * 100
            print(f"  vs Chord: {hop_diff:+.1f}% hops")
        
        if 'Kademlia' in all_by_protocol:
            kad = all_by_protocol['Kademlia']
            hop_diff = ((drfer['hops'] - kad['hops']) / kad['hops']) * 100
            print(f"  vs Kademlia: {hop_diff:+.1f}% hops")
    
    print()

def main():
    if len(sys.argv) > 1:
        filename = sys.argv[1]
    else:
        filename = 'baseline_comparison.json'
    
    try:
        results = load_results(filename)
    except FileNotFoundError:
        print(f"Error: File '{filename}' not found.")
        sys.exit(1)
    except json.JSONDecodeError:
        print(f"Error: Invalid JSON in '{filename}'.")
        sys.exit(1)
    
    print()
    analyze_by_protocol(results)
    analyze_by_topology(results)
    analyze_scalability(results)
    generate_comparison_summary(results)

if __name__ == '__main__':
    main()
