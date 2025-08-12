#!/bin/bash

# Script to measure build time improvements
# Usage: ./scripts/measure-build-time.sh

set -e

echo "🔧 Measuring build performance..."

# Clean previous builds
echo "Cleaning previous builds..."
cargo clean

# Measure check time
echo "📊 Measuring cargo check time..."
time_output=$(time cargo check --quiet 2>&1 | tail -1)
echo "Cargo check time: $time_output"

# Clean again for build measurement
cargo clean

# Measure build time
echo "📊 Measuring cargo build time..."
time_output=$(time cargo build --quiet 2>&1 | tail -1)
echo "Cargo build time: $time_output"

# Measure incremental build time
echo "📊 Measuring incremental build time..."
# Make a small change
touch src/main.rs
time_output=$(time cargo build --quiet 2>&1 | tail -1)
echo "Incremental build time: $time_output"

# Test release build time
echo "📊 Measuring release build time..."
cargo clean
time_output=$(time cargo build --release --quiet 2>&1 | tail -1)
echo "Release build time: $time_output"

echo "✅ Build time measurement complete!"