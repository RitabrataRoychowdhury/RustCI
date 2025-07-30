#!/bin/bash
# Simple wrapper script to run the automated pipeline test
# This script provides a clean interface for running the comprehensive pipeline test

set -e

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 RustCI Pipeline Test Runner${NC}"
echo "=================================="
echo ""

# Check if we're in the right directory
if [[ ! -f "pipeline.yaml" ]]; then
    echo -e "${YELLOW}⚠️  Warning: pipeline.yaml not found in current directory${NC}"
    echo "Please run this script from the RustCI root directory"
    exit 1
fi

# Check if the automated test script exists
if [[ ! -f "scripts/automated-pipeline-test.sh" ]]; then
    echo -e "${YELLOW}❌ Error: automated-pipeline-test.sh not found${NC}"
    echo "Please ensure the test script is in the scripts/ directory"
    exit 1
fi

echo -e "${GREEN}✅ Found pipeline.yaml and test script${NC}"
echo ""

# Run the automated test
echo -e "${BLUE}Starting automated pipeline test...${NC}"
echo ""

# Execute the test script
if ./scripts/automated-pipeline-test.sh; then
    echo ""
    echo -e "${GREEN}🎉 Pipeline test completed successfully!${NC}"
    echo ""
    echo "Check the pipeline-test-results/ directory for detailed results:"
    echo "- Test logs and execution details"
    echo "- API responses and error messages"
    echo "- Comprehensive summary report"
    echo ""
    exit 0
else
    echo ""
    echo -e "${YELLOW}⚠️  Pipeline test completed with issues${NC}"
    echo ""
    echo "Check the pipeline-test-results/ directory for detailed analysis:"
    echo "- Error logs and failure details"
    echo "- Specific recommendations for fixes"
    echo "- Complete execution trace"
    echo ""
    exit 1
fi