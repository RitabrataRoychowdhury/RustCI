# Automated Pipeline Testing

This document describes the automated pipeline testing system that tests the current `pipeline.yaml` using the exact cURL commands provided by users.

## Overview

The automated pipeline test script (`scripts/automated-pipeline-test.sh`) provides comprehensive testing of the RustCI pipeline execution system. It tests the complete pipeline lifecycle from upload to execution monitoring, using the exact cURL commands that users would use.

## Features

### Core Testing Capabilities
- **Exact cURL Command Testing**: Uses the precise cURL commands provided by users
- **Complete Pipeline Lifecycle**: Tests upload, trigger, and execution monitoring
- **Comprehensive Logging**: Captures all API responses, server logs, and execution details
- **Error Analysis**: Identifies common issues like Git clone failures, Docker problems, and SSH issues
- **Docker Deployment Verification**: Tests SSH-based container deployment if available
- **Detailed Reporting**: Generates comprehensive pass/fail reports with specific error details

### Test Coverage
- JWT token generation and validation
- API health checks
- Pipeline upload via multipart form data
- Pipeline triggering with manual trigger type
- Execution monitoring and log capture
- Docker container deployment verification via SSH
- Error pattern detection and analysis

## Usage

### Quick Start

```bash
# Run from the RustCI root directory
./scripts/run-pipeline-test.sh
```

### Direct Script Execution

```bash
# Run the full automated test
./scripts/automated-pipeline-test.sh
```

### Prerequisites

The test script automatically checks for required tools:

**Required Tools:**
- `curl` - For API testing
- `cargo` - For running RustCI and token generation
- `docker` - For container operations
- `git` - For repository operations

**Optional Tools (recommended):**
- `jq` - For JSON parsing and response analysis
- `sshpass` - For SSH-based deployment testing
- `python3` with `PyJWT` - For fallback token generation

## Test Process

### 1. Environment Setup
- Creates test results directory (`pipeline-test-results/`)
- Initializes comprehensive logging
- Checks all prerequisites and dependencies

### 2. JWT Token Generation
- Attempts to use Rust binary token generator (`cargo run --bin generate_token`)
- Falls back to Python-based token generation if needed
- Validates token before proceeding

### 3. Server Management
- Checks if RustCI server is already running
- Starts server if needed with full logging
- Waits for server to be ready with health checks

### 4. API Testing
- Tests health endpoint
- Executes exact cURL commands for pipeline upload
- Executes exact cURL commands for pipeline triggering
- Captures all HTTP responses and timing information

### 5. Execution Monitoring
- Monitors server logs for execution progress
- Captures real-time execution details
- Identifies error patterns (Git clone, Docker, SSH issues)
- Records complete execution trace

### 6. Deployment Verification
- Tests SSH connectivity to deployment server (localhost:2222)
- Checks Docker container status via SSH
- Verifies container health endpoints
- Captures container logs for analysis

### 7. Analysis and Reporting
- Analyzes server logs for common issues
- Generates comprehensive test summary
- Creates detailed markdown report
- Provides specific recommendations for fixes

## Output Files

All test results are saved in the `pipeline-test-results/` directory:

### Core Files
- `test_execution_TIMESTAMP.log` - Complete test execution log
- `test_summary_TIMESTAMP.md` - Comprehensive markdown summary report

### API Responses
- `upload_response.json` - Pipeline upload API response
- `trigger_response.json` - Pipeline trigger API response

### Execution Data
- `execution_logs.txt` - Server logs during pipeline execution
- `container_logs.txt` - Docker container logs (if available)
- `analysis_results.txt` - Automated issue analysis results

## Example cURL Commands Tested

### Pipeline Upload
```bash
curl --location 'http://localhost:8000/api/ci/pipelines/upload' \
  --header 'Authorization: Bearer [JWT_TOKEN]' \
  --form 'pipeline=@pipeline.yaml'
```

### Pipeline Trigger
```bash
curl --location 'http://localhost:8000/api/ci/pipelines/{PIPELINE_ID}/trigger' \
  --header 'Authorization: Bearer [JWT_TOKEN]' \
  --header 'Content-Type: application/json' \
  --data '{"trigger_type": "manual"}'
```

## Common Issues Detected

The test script automatically detects and reports:

### Git Clone Issues
- **Workspace Directory Missing**: `fatal: Unable to read current working directory`
- **Hardcoded Paths**: Usage of `/tmp/rustci` instead of dynamic workspace paths
- **Permission Problems**: Directory access and creation issues

### Docker Issues
- **Build Failures**: Docker image build problems
- **Daemon Connectivity**: Docker daemon connection issues
- **Permission Problems**: Docker access permission errors

### SSH Deployment Issues
- **Connection Failures**: SSH server connectivity problems
- **Authentication Issues**: SSH credential problems
- **Container Status**: Docker container deployment verification

### Workspace Management Issues
- **Creation Timing**: Workspace created after Git clone attempts
- **Path Resolution**: Hardcoded vs. dynamic path usage
- **Cleanup Problems**: Workspace cleanup and management issues

## Integration with Development Workflow

### Continuous Testing
```bash
# Run test after code changes
./scripts/run-pipeline-test.sh

# Check specific issues
grep -i "error\|fail" pipeline-test-results/test_execution_*.log
```

### Debugging Pipeline Issues
```bash
# Run test and examine detailed logs
./scripts/automated-pipeline-test.sh
cat pipeline-test-results/execution_logs.txt
```

### Validating Fixes
```bash
# Test after implementing fixes
./scripts/run-pipeline-test.sh
# Check success rate in summary report
cat pipeline-test-results/test_summary_*.md
```

## Configuration

### Environment Variables
The test script uses standard RustCI environment variables:
- `JWT_SECRET` - For token generation
- `MONGODB_URI` - For database connection
- `PORT` - For server configuration

### Test Parameters
Key parameters can be modified in the script:
- `API_BASE_URL` - Default: `http://localhost:8000`
- `PIPELINE_YAML_PATH` - Default: `pipeline.yaml`
- Monitor duration and timeout values

## Troubleshooting

### Common Issues

**Token Generation Fails**
```bash
# Install PyJWT for fallback
pip3 install PyJWT

# Or use manual token generation
./scripts/get-admin-token.sh
```

**Server Won't Start**
```bash
# Check for port conflicts
lsof -i :8000

# Clean up existing processes
pkill -f "cargo.*run"
```

**SSH Testing Fails**
```bash
# Install sshpass
brew install sshpass  # macOS
apt-get install sshpass  # Linux

# Start SSH server for testing
# (Use deployment scripts to set up test environment)
```

### Log Analysis

**Check for specific errors:**
```bash
# Git clone issues
grep -i "git.*error\|clone.*fail" pipeline-test-results/test_execution_*.log

# Docker issues
grep -i "docker.*error\|build.*fail" pipeline-test-results/test_execution_*.log

# SSH issues
grep -i "ssh.*error\|connection.*fail" pipeline-test-results/test_execution_*.log
```

## Next Steps

After running the automated test:

1. **Review Summary Report**: Check `test_summary_*.md` for overall results
2. **Analyze Specific Failures**: Examine detailed logs for error patterns
3. **Implement Fixes**: Address identified issues based on recommendations
4. **Re-test**: Run the test again to verify fixes
5. **Iterate**: Continue testing and fixing until all tests pass

## Contributing

To extend the automated testing:

1. **Add New Test Cases**: Extend the test functions for additional scenarios
2. **Improve Error Detection**: Add new error patterns to the analysis
3. **Enhance Reporting**: Add more detailed analysis and recommendations
4. **Add Integration Tests**: Include tests for different deployment types

The automated testing system is designed to be comprehensive, reliable, and easy to use for validating pipeline execution functionality.