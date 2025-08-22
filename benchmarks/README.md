# Valkyrie Protocol Unified Benchmark Suite

This directory contains the comprehensive unified benchmark suite for Valkyrie Protocol v2.0, providing performance validation, regression detection, and CI/CD integration for enterprise-grade quality assurance.

## 🚀 Features

- **Unified Performance Validation** - Consolidates HTTP bridge, protocol core, transport, security, and end-to-end benchmarks
- **Automated Regression Detection** - Statistical analysis with configurable thresholds and CI/CD integration
- **Real-time Metrics Collection** - Performance trend analysis and predictive insights
- **Enterprise CI/CD Integration** - Automated performance gates and deployment blocking
- **Comprehensive Reporting** - Multiple output formats (JSON, CSV, Markdown, Dashboard)

## 📁 Structure

```
benchmarks/
├── src/                           # Unified benchmark engine source code
│   ├── main.rs                    # Main benchmark runner
│   ├── unified_benchmark_engine.rs # Core benchmark orchestration
│   ├── regression_detector.rs     # Performance regression analysis
│   ├── metrics_collector.rs       # Real-time metrics collection
│   └── benchmark_orchestrator.rs  # CI/CD integration & orchestration
├── configs/                       # Benchmark configuration files
│   ├── performance.yaml          # Full performance validation config
│   ├── quick.yaml                # Quick development testing config
│   └── custom.yaml               # Custom benchmark configurations
├── scripts/                       # Benchmark execution scripts
│   ├── run-performance-validation.sh # Full performance validation
│   ├── quick-benchmark.sh         # Quick development benchmark
│   └── ci-integration.sh          # CI/CD integration script
├── reports/                       # Generated benchmark reports
└── Cargo.toml                    # Rust project configuration
```

## 🎯 Performance Targets

The benchmark suite validates these sub-millisecond performance targets:

| Component | Target | Validation |
|-----------|--------|------------|
| **HTTP Bridge** | <500μs average latency | ✅ Automated |
| **Protocol Core** | <100μs message processing | ✅ Automated |
| **QUIC Transport** | 15+ Gbps throughput | ✅ Automated |
| **Unix Sockets** | 20+ Gbps throughput | ✅ Automated |
| **Security** | <50μs encryption latency | ✅ Automated |
| **End-to-End** | <100ms pipeline latency | ✅ Automated |

## 🚀 Quick Start

### 1. Quick Development Benchmark (30 seconds)

```bash
# Run quick benchmark for development
./scripts/quick-benchmark.sh
```

### 2. Full Performance Validation (5-10 minutes)

```bash
# Run comprehensive performance validation
./scripts/run-performance-validation.sh
```

### 3. Custom Configuration

```bash
# Build the benchmark binary
cargo build --release --bin valkyrie-benchmark

# Run with custom configuration
./target/release/valkyrie-benchmark \
    --config configs/performance.yaml \
    --output reports/$(date +%Y%m%d_%H%M%S) \
    --mode full \
    --regression-detection \
    --real-time-metrics \
    --verbose
```

## ⚙️ Configuration Options

### Benchmark Modes

- **`quick`** - Fast validation (30s, reduced concurrency)
- **`full`** - Comprehensive validation (5-10min, full targets)
- **`custom`** - User-defined configuration

### Command Line Options

```bash
valkyrie-benchmark [OPTIONS]

OPTIONS:
    -c, --config <FILE>         Configuration file path
    -o, --output <DIR>          Output directory for reports
    -m, --mode <MODE>           Run mode: full, quick, or custom
    -v, --verbose               Enable verbose output
        --regression-detection  Enable regression analysis
        --real-time-metrics     Enable real-time metrics collection
        --ci-cd-integration     Enable CI/CD integration
        --timeout <SECONDS>     Execution timeout (default: 3600)
```

## 📊 Output Formats

### Generated Reports

1. **JSON Report** (`valkyrie_benchmark_TIMESTAMP.json`)
   - Machine-readable comprehensive results
   - Suitable for automated processing and CI/CD

2. **Markdown Report** (`valkyrie_benchmark_TIMESTAMP.md`)
   - Human-readable detailed analysis
   - Performance grades and recommendations

3. **CSV Summary** (`valkyrie_benchmark_TIMESTAMP.csv`)
   - Tabular data for spreadsheet analysis
   - Key metrics and target achievement

4. **Performance Dashboard** (`valkyrie_benchmark_TIMESTAMP_dashboard.json`)
   - Real-time metrics and trend analysis
   - Suitable for monitoring systems

### Sample Output

```
🚀 Valkyrie Protocol Unified Benchmark Runner v2.0
==================================================

📊 Starting real-time metrics collection...
🎯 Running unified benchmark suite...

📡 Running HTTP Bridge Benchmarks...
⚙️ Running Protocol Core Benchmarks...
🌐 Running Transport Layer Benchmarks...
🔒 Running Security Layer Benchmarks...
🎯 Running End-to-End Benchmarks...

🔍 Analyzing performance for regressions...

🏆 UNIFIED VALKYRIE PROTOCOL BENCHMARK RESULTS 🏆
==================================================

🥇 OVERALL GRADE: EXCELLENT (96.2%)
   Components: HTTP Bridge: 95.0%, Protocol Core: 98.0%, Transport: 97.0%, Security: 97.0%, End-to-End: 94.0%

🎯 PERFORMANCE TARGETS:
  HTTP Bridge: 450μs (target: 500μs) - ✅
  Protocol Core: 85μs (target: 100μs) - ✅
  QUIC Transport: 16.5Gbps (target: 15.0Gbps) - ✅
  Unix Socket: 22.0Gbps (target: 20.0Gbps) - ✅
  Security: 35μs (target: 50μs) - ✅

🎉 SUB-MILLISECOND PERFORMANCE ACHIEVED! 🎉
✅ HTTP Bridge: 450μs average
✅ Protocol Core: 85μs average

✅ NO PERFORMANCE REGRESSIONS DETECTED

📊 Test completed in 287.45s
```

## 🔍 Regression Detection

The benchmark suite includes automated regression detection with:

- **Statistical Analysis** - T-tests and confidence intervals
- **Configurable Thresholds** - Custom degradation limits per metric
- **Historical Comparison** - Trend analysis over time
- **CI/CD Integration** - Automated build blocking on regressions

### Regression Configuration

```yaml
regression_detection:
  degradation_threshold_percent: 5.0    # Block on >5% degradation
  historical_window_size: 10            # Compare against last 10 runs
  significance_level: 0.95              # 95% statistical confidence
  ci_cd_integration: true               # Enable CI/CD blocking
```

## 🔗 CI/CD Integration

### GitHub Actions Example

```yaml
name: Performance Validation
on: [push, pull_request]

jobs:
  performance:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Run Performance Validation
        run: |
          cd benchmarks
          ./scripts/run-performance-validation.sh
      - name: Upload Results
        uses: actions/upload-artifact@v3
        with:
          name: performance-results
          path: benchmarks/reports/
```

### Performance Gates

The system can automatically block deployments if performance targets are not met:

```yaml
performance_gates:
  failure_action: "block"  # Options: block, warn, continue
  thresholds:
    http_bridge_latency_mean:
      threshold_value: 500.0
      comparison: "LessThan"
      critical: true
```

## 🛠️ Development

### Building

```bash
# Build benchmark binary
cargo build --release --bin valkyrie-benchmark

# Run tests
cargo test

# Check code quality
cargo clippy -- -D warnings
cargo fmt --check
```

### Adding New Benchmarks

1. Extend `UnifiedBenchmarkConfig` with new benchmark configuration
2. Add benchmark implementation to `UnifiedBenchmarkEngine`
3. Update performance targets and regression detection
4. Add configuration examples and documentation

## 📈 Performance Monitoring

### Continuous Monitoring

Set up automated performance monitoring:

```bash
# Schedule daily performance validation
0 2 * * * /path/to/benchmarks/scripts/run-performance-validation.sh

# Quick validation on every commit
git hook: ./scripts/quick-benchmark.sh
```

### Metrics Integration

The benchmark suite can export metrics to:

- **Prometheus** - Time-series metrics collection
- **Grafana** - Performance dashboards and alerting
- **InfluxDB** - Long-term performance trend storage
- **Custom APIs** - Webhook integration for custom systems

## 🎯 Best Practices

1. **Regular Validation** - Run full benchmarks before releases
2. **Regression Monitoring** - Enable automated regression detection
3. **Performance Baselines** - Establish and maintain performance baselines
4. **Trend Analysis** - Monitor performance trends over time
5. **CI/CD Integration** - Block deployments on performance regressions

## 🆘 Troubleshooting

### Common Issues

1. **Build Failures**
   ```bash
   # Clean and rebuild
   cargo clean
   cargo build --release
   ```

2. **Permission Errors**
   ```bash
   # Make scripts executable
   chmod +x scripts/*.sh
   ```

3. **Performance Issues**
   - Check system resources (CPU, memory, disk)
   - Verify network connectivity for transport tests
   - Review benchmark configuration for appropriate targets

### Getting Help

- Check the [troubleshooting guide](docs/troubleshooting.md)
- Review [performance optimization guide](docs/performance-guide.md)
- Open an issue with benchmark results and system information

---

*Valkyrie Protocol Unified Benchmark Suite v2.0 - Enterprise Performance Validation*
