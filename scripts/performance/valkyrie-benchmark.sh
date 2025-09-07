#!/bin/bash

# Valkyrie Protocol Benchmark Suite
# Performance testing and load scenarios for the Valkyrie Protocol

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

# Configuration
CARGO_BIN="cargo"
BENCHMARK_DURATION=${BENCHMARK_DURATION:-60}
WARMUP_DURATION=${WARMUP_DURATION:-10}
CONCURRENT_CONNECTIONS=${CONCURRENT_CONNECTIONS:-100}
MESSAGE_SIZE=${MESSAGE_SIZE:-1024}
RESULTS_DIR="benchmark-results"
VERBOSE=${VERBOSE:-false}

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_benchmark() {
    echo -e "${PURPLE}[BENCHMARK]${NC} $1"
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking benchmark prerequisites..."
    
    # Check if cargo is installed
    if ! command -v cargo &> /dev/null; then
        log_error "Cargo is not installed. Please install Rust and Cargo."
        exit 1
    fi
    
    # Check if the project is a Rust project
    if [ ! -f "Cargo.toml" ]; then
        log_error "Not in a Rust project directory. Please run from the project root."
        exit 1
    fi
    
    # Check if performance test tools are available
    if ! command -v htop &> /dev/null && ! command -v top &> /dev/null; then
        log_warning "System monitoring tools (htop/top) not available. Resource monitoring will be limited."
    fi
    
    # Create results directory
    mkdir -p "$RESULTS_DIR"
    
    log_success "Prerequisites check passed"
}

# Build optimized binary
build_optimized() {
    log_info "Building optimized release binary..."
    
    # Build with maximum optimizations
    RUSTFLAGS="-C target-cpu=native -C opt-level=3" $CARGO_BIN build --release
    
    if [ $? -eq 0 ]; then
        log_success "Optimized binary built successfully"
    else
        log_error "Failed to build optimized binary"
        exit 1
    fi
}

# System information gathering
gather_system_info() {
    log_info "Gathering system information..."
    
    local info_file="$RESULTS_DIR/system-info.txt"
    
    cat > "$info_file" << EOF
Valkyrie Protocol Benchmark System Information
Generated: $(date)
==============================================

System Information:
- OS: $(uname -s) $(uname -r)
- Architecture: $(uname -m)
- CPU: $(grep "model name" /proc/cpuinfo | head -1 | cut -d: -f2 | xargs || echo "Unknown")
- CPU Cores: $(nproc || echo "Unknown")
- Memory: $(free -h | grep "Mem:" | awk '{print $2}' || echo "Unknown")
- Disk: $(df -h . | tail -1 | awk '{print $4}' || echo "Unknown")

Rust Information:
- Rust Version: $(rustc --version)
- Cargo Version: $(cargo --version)

Benchmark Configuration:
- Duration: ${BENCHMARK_DURATION}s
- Warmup: ${WARMUP_DURATION}s
- Concurrent Connections: $CONCURRENT_CONNECTIONS
- Message Size: ${MESSAGE_SIZE} bytes
- Results Directory: $RESULTS_DIR

EOF
    
    log_success "System information saved to $info_file"
}

# Latency benchmark
run_latency_benchmark() {
    log_benchmark "Running latency benchmark..."
    
    local results_file="$RESULTS_DIR/latency-benchmark.json"
    
    log_info "Measuring sub-millisecond latency performance..."
    
    # Run both sub-millisecond and ultra-low latency tests
    $CARGO_BIN test --release --test valkyrie performance_tests::test_sub_millisecond_latency -- \
        --nocapture --exact > "$RESULTS_DIR/latency-output.txt" 2>&1
    
    local sub_ms_result=$?
    
    $CARGO_BIN test --release --test valkyrie performance_tests::test_ultra_low_latency -- \
        --nocapture --exact >> "$RESULTS_DIR/latency-output.txt" 2>&1
    
    local ultra_low_result=$?
    
    $CARGO_BIN test --release --test valkyrie performance_tests::test_micro_benchmark_critical_path -- \
        --nocapture --exact >> "$RESULTS_DIR/latency-output.txt" 2>&1
    
    local micro_result=$?
    
    if [ $sub_ms_result -eq 0 ] && [ $ultra_low_result -eq 0 ] && [ $micro_result -eq 0 ]; then
        log_success "Latency benchmark completed - ALL SUB-MILLISECOND TARGETS MET"
        
        # Extract detailed latency statistics
        grep -E "(P50|P95|P99|P99\.9|Mean|Sub-millisecond|Ultra-fast|μs|ns)" "$RESULTS_DIR/latency-output.txt" > "$RESULTS_DIR/latency-stats.txt" || true
        
        log_info "Latency results:"
        cat "$RESULTS_DIR/latency-stats.txt" 2>/dev/null || echo "  No detailed stats available"
        
        # Validate sub-millisecond claims
        local sub_ms_percentage=$(grep "Sub-millisecond requests" "$RESULTS_DIR/latency-output.txt" | tail -1 | grep -o '[0-9.]*%' | head -1 | sed 's/%//')
        if [ ! -z "$sub_ms_percentage" ]; then
            log_success "Sub-millisecond performance: ${sub_ms_percentage}% of requests"
            
            # Ensure we meet our claims
            if (( $(echo "$sub_ms_percentage >= 99.0" | bc -l) )); then
                log_success "✅ SUB-MILLISECOND CLAIM VALIDATED: ${sub_ms_percentage}% >= 99%"
            else
                log_error "❌ SUB-MILLISECOND CLAIM FAILED: ${sub_ms_percentage}% < 99%"
                return 1
            fi
        fi
        
    else
        log_error "Latency benchmark failed - sub-millisecond targets not met"
        log_error "Sub-ms test: $sub_ms_result, Ultra-low test: $ultra_low_result, Micro test: $micro_result"
        return 1
    fi
}

# Throughput benchmark
run_throughput_benchmark() {
    log_benchmark "Running throughput benchmark..."
    
    local results_file="$RESULTS_DIR/throughput-benchmark.json"
    
    log_info "Measuring high throughput performance..."
    
    $CARGO_BIN test --release --test valkyrie performance_tests::test_high_throughput -- \
        --nocapture --exact > "$RESULTS_DIR/throughput-output.txt" 2>&1
    
    if [ $? -eq 0 ]; then
        log_success "Throughput benchmark completed"
        
        # Extract throughput statistics
        grep -E "Ops/sec|Operations|Error rate" "$RESULTS_DIR/throughput-output.txt" > "$RESULTS_DIR/throughput-stats.txt" || true
        
        log_info "Throughput results:"
        cat "$RESULTS_DIR/throughput-stats.txt" 2>/dev/null || echo "  No detailed stats available"
    else
        log_error "Throughput benchmark failed"
        return 1
    fi
}

# Scalability benchmark
run_scalability_benchmark() {
    log_benchmark "Running scalability benchmark..."
    
    log_info "Testing concurrent connections scalability..."
    
    $CARGO_BIN test --release --test valkyrie performance_tests::test_concurrent_connections_scalability -- \
        --nocapture --exact > "$RESULTS_DIR/scalability-output.txt" 2>&1
    
    if [ $? -eq 0 ]; then
        log_success "Scalability benchmark completed"
        
        # Extract scalability statistics
        grep -E "concurrent connections|Successful connections|Connection time" "$RESULTS_DIR/scalability-output.txt" > "$RESULTS_DIR/scalability-stats.txt" || true
        
        log_info "Scalability results:"
        cat "$RESULTS_DIR/scalability-stats.txt" 2>/dev/null || echo "  No detailed stats available"
    else
        log_error "Scalability benchmark failed"
        return 1
    fi
}

# Memory usage benchmark
run_memory_benchmark() {
    log_benchmark "Running memory usage benchmark..."
    
    log_info "Testing memory usage scalability..."
    
    $CARGO_BIN test --release --test valkyrie performance_tests::test_memory_usage_scalability -- \
        --nocapture --exact > "$RESULTS_DIR/memory-output.txt" 2>&1
    
    if [ $? -eq 0 ]; then
        log_success "Memory benchmark completed"
        
        # Extract memory statistics
        grep -E "Memory|MB|KB" "$RESULTS_DIR/memory-output.txt" > "$RESULTS_DIR/memory-stats.txt" || true
        
        log_info "Memory usage results:"
        cat "$RESULTS_DIR/memory-stats.txt" 2>/dev/null || echo "  No detailed stats available"
    else
        log_error "Memory benchmark failed"
        return 1
    fi
}

# Load scenario benchmarks
run_load_scenarios() {
    log_benchmark "Running load scenario benchmarks..."
    
    log_info "Testing various load scenarios..."
    
    $CARGO_BIN test --release --test valkyrie performance_tests::test_load_scenarios -- \
        --nocapture --exact > "$RESULTS_DIR/load-scenarios-output.txt" 2>&1
    
    if [ $? -eq 0 ]; then
        log_success "Load scenarios benchmark completed"
        
        # Extract load scenario statistics
        grep -E "load scenario|Successful messages|throughput|Error rate" "$RESULTS_DIR/load-scenarios-output.txt" > "$RESULTS_DIR/load-scenarios-stats.txt" || true
        
        log_info "Load scenarios results:"
        cat "$RESULTS_DIR/load-scenarios-stats.txt" 2>/dev/null || echo "  No detailed stats available"
    else
        log_error "Load scenarios benchmark failed"
        return 1
    fi
}

# Transport performance comparison
run_transport_benchmark() {
    log_benchmark "Running transport performance comparison..."
    
    log_info "Comparing transport layer performance..."
    
    $CARGO_BIN test --release --test valkyrie transport_tests::test_transport_performance_comparison -- \
        --nocapture --exact > "$RESULTS_DIR/transport-performance-output.txt" 2>&1
    
    if [ $? -eq 0 ]; then
        log_success "Transport benchmark completed"
        
        # Extract transport performance statistics
        grep -E "Transport.*connection time" "$RESULTS_DIR/transport-performance-output.txt" > "$RESULTS_DIR/transport-performance-stats.txt" || true
        
        log_info "Transport performance results:"
        cat "$RESULTS_DIR/transport-performance-stats.txt" 2>/dev/null || echo "  No detailed stats available"
    else
        log_error "Transport benchmark failed"
        return 1
    fi
}

# Security performance benchmark
run_security_benchmark() {
    log_benchmark "Running security performance benchmark..."
    
    log_info "Testing encryption performance..."
    
    $CARGO_BIN test --release --test valkyrie security_tests::test_encryption_performance -- \
        --nocapture --exact > "$RESULTS_DIR/security-performance-output.txt" 2>&1
    
    if [ $? -eq 0 ]; then
        log_success "Security benchmark completed"
        
        # Extract security performance statistics
        grep -E "Encryption method.*time" "$RESULTS_DIR/security-performance-output.txt" > "$RESULTS_DIR/security-performance-stats.txt" || true
        
        log_info "Security performance results:"
        cat "$RESULTS_DIR/security-performance-stats.txt" 2>/dev/null || echo "  No detailed stats available"
    else
        log_error "Security benchmark failed"
        return 1
    fi
}

# CPU utilization benchmark
run_cpu_benchmark() {
    log_benchmark "Running CPU utilization benchmark..."
    
    log_info "Testing CPU utilization under load..."
    
    $CARGO_BIN test --release --test valkyrie performance_tests::test_cpu_utilization -- \
        --nocapture --exact > "$RESULTS_DIR/cpu-utilization-output.txt" 2>&1
    
    if [ $? -eq 0 ]; then
        log_success "CPU utilization benchmark completed"
        
        # Extract CPU utilization statistics
        grep -E "CPU.*%" "$RESULTS_DIR/cpu-utilization-output.txt" > "$RESULTS_DIR/cpu-utilization-stats.txt" || true
        
        log_info "CPU utilization results:"
        cat "$RESULTS_DIR/cpu-utilization-stats.txt" 2>/dev/null || echo "  No detailed stats available"
    else
        log_error "CPU utilization benchmark failed"
        return 1
    fi
}

# Network bandwidth benchmark
run_bandwidth_benchmark() {
    log_benchmark "Running network bandwidth benchmark..."
    
    log_info "Testing network bandwidth utilization..."
    
    $CARGO_BIN test --release --test valkyrie performance_tests::test_network_bandwidth_utilization -- \
        --nocapture --exact > "$RESULTS_DIR/bandwidth-output.txt" 2>&1
    
    if [ $? -eq 0 ]; then
        log_success "Network bandwidth benchmark completed"
        
        # Extract bandwidth statistics
        grep -E "Bandwidth.*Mbps" "$RESULTS_DIR/bandwidth-output.txt" > "$RESULTS_DIR/bandwidth-stats.txt" || true
        
        log_info "Network bandwidth results:"
        cat "$RESULTS_DIR/bandwidth-stats.txt" 2>/dev/null || echo "  No detailed stats available"
    else
        log_error "Network bandwidth benchmark failed"
        return 1
    fi
}

# Validate performance claims
validate_performance_claims() {
    log_benchmark "Validating sub-millisecond performance claims..."
    
    local validation_file="$RESULTS_DIR/performance-validation.txt"
    
    cat > "$validation_file" << EOF
Valkyrie Protocol Performance Claims Validation
Generated: $(date)
==============================================

CLAIM: Sub-millisecond latency for 99% of requests
EOF
    
    # Extract performance metrics from test outputs
    if [ -f "$RESULTS_DIR/latency-output.txt" ]; then
        local sub_ms_percentage=$(grep "Sub-millisecond requests" "$RESULTS_DIR/latency-output.txt" | tail -1 | grep -o '[0-9.]*%' | head -1 | sed 's/%//')
        local p95_latency=$(grep "P95:" "$RESULTS_DIR/latency-output.txt" | tail -1 | grep -o '[0-9.]*μs' | head -1)
        local p99_latency=$(grep "P99:" "$RESULTS_DIR/latency-output.txt" | tail -1 | grep -o '[0-9.]*μs' | head -1)
        
        echo "RESULT: ${sub_ms_percentage}% of requests are sub-millisecond" >> "$validation_file"
        echo "P95 Latency: ${p95_latency}" >> "$validation_file"
        echo "P99 Latency: ${p99_latency}" >> "$validation_file"
        
        if [ ! -z "$sub_ms_percentage" ] && (( $(echo "$sub_ms_percentage >= 99.0" | bc -l) )); then
            echo "STATUS: ✅ CLAIM VALIDATED" >> "$validation_file"
            log_success "Performance claims validated: ${sub_ms_percentage}% sub-millisecond"
        else
            echo "STATUS: ❌ CLAIM FAILED" >> "$validation_file"
            log_error "Performance claims failed: ${sub_ms_percentage}% sub-millisecond"
            return 1
        fi
    else
        echo "STATUS: ❓ UNABLE TO VALIDATE - No test data" >> "$validation_file"
        log_warning "Unable to validate performance claims - no test data"
        return 1
    fi
    
    cat >> "$validation_file" << EOF

CLAIM: High throughput (>50,000 ops/sec)
EOF
    
    if [ -f "$RESULTS_DIR/throughput-output.txt" ]; then
        local ops_per_sec=$(grep "Ops/sec:" "$RESULTS_DIR/throughput-output.txt" | tail -1 | grep -o '[0-9.]*' | head -1)
        
        echo "RESULT: ${ops_per_sec} operations per second" >> "$validation_file"
        
        if [ ! -z "$ops_per_sec" ] && (( $(echo "$ops_per_sec >= 50000" | bc -l) )); then
            echo "STATUS: ✅ CLAIM VALIDATED" >> "$validation_file"
            log_success "Throughput claims validated: ${ops_per_sec} ops/sec"
        else
            echo "STATUS: ❌ CLAIM FAILED" >> "$validation_file"
            log_error "Throughput claims failed: ${ops_per_sec} ops/sec"
            return 1
        fi
    else
        echo "STATUS: ❓ UNABLE TO VALIDATE - No test data" >> "$validation_file"
        log_warning "Unable to validate throughput claims - no test data"
        return 1
    fi
    
    log_success "Performance validation report: $validation_file"
}

# Generate comprehensive benchmark report
generate_benchmark_report() {
    log_info "Generating comprehensive benchmark report..."
    
    local report_file="$RESULTS_DIR/valkyrie-benchmark-report.md"
    local timestamp=$(date)
    
    cat > "$report_file" << EOF
# Valkyrie Protocol Benchmark Report

**Generated:** $timestamp

## Executive Summary

This report contains comprehensive performance benchmarks for the Valkyrie Protocol implementation.

## System Configuration

EOF
    
    # Append system info
    cat "$RESULTS_DIR/system-info.txt" >> "$report_file"
    
    cat >> "$report_file" << EOF

## Benchmark Results

### Latency Performance

EOF
    
    if [ -f "$RESULTS_DIR/latency-stats.txt" ]; then
        echo '```' >> "$report_file"
        cat "$RESULTS_DIR/latency-stats.txt" >> "$report_file"
        echo '```' >> "$report_file"
    else
        echo "Latency benchmark data not available." >> "$report_file"
    fi
    
    cat >> "$report_file" << EOF

### Throughput Performance

EOF
    
    if [ -f "$RESULTS_DIR/throughput-stats.txt" ]; then
        echo '```' >> "$report_file"
        cat "$RESULTS_DIR/throughput-stats.txt" >> "$report_file"
        echo '```' >> "$report_file"
    else
        echo "Throughput benchmark data not available." >> "$report_file"
    fi
    
    cat >> "$report_file" << EOF

### Scalability Performance

EOF
    
    if [ -f "$RESULTS_DIR/scalability-stats.txt" ]; then
        echo '```' >> "$report_file"
        cat "$RESULTS_DIR/scalability-stats.txt" >> "$report_file"
        echo '```' >> "$report_file"
    else
        echo "Scalability benchmark data not available." >> "$report_file"
    fi
    
    cat >> "$report_file" << EOF

### Memory Usage

EOF
    
    if [ -f "$RESULTS_DIR/memory-stats.txt" ]; then
        echo '```' >> "$report_file"
        cat "$RESULTS_DIR/memory-stats.txt" >> "$report_file"
        echo '```' >> "$report_file"
    else
        echo "Memory usage benchmark data not available." >> "$report_file"
    fi
    
    cat >> "$report_file" << EOF

### Transport Performance Comparison

EOF
    
    if [ -f "$RESULTS_DIR/transport-performance-stats.txt" ]; then
        echo '```' >> "$report_file"
        cat "$RESULTS_DIR/transport-performance-stats.txt" >> "$report_file"
        echo '```' >> "$report_file"
    else
        echo "Transport performance benchmark data not available." >> "$report_file"
    fi
    
    cat >> "$report_file" << EOF

### Security Performance

EOF
    
    if [ -f "$RESULTS_DIR/security-performance-stats.txt" ]; then
        echo '```' >> "$report_file"
        cat "$RESULTS_DIR/security-performance-stats.txt" >> "$report_file"
        echo '```' >> "$report_file"
    else
        echo "Security performance benchmark data not available." >> "$report_file"
    fi
    
    cat >> "$report_file" << EOF

### Resource Utilization

#### CPU Utilization
EOF
    
    if [ -f "$RESULTS_DIR/cpu-utilization-stats.txt" ]; then
        echo '```' >> "$report_file"
        cat "$RESULTS_DIR/cpu-utilization-stats.txt" >> "$report_file"
        echo '```' >> "$report_file"
    else
        echo "CPU utilization benchmark data not available." >> "$report_file"
    fi
    
    cat >> "$report_file" << EOF

#### Network Bandwidth
EOF
    
    if [ -f "$RESULTS_DIR/bandwidth-stats.txt" ]; then
        echo '```' >> "$report_file"
        cat "$RESULTS_DIR/bandwidth-stats.txt" >> "$report_file"
        echo '```' >> "$report_file"
    else
        echo "Network bandwidth benchmark data not available." >> "$report_file"
    fi
    
    cat >> "$report_file" << EOF

### Load Scenarios

EOF
    
    if [ -f "$RESULTS_DIR/load-scenarios-stats.txt" ]; then
        echo '```' >> "$report_file"
        cat "$RESULTS_DIR/load-scenarios-stats.txt" >> "$report_file"
        echo '```' >> "$report_file"
    else
        echo "Load scenarios benchmark data not available." >> "$report_file"
    fi
    
    cat >> "$report_file" << EOF

## Recommendations

Based on the benchmark results, consider the following optimizations:

1. **Latency Optimization**: If P99 latency exceeds 1ms, consider tuning buffer sizes and connection pooling.
2. **Throughput Scaling**: If throughput is below 10,000 ops/sec, investigate bottlenecks in message processing.
3. **Memory Efficiency**: Monitor memory usage per connection and implement connection pooling if needed.
4. **Transport Selection**: Choose the optimal transport based on your use case requirements.
5. **Security Trade-offs**: Balance security features with performance requirements.

## Raw Data Files

All raw benchmark data is available in the following files:
- System Information: \`system-info.txt\`
- Latency Results: \`latency-output.txt\`
- Throughput Results: \`throughput-output.txt\`
- Scalability Results: \`scalability-output.txt\`
- Memory Usage: \`memory-output.txt\`
- Transport Performance: \`transport-performance-output.txt\`
- Security Performance: \`security-performance-output.txt\`
- CPU Utilization: \`cpu-utilization-output.txt\`
- Network Bandwidth: \`bandwidth-output.txt\`
- Load Scenarios: \`load-scenarios-output.txt\`

EOF
    
    log_success "Benchmark report generated: $report_file"
}

# Cleanup function
cleanup() {
    log_info "Cleaning up benchmark processes..."
    
    # Kill any background benchmark processes
    pkill -f "valkyrie.*benchmark" > /dev/null 2>&1 || true
    
    # Clean up temporary files
    rm -f /tmp/valkyrie-benchmark-* > /dev/null 2>&1 || true
    
    log_success "Cleanup completed"
}

# Main benchmark execution
run_all_benchmarks() {
    local failed_benchmarks=()
    
    log_benchmark "Starting Valkyrie Protocol benchmark suite..."
    
    # Gather system information
    gather_system_info
    
    # Run each benchmark
    run_latency_benchmark || failed_benchmarks+=("latency")
    run_throughput_benchmark || failed_benchmarks+=("throughput")
    run_scalability_benchmark || failed_benchmarks+=("scalability")
    run_memory_benchmark || failed_benchmarks+=("memory")
    run_load_scenarios || failed_benchmarks+=("load_scenarios")
    run_transport_benchmark || failed_benchmarks+=("transport")
    run_security_benchmark || failed_benchmarks+=("security")
    run_cpu_benchmark || failed_benchmarks+=("cpu")
    run_bandwidth_benchmark || failed_benchmarks+=("bandwidth")
    
    # Validate performance claims
    validate_performance_claims || failed_benchmarks+=("performance_validation")
    
    # Generate comprehensive report
    generate_benchmark_report
    
    # Report results
    if [ ${#failed_benchmarks[@]} -eq 0 ]; then
        log_success "🎉 ALL VALKYRIE PROTOCOL BENCHMARKS COMPLETED SUCCESSFULLY!"
        log_success "✅ SUB-MILLISECOND PERFORMANCE CLAIMS VALIDATED"
        log_info "Results available in: $RESULTS_DIR/"
        return 0
    else
        log_error "❌ The following benchmarks failed: ${failed_benchmarks[*]}"
        log_error "⚠️  PERFORMANCE CLAIMS MAY NOT BE MET"
        log_info "Partial results available in: $RESULTS_DIR/"
        return 1
    fi
}

# Help function
show_help() {
    cat << EOF
Valkyrie Protocol Benchmark Suite

Usage: $0 [OPTIONS] [BENCHMARK_TYPE]

Options:
    -d, --duration SECONDS  Benchmark duration (default: 60)
    -w, --warmup SECONDS    Warmup duration (default: 10)
    -c, --connections N     Concurrent connections (default: 100)
    -s, --size BYTES        Message size (default: 1024)
    -o, --output DIR        Results directory (default: benchmark-results)
    -v, --verbose           Enable verbose output
    -h, --help              Show this help message

Benchmark Types:
    latency                 Sub-millisecond latency benchmark
    throughput              High throughput benchmark
    scalability             Concurrent connections scalability
    memory                  Memory usage benchmark
    load                    Load scenario benchmarks
    transport               Transport performance comparison
    security                Security performance benchmark
    cpu                     CPU utilization benchmark
    bandwidth               Network bandwidth benchmark
    all                     Run all benchmarks (default)

Examples:
    $0                              # Run all benchmarks
    $0 latency                      # Run only latency benchmark
    $0 -d 120 -c 500 throughput     # Run throughput test for 2 minutes with 500 connections
    $0 --verbose all                # Run all benchmarks with verbose output

Environment Variables:
    BENCHMARK_DURATION      Benchmark duration in seconds
    WARMUP_DURATION         Warmup duration in seconds
    CONCURRENT_CONNECTIONS  Number of concurrent connections
    MESSAGE_SIZE            Message size in bytes
    VERBOSE                 Enable verbose output (true/false)
EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -d|--duration)
            BENCHMARK_DURATION="$2"
            shift 2
            ;;
        -w|--warmup)
            WARMUP_DURATION="$2"
            shift 2
            ;;
        -c|--connections)
            CONCURRENT_CONNECTIONS="$2"
            shift 2
            ;;
        -s|--size)
            MESSAGE_SIZE="$2"
            shift 2
            ;;
        -o|--output)
            RESULTS_DIR="$2"
            shift 2
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        latency|throughput|scalability|memory|load|transport|security|cpu|bandwidth)
            BENCHMARK_TYPE="$1"
            shift
            ;;
        all)
            BENCHMARK_TYPE="all"
            shift
            ;;
        *)
            log_error "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
done

# Set default benchmark type
BENCHMARK_TYPE=${BENCHMARK_TYPE:-all}

# Set up signal handlers
trap cleanup EXIT
trap 'log_error "Benchmark interrupted"; exit 130' INT TERM

# Main execution
main() {
    check_prerequisites
    build_optimized
    
    case $BENCHMARK_TYPE in
        latency)
            gather_system_info
            run_latency_benchmark
            ;;
        throughput)
            gather_system_info
            run_throughput_benchmark
            ;;
        scalability)
            gather_system_info
            run_scalability_benchmark
            ;;
        memory)
            gather_system_info
            run_memory_benchmark
            ;;
        load)
            gather_system_info
            run_load_scenarios
            ;;
        transport)
            gather_system_info
            run_transport_benchmark
            ;;
        security)
            gather_system_info
            run_security_benchmark
            ;;
        cpu)
            gather_system_info
            run_cpu_benchmark
            ;;
        bandwidth)
            gather_system_info
            run_bandwidth_benchmark
            ;;
        all)
            run_all_benchmarks
            ;;
        *)
            log_error "Unknown benchmark type: $BENCHMARK_TYPE"
            exit 1
            ;;
    esac
    
    local exit_code=$?
    
    if [ $exit_code -eq 0 ]; then
        log_success "Valkyrie Protocol benchmarks completed successfully!"
        log_info "Results saved to: $RESULTS_DIR/"
    else
        log_error "Valkyrie Protocol benchmarks failed!"
    fi
    
    exit $exit_code
}

# Run main function
main "$@"