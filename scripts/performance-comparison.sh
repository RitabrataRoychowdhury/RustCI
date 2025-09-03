#!/bin/bash

# Performance Comparison Script
# This script runs performance validation before and after changes
# and generates a comparison report

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
REPORTS_DIR="$PROJECT_ROOT/performance_reports"
BASELINE_DIR="$REPORTS_DIR/baseline"
CURRENT_DIR="$REPORTS_DIR/current"
COMPARISON_DIR="$REPORTS_DIR/comparison"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
MODE="full"
DURATION=300
CONCURRENT_USERS=100
VERBOSE=false
EXPORT_FORMAT="all"

# Function to print colored output
print_info() {
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

# Function to show usage
show_usage() {
    cat << EOF
Performance Comparison Script

Usage: $0 [OPTIONS] COMMAND

Commands:
    baseline    Run baseline performance validation
    current     Run current performance validation
    compare     Compare baseline vs current performance
    full        Run complete comparison workflow (baseline -> current -> compare)

Options:
    -m, --mode MODE             Validation mode: quick, full, benchmark (default: full)
    -d, --duration SECONDS      Test duration in seconds (default: 300)
    -c, --concurrent-users NUM  Number of concurrent users (default: 100)
    -f, --format FORMAT         Export format: json, csv, markdown, all (default: all)
    -v, --verbose               Enable verbose output
    -h, --help                  Show this help message

Examples:
    $0 full                                    # Run complete comparison workflow
    $0 baseline -m benchmark -d 600           # Run baseline with benchmark mode for 10 minutes
    $0 current -c 500 -v                      # Run current validation with 500 users, verbose
    $0 compare                                 # Compare existing baseline and current results

Environment Variables:
    PERFORMANCE_BASELINE_FILE   Path to baseline performance file
    PERFORMANCE_REQUIREMENTS    Path to performance requirements file
EOF
}

# Function to parse command line arguments
parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            -m|--mode)
                MODE="$2"
                shift 2
                ;;
            -d|--duration)
                DURATION="$2"
                shift 2
                ;;
            -c|--concurrent-users)
                CONCURRENT_USERS="$2"
                shift 2
                ;;
            -f|--format)
                EXPORT_FORMAT="$2"
                shift 2
                ;;
            -v|--verbose)
                VERBOSE=true
                shift
                ;;
            -h|--help)
                show_usage
                exit 0
                ;;
            baseline|current|compare|full)
                COMMAND="$1"
                shift
                ;;
            *)
                print_error "Unknown option: $1"
                show_usage
                exit 1
                ;;
        esac
    done
}

# Function to check prerequisites
check_prerequisites() {
    print_info "Checking prerequisites..."
    
    # Check if Rust is installed
    if ! command -v cargo &> /dev/null; then
        print_error "Cargo not found. Please install Rust."
        exit 1
    fi
    
    # Check if the project builds
    print_info "Building project..."
    cd "$PROJECT_ROOT"
    if ! cargo build --release --bin performance_validator; then
        print_error "Failed to build performance validator"
        exit 1
    fi
    
    # Create directories
    mkdir -p "$BASELINE_DIR" "$CURRENT_DIR" "$COMPARISON_DIR"
    
    print_success "Prerequisites check completed"
}

# Function to run performance validation
run_performance_validation() {
    local output_dir="$1"
    local label="$2"
    
    print_info "Running $label performance validation..."
    
    local validator_args=(
        "--mode" "$MODE"
        "--output" "$output_dir"
        "--duration" "$DURATION"
        "--concurrent-users" "$CONCURRENT_USERS"
        "--format" "$EXPORT_FORMAT"
    )
    
    if [[ "$VERBOSE" == "true" ]]; then
        validator_args+=("--verbose")
    fi
    
    if [[ -n "${PERFORMANCE_BASELINE_FILE:-}" ]]; then
        validator_args+=("--baseline" "$PERFORMANCE_BASELINE_FILE")
    fi
    
    if [[ -n "${PERFORMANCE_REQUIREMENTS:-}" ]]; then
        validator_args+=("--requirements" "$PERFORMANCE_REQUIREMENTS")
    fi
    
    cd "$PROJECT_ROOT"
    if cargo run --release --bin performance_validator -- "${validator_args[@]}"; then
        print_success "$label validation completed successfully"
        return 0
    else
        print_error "$label validation failed"
        return 1
    fi
}

# Function to run baseline validation
run_baseline() {
    print_info "🏁 Running baseline performance validation"
    
    # Clean baseline directory
    rm -rf "$BASELINE_DIR"/*
    
    if run_performance_validation "$BASELINE_DIR" "Baseline"; then
        # Find the latest baseline file
        local baseline_file
        baseline_file=$(find "$BASELINE_DIR" -name "*.json" -type f | head -1)
        
        if [[ -n "$baseline_file" ]]; then
            # Set as environment variable for future runs
            export PERFORMANCE_BASELINE_FILE="$baseline_file"
            print_success "Baseline saved: $baseline_file"
            
            # Create a symlink for easy access
            ln -sf "$baseline_file" "$BASELINE_DIR/latest_baseline.json"
        fi
    fi
}

# Function to run current validation
run_current() {
    print_info "🚀 Running current performance validation"
    
    # Clean current directory
    rm -rf "$CURRENT_DIR"/*
    
    # Use baseline if available
    local baseline_file="${PERFORMANCE_BASELINE_FILE:-$BASELINE_DIR/latest_baseline.json}"
    if [[ -f "$baseline_file" ]]; then
        export PERFORMANCE_BASELINE_FILE="$baseline_file"
        print_info "Using baseline: $baseline_file"
    fi
    
    if run_performance_validation "$CURRENT_DIR" "Current"; then
        local current_file
        current_file=$(find "$CURRENT_DIR" -name "*.json" -type f | head -1)
        
        if [[ -n "$current_file" ]]; then
            ln -sf "$current_file" "$CURRENT_DIR/latest_current.json"
            print_success "Current results saved: $current_file"
        fi
    fi
}

# Function to compare performance results
compare_performance() {
    print_info "📊 Comparing performance results"
    
    local baseline_file="$BASELINE_DIR/latest_baseline.json"
    local current_file="$CURRENT_DIR/latest_current.json"
    
    if [[ ! -f "$baseline_file" ]]; then
        print_error "Baseline file not found: $baseline_file"
        print_info "Run 'baseline' command first"
        exit 1
    fi
    
    if [[ ! -f "$current_file" ]]; then
        print_error "Current file not found: $current_file"
        print_info "Run 'current' command first"
        exit 1
    fi
    
    print_info "Baseline: $baseline_file"
    print_info "Current:  $current_file"
    
    # Generate comparison report using Python script (if available) or simple diff
    if command -v python3 &> /dev/null; then
        generate_python_comparison "$baseline_file" "$current_file"
    else
        generate_simple_comparison "$baseline_file" "$current_file"
    fi
}

# Function to generate Python-based comparison
generate_python_comparison() {
    local baseline_file="$1"
    local current_file="$2"
    
    print_info "Generating detailed comparison report..."
    
    # Create Python comparison script
    cat > "$COMPARISON_DIR/compare.py" << 'EOF'
#!/usr/bin/env python3
import json
import sys
from datetime import datetime

def load_json(file_path):
    with open(file_path, 'r') as f:
        return json.load(f)

def calculate_change(baseline, current):
    if baseline == 0:
        return float('inf') if current > 0 else 0
    return ((current - baseline) / baseline) * 100

def format_change(change):
    if change == float('inf'):
        return "∞% ↑"
    elif change > 0:
        return f"+{change:.1f}% ↑"
    elif change < 0:
        return f"{change:.1f}% ↓"
    else:
        return "0% →"

def main():
    if len(sys.argv) != 3:
        print("Usage: compare.py <baseline.json> <current.json>")
        sys.exit(1)
    
    baseline_file = sys.argv[1]
    current_file = sys.argv[2]
    
    try:
        baseline = load_json(baseline_file)
        current = load_json(current_file)
    except Exception as e:
        print(f"Error loading files: {e}")
        sys.exit(1)
    
    print("🔍 Performance Comparison Report")
    print("=" * 50)
    
    # Extract key metrics
    baseline_latency = baseline.get('latency_benchmarks', {}).get('api_latency', {}).get('mean_us', 0)
    current_latency = current.get('latency_benchmarks', {}).get('api_latency', {}).get('mean_us', 0)
    
    baseline_throughput = baseline.get('throughput_benchmarks', {}).get('requests_per_second', 0)
    current_throughput = current.get('throughput_benchmarks', {}).get('requests_per_second', 0)
    
    baseline_cpu = baseline.get('resource_benchmarks', {}).get('resource_utilization', {}).get('cpu_peak', 0)
    current_cpu = current.get('resource_benchmarks', {}).get('resource_utilization', {}).get('cpu_peak', 0)
    
    baseline_memory = baseline.get('resource_benchmarks', {}).get('resource_utilization', {}).get('memory_peak', 0)
    current_memory = current.get('resource_benchmarks', {}).get('resource_utilization', {}).get('memory_peak', 0)
    
    # Calculate changes
    latency_change = calculate_change(baseline_latency, current_latency)
    throughput_change = calculate_change(baseline_throughput, current_throughput)
    cpu_change = calculate_change(baseline_cpu, current_cpu)
    memory_change = calculate_change(baseline_memory, current_memory)
    
    print(f"📊 Key Metrics Comparison:")
    print(f"  API Latency:     {baseline_latency:.1f}μs → {current_latency:.1f}μs ({format_change(latency_change)})")
    print(f"  Throughput:      {baseline_throughput:.0f} RPS → {current_throughput:.0f} RPS ({format_change(throughput_change)})")
    print(f"  CPU Peak:        {baseline_cpu:.1f}% → {current_cpu:.1f}% ({format_change(cpu_change)})")
    print(f"  Memory Peak:     {baseline_memory:.1f}% → {current_memory:.1f}% ({format_change(memory_change)})")
    
    # Determine overall assessment
    print(f"\n🎯 Assessment:")
    
    regressions = []
    improvements = []
    
    if latency_change > 10:
        regressions.append(f"API latency increased by {latency_change:.1f}%")
    elif latency_change < -10:
        improvements.append(f"API latency improved by {abs(latency_change):.1f}%")
    
    if throughput_change < -10:
        regressions.append(f"Throughput decreased by {abs(throughput_change):.1f}%")
    elif throughput_change > 10:
        improvements.append(f"Throughput improved by {throughput_change:.1f}%")
    
    if cpu_change > 20:
        regressions.append(f"CPU usage increased by {cpu_change:.1f}%")
    elif cpu_change < -20:
        improvements.append(f"CPU usage improved by {abs(cpu_change):.1f}%")
    
    if memory_change > 20:
        regressions.append(f"Memory usage increased by {memory_change:.1f}%")
    elif memory_change < -20:
        improvements.append(f"Memory usage improved by {abs(memory_change):.1f}%")
    
    if regressions:
        print(f"  ⚠️  Regressions detected:")
        for regression in regressions:
            print(f"    • {regression}")
    
    if improvements:
        print(f"  ✅ Improvements detected:")
        for improvement in improvements:
            print(f"    • {improvement}")
    
    if not regressions and not improvements:
        print(f"  ➡️  Performance is stable (no significant changes)")
    
    # Performance grade comparison
    baseline_grade = baseline.get('performance_grade', {})
    current_grade = current.get('performance_grade', {})
    
    if baseline_grade and current_grade:
        print(f"\n🏆 Performance Grade:")
        print(f"  Baseline: {baseline_grade}")
        print(f"  Current:  {current_grade}")

if __name__ == "__main__":
    main()
EOF
    
    chmod +x "$COMPARISON_DIR/compare.py"
    
    # Run comparison
    if python3 "$COMPARISON_DIR/compare.py" "$baseline_file" "$current_file" > "$COMPARISON_DIR/comparison_report.txt"; then
        print_success "Comparison report generated: $COMPARISON_DIR/comparison_report.txt"
        
        # Display the report
        if [[ "$VERBOSE" == "true" ]]; then
            cat "$COMPARISON_DIR/comparison_report.txt"
        else
            head -20 "$COMPARISON_DIR/comparison_report.txt"
            echo "..."
            echo "(Use --verbose to see full report)"
        fi
    else
        print_error "Failed to generate comparison report"
    fi
}

# Function to generate simple comparison
generate_simple_comparison() {
    local baseline_file="$1"
    local current_file="$2"
    
    print_info "Generating simple comparison..."
    
    {
        echo "🔍 Performance Comparison Report"
        echo "=" * 50
        echo "Baseline: $(basename "$baseline_file")"
        echo "Current:  $(basename "$current_file")"
        echo ""
        echo "Files compared:"
        echo "  Baseline size: $(wc -c < "$baseline_file") bytes"
        echo "  Current size:  $(wc -c < "$current_file") bytes"
        echo ""
        echo "Note: Install Python 3 for detailed performance analysis"
    } > "$COMPARISON_DIR/simple_comparison.txt"
    
    print_success "Simple comparison saved: $COMPARISON_DIR/simple_comparison.txt"
    cat "$COMPARISON_DIR/simple_comparison.txt"
}

# Function to run full workflow
run_full_workflow() {
    print_info "🔄 Running complete performance comparison workflow"
    
    print_info "Step 1/3: Running baseline validation..."
    run_baseline
    
    print_info "Step 2/3: Running current validation..."
    run_current
    
    print_info "Step 3/3: Comparing results..."
    compare_performance
    
    print_success "Complete workflow finished!"
    print_info "Reports available in: $REPORTS_DIR"
}

# Function to show summary
show_summary() {
    print_info "📋 Performance Comparison Summary"
    echo ""
    echo "Reports Directory: $REPORTS_DIR"
    echo ""
    
    if [[ -d "$BASELINE_DIR" ]] && [[ -n "$(ls -A "$BASELINE_DIR" 2>/dev/null)" ]]; then
        echo "📊 Baseline Reports:"
        ls -la "$BASELINE_DIR"
        echo ""
    fi
    
    if [[ -d "$CURRENT_DIR" ]] && [[ -n "$(ls -A "$CURRENT_DIR" 2>/dev/null)" ]]; then
        echo "🚀 Current Reports:"
        ls -la "$CURRENT_DIR"
        echo ""
    fi
    
    if [[ -d "$COMPARISON_DIR" ]] && [[ -n "$(ls -A "$COMPARISON_DIR" 2>/dev/null)" ]]; then
        echo "🔍 Comparison Reports:"
        ls -la "$COMPARISON_DIR"
        echo ""
    fi
    
    echo "Usage examples:"
    echo "  View baseline report:    cat $BASELINE_DIR/latest_baseline.json | jq ."
    echo "  View current report:     cat $CURRENT_DIR/latest_current.json | jq ."
    echo "  View comparison:         cat $COMPARISON_DIR/comparison_report.txt"
}

# Main execution
main() {
    local COMMAND=""
    
    # Parse arguments
    parse_args "$@"
    
    # Check if command was provided
    if [[ -z "${COMMAND:-}" ]]; then
        print_error "No command specified"
        show_usage
        exit 1
    fi
    
    # Check prerequisites
    check_prerequisites
    
    # Execute command
    case "$COMMAND" in
        baseline)
            run_baseline
            ;;
        current)
            run_current
            ;;
        compare)
            compare_performance
            ;;
        full)
            run_full_workflow
            ;;
        *)
            print_error "Unknown command: $COMMAND"
            show_usage
            exit 1
            ;;
    esac
    
    # Show summary
    show_summary
}

# Run main function with all arguments
main "$@"