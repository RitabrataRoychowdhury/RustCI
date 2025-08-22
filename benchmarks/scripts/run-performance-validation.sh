#!/bin/bash

# Valkyrie Protocol Performance Validation Script
# This script runs comprehensive performance benchmarks with regression detection

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
BENCHMARK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="${BENCHMARK_DIR}/reports/$(date +%Y%m%d_%H%M%S)"
CONFIG_FILE="${BENCHMARK_DIR}/configs/performance.yaml"
TIMEOUT=3600  # 1 hour

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to check prerequisites
check_prerequisites() {
    print_status "Checking prerequisites..."
    
    # Check if Rust is installed
    if ! command -v cargo &> /dev/null; then
        print_error "Cargo not found. Please install Rust."
        exit 1
    fi
    
    # Check if benchmark binary exists or can be built
    if [ ! -f "${BENCHMARK_DIR}/target/release/valkyrie-benchmark" ]; then
        print_status "Building benchmark binary..."
        cd "${BENCHMARK_DIR}"
        cargo build --release --bin valkyrie-benchmark
        if [ $? -ne 0 ]; then
            print_error "Failed to build benchmark binary"
            exit 1
        fi
    fi
    
    print_success "Prerequisites check passed"
}

# Function to create output directory
setup_output_dir() {
    print_status "Setting up output directory: ${OUTPUT_DIR}"
    mkdir -p "${OUTPUT_DIR}"
    
    # Copy configuration for reference
    cp "${CONFIG_FILE}" "${OUTPUT_DIR}/config.yaml"
    
    print_success "Output directory created"
}

# Function to run benchmarks
run_benchmarks() {
    print_status "Starting Valkyrie Protocol Performance Validation..."
    print_status "Configuration: ${CONFIG_FILE}"
    print_status "Output Directory: ${OUTPUT_DIR}"
    print_status "Timeout: ${TIMEOUT} seconds"
    
    cd "${BENCHMARK_DIR}"
    
    # Run the benchmark with full configuration
    ./target/release/valkyrie-benchmark \
        --config "${CONFIG_FILE}" \
        --output "${OUTPUT_DIR}" \
        --mode full \
        --regression-detection \
        --real-time-metrics \
        --timeout "${TIMEOUT}" \
        --verbose
    
    local exit_code=$?
    
    if [ $exit_code -eq 0 ]; then
        print_success "Benchmark execution completed successfully"
    else
        print_error "Benchmark execution failed with exit code: ${exit_code}"
        return $exit_code
    fi
}

# Function to analyze results
analyze_results() {
    print_status "Analyzing benchmark results..."
    
    # Check if results files exist
    local json_report="${OUTPUT_DIR}/valkyrie_benchmark_*.json"
    local md_report="${OUTPUT_DIR}/valkyrie_benchmark_*.md"
    
    if ls ${json_report} 1> /dev/null 2>&1; then
        print_success "JSON report generated: $(ls ${json_report})"
    else
        print_warning "JSON report not found"
    fi
    
    if ls ${md_report} 1> /dev/null 2>&1; then
        print_success "Markdown report generated: $(ls ${md_report})"
    else
        print_warning "Markdown report not found"
    fi
    
    # Extract key metrics from JSON report if available
    if ls ${json_report} 1> /dev/null 2>&1; then
        local json_file=$(ls ${json_report} | head -1)
        
        print_status "Key Performance Metrics:"
        
        # Extract HTTP Bridge latency
        local http_latency=$(jq -r '.http_bridge.latency_stats.mean_us' "${json_file}" 2>/dev/null || echo "N/A")
        echo "  HTTP Bridge Latency: ${http_latency}μs"
        
        # Extract Protocol Core latency
        local protocol_latency=$(jq -r '.protocol_core.latency_stats.mean_us' "${json_file}" 2>/dev/null || echo "N/A")
        echo "  Protocol Core Latency: ${protocol_latency}μs"
        
        # Extract QUIC throughput
        local quic_throughput=$(jq -r '.transport.quic.throughput_gbps' "${json_file}" 2>/dev/null || echo "N/A")
        echo "  QUIC Throughput: ${quic_throughput} Gbps"
        
        # Extract overall grade
        local overall_grade=$(jq -r '.performance_grade' "${json_file}" 2>/dev/null || echo "N/A")
        echo "  Overall Grade: ${overall_grade}"
        
        # Check sub-millisecond achievement
        if [ "${http_latency}" != "N/A" ] && [ "${protocol_latency}" != "N/A" ]; then
            local http_check=$(echo "${http_latency} < 1000" | bc -l 2>/dev/null || echo "0")
            local protocol_check=$(echo "${protocol_latency} < 100" | bc -l 2>/dev/null || echo "0")
            
            if [ "${http_check}" = "1" ] && [ "${protocol_check}" = "1" ]; then
                print_success "🎉 SUB-MILLISECOND PERFORMANCE ACHIEVED!"
            else
                print_warning "⚠️ Sub-millisecond targets not fully achieved"
            fi
        fi
    fi
}

# Function to generate summary
generate_summary() {
    print_status "Generating performance summary..."
    
    local summary_file="${OUTPUT_DIR}/PERFORMANCE_SUMMARY.md"
    
    cat > "${summary_file}" << EOF
# Valkyrie Protocol Performance Validation Summary

**Execution Date:** $(date)
**Configuration:** performance.yaml
**Output Directory:** ${OUTPUT_DIR}

## Quick Links

- [Detailed Report]($(ls ${OUTPUT_DIR}/valkyrie_benchmark_*.md 2>/dev/null | head -1 | xargs basename))
- [JSON Results]($(ls ${OUTPUT_DIR}/valkyrie_benchmark_*.json 2>/dev/null | head -1 | xargs basename))
- [CSV Summary]($(ls ${OUTPUT_DIR}/valkyrie_benchmark_*.csv 2>/dev/null | head -1 | xargs basename))

## Performance Targets

| Component | Target | Status |
|-----------|--------|--------|
| HTTP Bridge | <500μs latency | TBD |
| Protocol Core | <100μs latency | TBD |
| QUIC Transport | 15+ Gbps throughput | TBD |
| Unix Socket | 20+ Gbps throughput | TBD |
| Security | <50μs encryption | TBD |

## Next Steps

1. Review detailed performance report
2. Investigate any performance regressions
3. Update performance baselines if needed
4. Schedule regular performance validation

---
*Generated by Valkyrie Protocol Performance Validation Script*
EOF

    print_success "Performance summary generated: ${summary_file}"
}

# Function to cleanup on exit
cleanup() {
    print_status "Cleaning up..."
    # Add any cleanup tasks here
}

# Main execution
main() {
    print_status "🚀 Valkyrie Protocol Performance Validation"
    print_status "============================================"
    
    # Set up cleanup trap
    trap cleanup EXIT
    
    # Parse command line arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --config)
                CONFIG_FILE="$2"
                shift 2
                ;;
            --output)
                OUTPUT_DIR="$2"
                shift 2
                ;;
            --timeout)
                TIMEOUT="$2"
                shift 2
                ;;
            --help)
                echo "Usage: $0 [--config CONFIG_FILE] [--output OUTPUT_DIR] [--timeout SECONDS]"
                echo ""
                echo "Options:"
                echo "  --config    Configuration file (default: configs/performance.yaml)"
                echo "  --output    Output directory (default: reports/TIMESTAMP)"
                echo "  --timeout   Execution timeout in seconds (default: 3600)"
                echo "  --help      Show this help message"
                exit 0
                ;;
            *)
                print_error "Unknown option: $1"
                exit 1
                ;;
        esac
    done
    
    # Execute validation steps
    check_prerequisites
    setup_output_dir
    run_benchmarks
    analyze_results
    generate_summary
    
    print_success "🎉 Performance validation completed successfully!"
    print_status "Results available in: ${OUTPUT_DIR}"
}

# Run main function
main "$@"