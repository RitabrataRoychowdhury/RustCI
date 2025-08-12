#!/bin/bash

# Run all Valkyrie Protocol benchmarks
set -e

BENCHMARK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPORTS_DIR="$BENCHMARK_DIR/reports"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")

echo "🚀 Running Valkyrie Protocol Benchmark Suite"
echo "============================================="

# Create reports directory
mkdir -p "$REPORTS_DIR/$TIMESTAMP"

# Run default benchmarks
echo "📊 Running default benchmark suite..."
cargo run --release --bin valkyrie-benchmark -- \
    --config "$BENCHMARK_DIR/configs/default.yaml" \
    --output "$REPORTS_DIR/$TIMESTAMP/default"

# Run performance benchmarks
echo "⚡ Running high-performance benchmarks..."
cargo run --release --bin valkyrie-benchmark -- \
    --config "$BENCHMARK_DIR/configs/performance.yaml" \
    --output "$REPORTS_DIR/$TIMESTAMP/performance"

# Run compatibility benchmarks
echo "🔄 Running compatibility benchmarks..."
cargo run --release --bin valkyrie-benchmark -- \
    --config "$BENCHMARK_DIR/configs/compatibility.yaml" \
    --output "$REPORTS_DIR/$TIMESTAMP/compatibility"

echo "✅ All benchmarks completed!"
echo "📄 Reports available in: $REPORTS_DIR/$TIMESTAMP"

# Generate summary report
echo "📋 Generating summary report..."
cat > "$REPORTS_DIR/$TIMESTAMP/SUMMARY.md" << EOL
# Valkyrie Protocol Benchmark Summary

**Timestamp**: $(date)
**Test Suite Version**: 2.0

## Test Results

- **Default Benchmarks**: See \`default_report.md\`
- **Performance Benchmarks**: See \`performance_report.md\`
- **Compatibility Benchmarks**: See \`compatibility_report.md\`

## Key Metrics

- **Sub-millisecond Achievement**: $(grep -q "SUB-MILLISECOND.*ACHIEVED" */report.md && echo "✅ YES" || echo "❌ NO")
- **Performance Grade**: $(grep "Overall Grade" */report.md | head -1 | cut -d: -f2 || echo "Unknown")
- **Error Rate**: $(grep "Error Rate" */report.md | head -1 | cut -d: -f2 || echo "Unknown")

## Recommendations

Based on the benchmark results, review individual reports for detailed analysis and optimization recommendations.
EOL

echo "🎉 Benchmark suite completed successfully!"
