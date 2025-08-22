#!/bin/bash

# Valkyrie Protocol Quick Benchmark Script
# This script runs a quick performance validation for development

set -e

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
BENCHMARK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="${BENCHMARK_DIR}/reports/quick_$(date +%Y%m%d_%H%M%S)"
CONFIG_FILE="${BENCHMARK_DIR}/configs/quick.yaml"

print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

main() {
    print_status "🚀 Valkyrie Protocol Quick Benchmark"
    print_status "===================================="
    
    # Create output directory
    mkdir -p "${OUTPUT_DIR}"
    
    # Build if needed
    if [ ! -f "${BENCHMARK_DIR}/target/release/valkyrie-benchmark" ]; then
        print_status "Building benchmark binary..."
        cd "${BENCHMARK_DIR}"
        cargo build --release --bin valkyrie-benchmark
    fi
    
    # Run quick benchmark
    print_status "Running quick benchmark..."
    cd "${BENCHMARK_DIR}"
    
    ./target/release/valkyrie-benchmark \
        --config "${CONFIG_FILE}" \
        --output "${OUTPUT_DIR}" \
        --mode quick \
        --timeout 300 \
        --verbose
    
    print_success "🎉 Quick benchmark completed!"
    print_status "Results available in: ${OUTPUT_DIR}"
    
    # Show quick summary if JSON exists
    local json_file=$(ls ${OUTPUT_DIR}/valkyrie_benchmark_*.json 2>/dev/null | head -1)
    if [ -f "${json_file}" ]; then
        print_status "Quick Results Summary:"
        echo "  Check detailed report: $(ls ${OUTPUT_DIR}/valkyrie_benchmark_*.md 2>/dev/null | head -1 | xargs basename)"
    fi
}

main "$@"