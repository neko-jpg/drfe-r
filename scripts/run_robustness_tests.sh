#!/bin/bash
# Comprehensive Robustness Testing Script for DRFE-R
# Tests all topology types with various configurations

source ~/.cargo/env
cd '/mnt/c/dev/network test'

OUTPUT_FILE="robustness_final.md"

echo "# DRFE-R Comprehensive Robustness Test Results" > "$OUTPUT_FILE"
echo "Date: $(date)" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"
echo "This document contains comprehensive robustness testing results for all supported topology types." >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

# Test configurations: topology, nodes, tests, ttl
declare -a configs=(
    "ba:100:500:200"
    "ba:300:500:600"
    "ba:500:500:1000"
    "ws:100:500:200"
    "ws:300:500:600"
    "ws:500:500:1000"
    "grid:100:500:200"
    "grid:300:500:600"
    "grid:500:500:1000"
    "line:100:500:300"
    "line:300:500:700"
    "lollipop:100:500:300"
    "lollipop:300:500:700"
)

echo "Starting comprehensive robustness tests..."
echo ""

for config in "${configs[@]}"; do
    IFS=':' read -r topology nodes tests ttl <<< "$config"
    
    echo "Testing: $topology topology with N=$nodes, tests=$tests, TTL=$ttl"
    
    echo "## Topology: $topology (N=$nodes, Tests=$tests, TTL=$ttl)" >> "$OUTPUT_FILE"
    echo '```' >> "$OUTPUT_FILE"
    
    ./target/release/simulator \
        --topology "$topology" \
        --nodes "$nodes" \
        --tests "$tests" \
        --ttl "$ttl" \
        --seed 12345 \
        >> "$OUTPUT_FILE" 2>&1
    
    echo '```' >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
done

echo ""
echo "All tests completed!"
echo "Results saved to: $OUTPUT_FILE"
echo ""
echo "Summary:"
grep -E "(Success rate:|✓ VERIFIED|○ MOSTLY|✗ VERIFICATION)" "$OUTPUT_FILE" | head -20
