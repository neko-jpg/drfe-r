#!/bin/bash
# Re-test problematic topologies with higher TTL

source ~/.cargo/env
cd '/mnt/c/dev/network test'

OUTPUT_FILE="robustness_final.md"

echo "" >> "$OUTPUT_FILE"
echo "---" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"
echo "## Re-test with Increased TTL for Pathological Topologies" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"
echo "Line and Lollipop topologies have high diameter and require higher TTL for 100% success." >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

# Re-test configurations with higher TTL
declare -a configs=(
    "line:100:500:500"
    "line:300:500:1200"
    "lollipop:100:500:500"
    "lollipop:300:500:1200"
)

echo "Re-testing problematic topologies with higher TTL..."
echo ""

for config in "${configs[@]}"; do
    IFS=':' read -r topology nodes tests ttl <<< "$config"
    
    echo "Re-testing: $topology topology with N=$nodes, tests=$tests, TTL=$ttl"
    
    echo "## Topology: $topology (N=$nodes, Tests=$tests, TTL=$ttl) - RETEST" >> "$OUTPUT_FILE"
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
echo "Re-tests completed!"
echo ""
echo "Summary of re-tests:"
tail -100 "$OUTPUT_FILE" | grep -E "(Success rate:|✓ VERIFIED|○ MOSTLY|✗ VERIFICATION)"
