#!/bin/bash
# Run comprehensive topology experiments across different network sizes

echo "DRFE-R Comprehensive Topology Experiments"
echo "=========================================="
echo ""

# Build the binary first
echo "Building topology_experiments binary..."
wsl -d ubuntu bash -c "source ~/.cargo/env && cargo build --release --bin topology_experiments"

if [ $? -ne 0 ]; then
    echo "Build failed!"
    exit 1
fi

echo "Build successful!"
echo ""

# Run experiments for different network sizes
SIZES=(100 200 300 500)
TESTS=1000

for SIZE in "${SIZES[@]}"; do
    echo "========================================"
    echo "Running experiments with $SIZE nodes"
    echo "========================================"
    echo ""
    
    OUTPUT_FILE="topology_experiments_n${SIZE}.json"
    
    wsl -d ubuntu bash -c "source ~/.cargo/env && cargo run --release --bin topology_experiments -- --nodes $SIZE --tests $TESTS --output $OUTPUT_FILE"
    
    if [ $? -eq 0 ]; then
        echo ""
        echo "✓ Completed experiments for $SIZE nodes"
        echo "  Results saved to: $OUTPUT_FILE"
        echo ""
    else
        echo ""
        echo "✗ Failed experiments for $SIZE nodes"
        echo ""
    fi
done

echo "========================================"
echo "All experiments completed!"
echo "========================================"
echo ""
echo "Result files:"
ls -lh topology_experiments_*.json
