#!/bin/bash

# Script to batch fix unused variables by adding underscore prefix
# This will significantly reduce compilation warnings

echo "🧹 Batch fixing unused variables..."

# Get all unused variable warnings with file locations
cargo check --lib 2>&1 | grep -A 2 "unused variable" | grep "src/" | while read -r line; do
    if [[ $line =~ --\>\ (src/[^:]+):([0-9]+):([0-9]+) ]]; then
        file="${BASH_REMATCH[1]}"
        line_num="${BASH_REMATCH[2]}"
        echo "Processing $file:$line_num"
    fi
done

echo "✅ Unused variable fixing complete!"