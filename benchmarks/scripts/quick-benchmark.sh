#!/bin/bash

# Quick Valkyrie Protocol benchmark for development
set -e

echo "⚡ Running quick Valkyrie Protocol benchmark..."

# Run a subset of benchmarks for quick validation
cargo test --release --package rustci \
    --test valkyrie_performance_tests \
    -- --nocapture

echo "✅ Quick benchmark completed!"
