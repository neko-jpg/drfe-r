# DRFE-R: Distributed Ricci Flow Embedding with Rendezvous

Welcome to the official documentation for DRFE-R, a revolutionary distributed routing protocol based on hyperbolic geometry and discrete Ricci Flow.

## What is DRFE-R?

DRFE-R is a **decentralized routing protocol** that:

- Embeds network topology into **hyperbolic space** (Poincaré disk)
- Uses **discrete Ricci Flow** to optimize coordinate assignments
- Provides **guaranteed packet delivery** via Gravity-Pressure-Tree routing
- Scales to **thousands of nodes** with O(k) complexity

## Key Features

### Mathematical Foundations
- **Poincaré Disk Model**: Conformal representation of hyperbolic geometry
- **Ollivier-Ricci Curvature**: Measures local connectivity quality
- **Adaptive Regularization**: Dynamic step size for faster convergence

### Routing Guarantees
- **Gravity Mode**: Greedy forwarding toward destination
- **Pressure Mode**: Escape local minima via backpressure
- **Tree Fallback**: BFS spanning tree ensures 100% delivery

### Production Ready
- **Lock-free Architecture**: 100K+ node parallelism
- **Byzantine Tolerance**: Defends against malicious nodes
- **OpenTelemetry**: Full distributed tracing

## Quick Start

```bash
# Clone repository
git clone https://github.com/drfe-r/drfe-r.git
cd drfe-r

# Build
cargo build --release

# Run simulator
cargo run --release --bin simulator -- --nodes 100 --tests 500
```

## Performance

| Nodes | Success Rate | Avg Hops | Stretch |
|-------|-------------|----------|---------|
| 100   | 100%        | 6.0      | 2.27    |
| 500   | 99.8%       | 17.1     | 5.29    |
| 1000  | 99.2%       | 31.1     | 8.93    |

## Next Steps

- [Hyperbolic Geometry Fundamentals](./hyperbolic/fundamentals.md)
- [Algorithm Overview](./algorithm/overview.md)
- [API Reference](./implementation/api.md)
