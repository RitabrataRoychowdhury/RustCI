# Valkyrie Protocol Benchmarks

This directory contains the unified benchmark suite for the Valkyrie Protocol, providing comprehensive performance testing and validation.

## Structure

```
benchmarks/
├── valkyrie/           # Organized benchmark implementations
│   ├── protocol/       # Core protocol benchmarks
│   ├── transport/      # Transport layer benchmarks
│   ├── security/       # Security layer benchmarks
│   ├── bridge/         # HTTP bridge benchmarks
│   └── integration/    # End-to-end integration benchmarks
├── configs/            # Benchmark configuration files
├── scripts/            # Benchmark runner scripts
├── reports/            # Generated benchmark reports
└── src/                # Benchmark runner binary
```

## Quick Start

```bash
# Run all benchmarks
./scripts/run-all-benchmarks.sh

# Run quick development benchmark
./scripts/quick-benchmark.sh

# Run specific benchmark configuration
cargo run --bin valkyrie-benchmark -- \
    --config configs/performance.yaml \
    --output reports/$(date +%Y%m%d_%H%M%S)
```

## Configuration

Benchmark configurations are stored in `configs/` directory:

- `default.yaml` - Standard benchmark suite
- `performance.yaml` - High-performance validation
- `compatibility.yaml` - Cross-platform compatibility tests

## Reports

Benchmark reports are generated in multiple formats:

- **Markdown** - Human-readable reports with charts and analysis
- **JSON** - Machine-readable data for further processing
- **CSV** - Tabular data for spreadsheet analysis
- **Prometheus** - Metrics for monitoring systems

## Performance Targets

The benchmark suite validates these performance targets:

- **HTTP Bridge**: <500μs average response time
- **Protocol Core**: <100μs message processing
- **QUIC Transport**: 15Gbps+ throughput
- **Unix Sockets**: 20Gbps+ throughput
- **Security**: <2ms handshake time

## Sub-millisecond Validation

The benchmark suite specifically validates Valkyrie's sub-millisecond performance claims across all components, providing detailed analysis and performance grading.
