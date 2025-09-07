#!/bin/bash

# Script to systematically fix unused imports
# This script identifies and removes unused imports from Rust files

echo "Starting unused import cleanup..."

# Get list of files with unused imports
FILES_WITH_UNUSED_IMPORTS=$(cargo check 2>&1 | grep -A1 "unused import" | grep "src/" | sed 's/.*--> //' | sed 's/:.*$//' | sort | uniq)

echo "Files with unused imports:"
echo "$FILES_WITH_UNUSED_IMPORTS"

# Count total warnings before
WARNINGS_BEFORE=$(cargo check 2>&1 | grep -c "warning:")
echo "Total warnings before: $WARNINGS_BEFORE"

# For each file, we'll need to manually fix the imports
# This script serves as a framework - actual fixes need to be done carefully

echo "Manual fixes needed for remaining unused imports."
echo "Use 'cargo check' to see specific unused imports and fix them one by one."

# Count warnings after (will be the same until manual fixes are applied)
WARNINGS_AFTER=$(cargo check 2>&1 | grep -c "warning:")
echo "Total warnings after: $WARNINGS_AFTER"

echo "Unused import cleanup framework ready."