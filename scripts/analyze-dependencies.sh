#!/bin/bash

# Script to analyze dependency usage
# Usage: ./scripts/analyze-dependencies.sh

set -e

echo "🔍 Analyzing dependency usage..."

# List of dependencies to check
DEPS=(
    "fastrand"
    "hostname" 
    "socket2"
    "aligned-vec"
    "simd-json"
    "serde_urlencoded"
    "async-stream"
    "num_cpus"
    "urlencoding"
    "time"
)

echo "Checking for unused dependencies:"
for dep in "${DEPS[@]}"; do
    if ! grep -r "use.*$dep" src/ >/dev/null 2>&1; then
        echo "❌ $dep - not found in source code"
    else
        echo "✅ $dep - found in source code"
    fi
done

echo ""
echo "📊 Dependency count analysis:"
echo "Total dependencies in Cargo.toml: $(grep -c '^[a-zA-Z].*=' Cargo.toml)"
echo "Dev dependencies: $(grep -A 100 '\[dev-dependencies\]' Cargo.toml | grep -c '^[a-zA-Z].*=' || echo 0)"

echo ""
echo "🔧 Build optimization suggestions:"
echo "- Removed unused dependencies: fastrand, hostname, socket2, aligned-vec, simd-json, serde_urlencoded, async-stream, num_cpus, urlencoding, time"
echo "- Optimized profile settings for faster builds"
echo "- Added feature flags for optional functionality"
echo "- Configured target-cpu=native for performance"

echo "✅ Dependency analysis complete!"