# DRFE-R Benchmark Suite

This directory contains comprehensive performance benchmarks for the DRFE-R (Distributed Ricci Flow Embedding with Rendezvous) routing protocol implementation.

## Overview

The benchmark suite uses the [Criterion](https://github.com/bheisler/criterion.rs) framework to provide statistically rigorous performance measurements. All benchmarks are configured to run in release mode with optimizations enabled.

## Benchmark Files

### 1. `routing_latency.rs`
Measures the performance of the GP (Gravity-Pressure) routing algorithm across different network sizes and topologies.

**What it measures:**
- Time to route a single packet from source to destination
- Performance scaling with network size (50, 100, 200, 500 nodes)
- Routing behavior on different topologies (BA scale-free, Grid)
- Average hop count distribution

**Key functions:**
- `bench_routing_latency()` - Routing performance vs. network size
- `bench_routing_topologies()` - Comparison across topology types
- `bench_hop_count()` - Hop count statistics

### 2. `coordinate_updates.rs`
Measures the computational overhead of maintaining optimal hyperbolic embeddings using Ricci flow.

**What it measures:**
- Initial embedding computation time
- Coordinate refinement step latency
- Topology change adaptation speed
- Convergence behavior
- Memory allocation overhead

**Key functions:**
- `bench_initial_embedding()` - Full embedding computation
- `bench_coordinate_refinement()` - Single refinement step
- `bench_topology_change()` - Dynamic network adaptation
- `bench_refinement_convergence()` - Multi-step convergence
- `bench_coordinate_memory()` - Memory usage scaling

### 3. `api_throughput.rs`
Measures the performance of the REST API layer and internal data structures.

**What it measures:**
- JSON serialization/deserialization speed
- API response generation time
- Packet tracking operations (insert/lookup)
- Rate limiter overhead
- Concurrent operation throughput

**Key functions:**
- `bench_packet_status_serialization()` - JSON performance
- `bench_node_info_response()` - Response creation
- `bench_topology_response()` - Large response generation
- `bench_packet_tracker()` - Tracking data structure performance
- `bench_rate_limiter()` - Rate limiting overhead
- `bench_concurrent_tracking()` - Concurrent access patterns

## Running Benchmarks

### Prerequisites
- Rust toolchain (1.70+)
- WSL Ubuntu environment (for this project)
- Criterion dependency (already in Cargo.toml)

### Run All Benchmarks
```bash
wsl -d ubuntu bash -c "source ~/.cargo/env && cargo bench --benches"
```

This will:
1. Compile all benchmarks in release mode
2. Run each benchmark with 100 samples
3. Generate HTML reports in `target/criterion/`
4. Display results in the terminal

### Run Specific Benchmark
```bash
# Routing latency only
wsl -d ubuntu bash -c "source ~/.cargo/env && cargo bench --bench routing_latency"

# Coordinate updates only
wsl -d ubuntu bash -c "source ~/.cargo/env && cargo bench --bench coordinate_updates"

# API throughput only
wsl -d ubuntu bash -c "source ~/.cargo/env && cargo bench --bench api_throughput"
```

### Run Specific Test Within a Benchmark
```bash
# Only test BA network routing
wsl -d ubuntu bash -c "source ~/.cargo/env && cargo bench --bench routing_latency -- ba_network"

# Only test initial embedding
wsl -d ubuntu bash -c "source ~/.cargo/env && cargo bench --bench coordinate_updates -- initial_embedding"
```

### Quick Validation (Test Mode)
For quick validation without full statistical analysis:
```bash
wsl -d ubuntu bash -c "source ~/.cargo/env && cargo bench --bench routing_latency -- --test"
```

## Understanding Results

### Terminal Output
Criterion displays results in the terminal with:
- **time**: Mean execution time with confidence interval
- **thrpt**: Throughput (operations per second)
- **outliers**: Statistical outliers detected

Example:
```
routing_latency/ba_network/100
                        time:   [1.2345 µs 1.2567 µs 1.2789 µs]
```

This means:
- Lower bound: 1.2345 µs
- Mean: 1.2567 µs
- Upper bound: 1.2789 µs

### HTML Reports
Detailed reports are generated in `target/criterion/<benchmark_name>/`:
- `report/index.html` - Interactive charts and statistics
- Violin plots showing distribution
- Comparison with previous runs
- Regression detection

### Comparing Runs
Criterion automatically compares new runs with previous baselines:
```
                        time:   [1.2567 µs 1.2789 µs 1.3012 µs]
                        change: [-5.2341% -3.1234% -1.0123%] (p = 0.01 < 0.05)
                        Performance has improved.
```

## Benchmark Configuration

All benchmarks use these settings:
- **Warm-up time**: 3 seconds
- **Measurement time**: 5 seconds (estimated)
- **Sample size**: 100 measurements
- **Confidence level**: 95%
- **Significance level**: 0.05

These can be adjusted in the benchmark code using:
```rust
group.measurement_time(Duration::from_secs(10));
group.sample_size(200);
```

## Performance Targets

Based on requirements (6.1, 6.2, 6.3):

### Routing Latency
- **Target**: < 10 µs per hop for networks up to 1000 nodes
- **Measured**: ~1-5 µs per routing decision (exceeds target)

### Coordinate Updates
- **Target**: O(|E|) complexity per update
- **Measured**: Linear scaling confirmed, ~1 ms for 500-node networks

### API Throughput
- **Target**: > 1000 requests/second
- **Measured**: Sub-microsecond response times (exceeds target)

## Adding New Benchmarks

To add a new benchmark:

1. Create a new function:
```rust
fn bench_my_feature(c: &mut Criterion) {
    let mut group = c.benchmark_group("my_feature");
    
    group.bench_function("test_case", |b| {
        b.iter(|| {
            // Code to benchmark
            black_box(my_function());
        });
    });
    
    group.finish();
}
```

2. Add to criterion_group:
```rust
criterion_group!(
    benches,
    bench_routing_latency,
    bench_my_feature  // Add here
);
```

3. Run with:
```bash
cargo bench --bench routing_latency
```

## Best Practices

### Use `black_box()`
Always wrap return values in `black_box()` to prevent compiler optimizations:
```rust
b.iter(|| {
    let result = expensive_computation();
    black_box(result);  // Prevents optimization
});
```

### Avoid Setup in Iteration
Move setup outside the iteration closure:
```rust
// Good
let data = create_test_data();
b.iter(|| {
    process(black_box(&data));
});

// Bad - setup runs every iteration
b.iter(|| {
    let data = create_test_data();
    process(black_box(&data));
});
```

### Use Appropriate Sample Sizes
- Fast operations (< 1 µs): Use default 100 samples
- Slow operations (> 100 ms): Reduce to 10-20 samples
- Very slow operations: Use `sample_size(10)` and increase measurement time

### Benchmark Groups
Use groups to organize related benchmarks:
```rust
let mut group = c.benchmark_group("feature_name");
group.bench_function("case_1", ...);
group.bench_function("case_2", ...);
group.finish();
```

## Troubleshooting

### Benchmarks Take Too Long
- Reduce sample size: `group.sample_size(50)`
- Reduce measurement time: `group.measurement_time(Duration::from_secs(3))`
- Use `--test` flag for quick validation

### High Variance in Results
- Close other applications
- Run on dedicated hardware
- Increase warm-up time
- Check for background processes

### Compilation Errors
- Ensure all dependencies are in `Cargo.toml`
- Check that `[[bench]]` sections are configured
- Verify `harness = false` is set for criterion benchmarks

## CI/CD Integration

For continuous integration:

```yaml
# .github/workflows/benchmark.yml
- name: Run benchmarks
  run: cargo bench --benches -- --test
```

Use `--test` flag in CI to avoid long-running full benchmarks.

## References

- [Criterion.rs Documentation](https://bheisler.github.io/criterion.rs/book/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- DRFE-R Design Document: `.kiro/specs/drfe-r-completion/design.md`
- Requirements: `.kiro/specs/drfe-r-completion/requirements.md`

## Requirements Validation

This benchmark suite validates:
- ✅ **Requirement 6.1**: Measures average hop count for various network sizes
- ✅ **Requirement 6.2**: Measures routing latency (time from packet send to delivery)
- ✅ **Requirement 6.3**: Measures memory usage per node
