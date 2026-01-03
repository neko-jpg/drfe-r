#!/bin/bash
# run_optimization_tests.sh

set -e
echo "Building simulator..."
~/.cargo/bin/cargo build --release --bin simulator

SIM="./target/release/simulator"
OUT="optimization_results.md"
echo "# Optimization Results (Pressure Fallback + Ricci Flow)" > $OUT
echo "Date: $(date)" >> $OUT
echo "" >> $OUT

# 1. Baseline: Pressure Fallback Enabled, No Ricci
# Testing on Watts-Strogatz (N=300) which had 1 failure in Step 2 result
# and Grid (N=300) which had pure Tree fallback overhead.
echo "## Baseline: Pressure Fallback (No Ricci)" >> $OUT
echo "Running WS N=300..."
$SIM -n 300 -t ws --tests 100 --ttl 600 --seed 12345 >> $OUT
echo "Running Grid N=300..."
$SIM -n 300 -t grid --tests 100 --ttl 600 --seed 12345 >> $OUT

# 2. Optimized: Pressure Fallback + Ricci Flow
echo "## Optimized: Pressure Fallback + Ricci Flow" >> $OUT
echo "Running WS N=300 (Ricci)..."
$SIM -n 300 -t ws --tests 100 --ttl 600 --seed 12345 --optimize --ricci-iter 30 >> $OUT
echo "Running Grid N=300 (Ricci)..."
$SIM -n 300 -t grid --tests 100 --ttl 600 --seed 12345 --optimize --ricci-iter 30 >> $OUT

echo "Done! Results saved to $OUT"
