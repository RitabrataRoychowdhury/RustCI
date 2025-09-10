# Runner Testing Guide

This guide provides comprehensive instructions for testing RustCI runners, including setup, execution, validation, and troubleshooting.

## Overview

RustCI runner testing covers multiple aspects:
- **Functional Testing**: Verify runner capabilities and job execution
- **Performance Testing**: Measure execution times and resource usage
- **Integration Testing**: Test runner-server communication
- **Security Testing**: Validate isolation and security controls
- **Stress Testing**: Test under high load conditions

## Test Environment Setup

### Prerequisites

1. **System Requirements**
   ```bash
   # Required tools
   - Docker Desktop or Docker Engine
   - Rust 1.70+ with Cargo
   - curl, jq, bc (for testing scripts)
   - Git (for repository operations)
   ```

2. **Network Requirements**
   ```bash
   # Required ports
   - 8080: RustCI server
   - 8081: Default runner port
   - 2376: Docker daemon (DIND)
   - 9090: Metrics endpoint
   ```

3. **Environment Variables**
   ```bash
   export RUSTCI_SERVER="http://localhost:8080"
   export TEST_TIMEOUT="300"
   export PERFORMANCE_ITERATIONS="5"
   ```

### Quick Setup

1. **Start RustCI Server**
   ```bash
   # Terminal 1: Start server
   cd /path/to/rustci
   cargo run
   ```

2. **Set Up Test Environment**
   ```bash
   # Terminal 2: Set up DIND
   ./scripts/runners/setup-dind-environment.sh start
   
   # Verify setup
   ./scripts/runners/manage-runners.sh status
   ```

3. **Run Comprehensive Tests**
   ```bash
   # Run all tests
   ./scripts/runners/comprehensive-runner-test.sh
   
   # Run specific test categories
   ./scripts/runners/comprehensive-runner-test.sh --timeout 600
   ```

## Test Categories

### 1. Functional Testing

#### Basic Runner Tests

Test basic runner functionality and registration:

```bash
# Test runner registration
curl -X POST http://localhost:8080/api/runners/register \
  -H "Content-Type: application/json" \
  -d '{
    "name": "test-runner",
    "type": "docker",
    "host": "localhost",
    "port": 8081,
    "capabilities": ["docker"]
  }'

# Verify registration
curl http://localhost:8080/api/runners

# Test runner health
curl http://localhost:8080/api/runners/{runner_id}/health
```

#### Job Execution Tests

Test job submission and execution:

```bash
# Submit simple job
curl -X POST http://localhost:8080/api/jobs \
  -H "Content-Type: application/json" \
  -d '{
    "name": "hello-world",
    "steps": [
      {
        "name": "greet",
        "run": "echo Hello World"
      }
    ]
  }'

# Monitor job status
curl http://localhost:8080/api/jobs/{job_id}/status

# Get job logs
curl http://localhost:8080/api/jobs/{job_id}/logs
```

#### Pipeline Tests

Test complex pipeline execution:

```yaml
# test-pipeline.yaml
name: "Functional Test Pipeline"
stages:
  - name: "setup"
    jobs:
      - name: "environment-setup"
        steps:
          - name: "check-environment"
            run: |
              echo "Testing environment setup"
              uname -a
              env | grep -E "^(PATH|HOME)" | head -5
              
  - name: "build"
    jobs:
      - name: "build-test"
        steps:
          - name: "create-artifact"
            run: |
              mkdir -p /tmp/artifacts
              echo "Build artifact $(date)" > /tmp/artifacts/build.txt
              
  - name: "test"
    jobs:
      - name: "unit-tests"
        steps:
          - name: "run-tests"
            run: |
              echo "Running unit tests"
              # Simulate test execution
              sleep 2
              echo "Tests passed"
```

```bash
# Submit pipeline
curl -X POST http://localhost:8080/api/pipelines/run \
  -H "Content-Type: application/yaml" \
  --data-binary @test-pipeline.yaml
```

### 2. Performance Testing

#### Execution Time Benchmarks

```bash
#!/bin/bash
# performance-test.sh

ITERATIONS=10
TOTAL_TIME=0

echo "Running performance tests..."

for i in $(seq 1 $ITERATIONS); do
    START_TIME=$(date +%s.%N)
    
    # Submit and wait for job completion
    JOB_ID=$(curl -s -X POST http://localhost:8080/api/jobs \
        -H "Content-Type: application/json" \
        -d '{
            "name": "perf-test-'$i'",
            "steps": [{"name": "test", "run": "echo Performance test '$i'"}]
        }' | jq -r '.job_id')
    
    # Wait for completion
    while true; do
        STATUS=$(curl -s http://localhost:8080/api/jobs/$JOB_ID/status | jq -r '.status')
        if [[ "$STATUS" == "completed" || "$STATUS" == "failed" ]]; then
            break
        fi
        sleep 0.1
    done
    
    END_TIME=$(date +%s.%N)
    DURATION=$(echo "$END_TIME - $START_TIME" | bc -l)
    TOTAL_TIME=$(echo "$TOTAL_TIME + $DURATION" | bc -l)
    
    echo "Test $i: ${DURATION}s"
done

AVERAGE=$(echo "scale=3; $TOTAL_TIME / $ITERATIONS" | bc -l)
echo "Average execution time: ${AVERAGE}s"
```

#### Resource Usage Monitoring

```bash
#!/bin/bash
# resource-monitor.sh

echo "Monitoring resource usage..."

# Start monitoring in background
(
    while true; do
        CPU=$(docker stats --no-stream --format "{{.CPUPerc}}" rustci-dind 2>/dev/null | sed 's/%//')
        MEM=$(docker stats --no-stream --format "{{.MemUsage}}" rustci-dind 2>/dev/null)
        echo "$(date): CPU=${CPU}%, Memory=${MEM}"
        sleep 5
    done
) &

MONITOR_PID=$!

# Run test workload
./scripts/runners/comprehensive-runner-test.sh

# Stop monitoring
kill $MONITOR_PID
```

#### Concurrent Job Testing

```bash
#!/bin/bash
# concurrent-test.sh

CONCURRENT_JOBS=5
JOB_IDS=()

echo "Starting $CONCURRENT_JOBS concurrent jobs..."

# Submit concurrent jobs
for i in $(seq 1 $CONCURRENT_JOBS); do
    JOB_ID=$(curl -s -X POST http://localhost:8080/api/jobs \
        -H "Content-Type: application/json" \
        -d '{
            "name": "concurrent-job-'$i'",
            "steps": [
                {
                    "name": "work",
                    "run": "echo Job '$i' started && sleep 10 && echo Job '$i' completed"
                }
            ]
        }' | jq -r '.job_id')
    
    JOB_IDS+=($JOB_ID)
    echo "Started job $i: $JOB_ID"
done

# Monitor all jobs
echo "Monitoring job completion..."
for JOB_ID in "${JOB_IDS[@]}"; do
    while true; do
        STATUS=$(curl -s http://localhost:8080/api/jobs/$JOB_ID/status | jq -r '.status')
        if [[ "$STATUS" == "completed" || "$STATUS" == "failed" ]]; then
            echo "Job $JOB_ID: $STATUS"
            break
        fi
        sleep 1
    done
done

echo "All concurrent jobs completed"
```

### 3. Integration Testing

#### Server-Runner Communication

```bash
#!/bin/bash
# integration-test.sh

echo "Testing server-runner integration..."

# Test 1: Runner registration and heartbeat
echo "1. Testing runner registration..."
RUNNER_RESPONSE=$(curl -s -X POST http://localhost:8080/api/runners/register \
    -H "Content-Type: application/json" \
    -d '{
        "name": "integration-test-runner",
        "type": "docker",
        "host": "localhost",
        "port": 8081,
        "capabilities": ["docker", "test"]
    }')

RUNNER_ID=$(echo "$RUNNER_RESPONSE" | jq -r '.runner_id')
echo "Runner registered: $RUNNER_ID"

# Test 2: Health check
echo "2. Testing health check..."
HEALTH_RESPONSE=$(curl -s http://localhost:8080/api/runners/$RUNNER_ID/health)
HEALTH_STATUS=$(echo "$HEALTH_RESPONSE" | jq -r '.health')
echo "Health status: $HEALTH_STATUS"

# Test 3: Job assignment
echo "3. Testing job assignment..."
JOB_RESPONSE=$(curl -s -X POST http://localhost:8080/api/runners/$RUNNER_ID/jobs \
    -H "Content-Type: application/json" \
    -d '{
        "name": "integration-test-job",
        "steps": [
            {
                "name": "test-step",
                "run": "echo Integration test successful"
            }
        ]
    }')

JOB_ID=$(echo "$JOB_RESPONSE" | jq -r '.job_id')
echo "Job assigned: $JOB_ID"

# Test 4: Job execution monitoring
echo "4. Monitoring job execution..."
while true; do
    STATUS_RESPONSE=$(curl -s http://localhost:8080/api/jobs/$JOB_ID/status)
    STATUS=$(echo "$STATUS_RESPONSE" | jq -r '.status')
    
    echo "Job status: $STATUS"
    
    if [[ "$STATUS" == "completed" ]]; then
        echo "✅ Integration test passed"
        break
    elif [[ "$STATUS" == "failed" ]]; then
        echo "❌ Integration test failed"
        break
    fi
    
    sleep 2
done

# Cleanup
curl -s -X DELETE http://localhost:8080/api/runners/$RUNNER_ID
echo "Cleanup completed"
```

#### DIND Integration Testing

```bash
#!/bin/bash
# dind-integration-test.sh

echo "Testing DIND integration..."

# Start DIND simulation
./scripts/runners/dind-server-runner-simulation.sh start

# Wait for services to be ready
sleep 30

# Test server health
echo "Testing server health..."
if curl -s http://localhost:8080/health | jq -e '.status == "healthy"'; then
    echo "✅ Server health check passed"
else
    echo "❌ Server health check failed"
    exit 1
fi

# Test runner registration
echo "Testing runner registration..."
RUNNERS=$(curl -s http://localhost:8080/api/runners | jq '. | length')
if [[ $RUNNERS -gt 0 ]]; then
    echo "✅ Runner registration test passed ($RUNNERS runners)"
else
    echo "❌ Runner registration test failed"
    exit 1
fi

# Test job execution
echo "Testing job execution..."
JOB_RESPONSE=$(curl -s -X POST http://localhost:8080/api/jobs \
    -H "Content-Type: application/json" \
    -d '{
        "name": "dind-integration-test",
        "steps": [
            {
                "name": "docker-test",
                "run": "docker --version && docker run --rm hello-world"
            }
        ]
    }')

JOB_ID=$(echo "$JOB_RESPONSE" | jq -r '.job_id')

# Monitor job
TIMEOUT=120
WAITED=0
while [[ $WAITED -lt $TIMEOUT ]]; do
    STATUS=$(curl -s http://localhost:8080/api/jobs/$JOB_ID/status | jq -r '.status')
    
    if [[ "$STATUS" == "completed" ]]; then
        echo "✅ DIND integration test passed"
        break
    elif [[ "$STATUS" == "failed" ]]; then
        echo "❌ DIND integration test failed"
        curl -s http://localhost:8080/api/jobs/$JOB_ID/logs
        exit 1
    fi
    
    sleep 5
    WAITED=$((WAITED + 5))
done

if [[ $WAITED -ge $TIMEOUT ]]; then
    echo "❌ DIND integration test timed out"
    exit 1
fi

# Cleanup
./scripts/runners/dind-server-runner-simulation.sh cleanup
echo "DIND integration test completed"
```

### 4. Security Testing

#### Isolation Testing

```bash
#!/bin/bash
# security-test.sh

echo "Testing runner security and isolation..."

# Test 1: Container isolation
echo "1. Testing container isolation..."
JOB_RESPONSE=$(curl -s -X POST http://localhost:8080/api/jobs \
    -H "Content-Type: application/json" \
    -d '{
        "name": "isolation-test",
        "steps": [
            {
                "name": "isolation-check",
                "run": "ps aux | wc -l && ls -la /proc | wc -l"
            }
        ]
    }')

JOB_ID=$(echo "$JOB_RESPONSE" | jq -r '.job_id')

# Wait for completion and check logs
while true; do
    STATUS=$(curl -s http://localhost:8080/api/jobs/$JOB_ID/status | jq -r '.status')
    if [[ "$STATUS" == "completed" || "$STATUS" == "failed" ]]; then
        break
    fi
    sleep 1
done

LOGS=$(curl -s http://localhost:8080/api/jobs/$JOB_ID/logs)
echo "Isolation test logs: $LOGS"

# Test 2: File system isolation
echo "2. Testing file system isolation..."
JOB_RESPONSE=$(curl -s -X POST http://localhost:8080/api/jobs \
    -H "Content-Type: application/json" \
    -d '{
        "name": "filesystem-test",
        "steps": [
            {
                "name": "filesystem-check",
                "run": "df -h && mount | grep -v proc | wc -l"
            }
        ]
    }')

# Test 3: Network isolation
echo "3. Testing network isolation..."
JOB_RESPONSE=$(curl -s -X POST http://localhost:8080/api/jobs \
    -H "Content-Type: application/json" \
    -d '{
        "name": "network-test",
        "steps": [
            {
                "name": "network-check",
                "run": "ip addr show && netstat -rn"
            }
        ]
    }')

echo "Security tests completed"
```

#### Input Validation Testing

```bash
#!/bin/bash
# input-validation-test.sh

echo "Testing input validation..."

# Test 1: Invalid job data
echo "1. Testing invalid job data..."
INVALID_RESPONSE=$(curl -s -X POST http://localhost:8080/api/jobs \
    -H "Content-Type: application/json" \
    -d '{
        "invalid": "data",
        "steps": "not_an_array"
    }')

if echo "$INVALID_RESPONSE" | jq -e '.error'; then
    echo "✅ Invalid job data properly rejected"
else
    echo "❌ Invalid job data not properly validated"
fi

# Test 2: Malicious commands
echo "2. Testing malicious command injection..."
MALICIOUS_RESPONSE=$(curl -s -X POST http://localhost:8080/api/jobs \
    -H "Content-Type: application/json" \
    -d '{
        "name": "malicious-test",
        "steps": [
            {
                "name": "injection-test",
                "run": "echo test; rm -rf / --no-preserve-root"
            }
        ]
    }')

# This should be handled by the runner's security controls
echo "Malicious command test submitted (should be contained)"

# Test 3: Large payload
echo "3. Testing large payload handling..."
LARGE_COMMAND=$(printf 'echo %*s' 10000 | tr ' ' 'A')
LARGE_RESPONSE=$(curl -s -X POST http://localhost:8080/api/jobs \
    -H "Content-Type: application/json" \
    -d "{
        \"name\": \"large-payload-test\",
        \"steps\": [
            {
                \"name\": \"large-command\",
                \"run\": \"$LARGE_COMMAND\"
            }
        ]
    }")

if echo "$LARGE_RESPONSE" | jq -e '.job_id'; then
    echo "✅ Large payload handled"
else
    echo "❌ Large payload rejected"
fi

echo "Input validation tests completed"
```

### 5. Stress Testing

#### High Load Testing

```bash
#!/bin/bash
# stress-test.sh

CONCURRENT_JOBS=20
DURATION=300  # 5 minutes
JOB_COUNT=0
SUCCESS_COUNT=0
FAILURE_COUNT=0

echo "Starting stress test: $CONCURRENT_JOBS concurrent jobs for ${DURATION}s"

START_TIME=$(date +%s)
END_TIME=$((START_TIME + DURATION))

# Function to submit job
submit_job() {
    local job_id=$1
    local response=$(curl -s -X POST http://localhost:8080/api/jobs \
        -H "Content-Type: application/json" \
        -d "{
            \"name\": \"stress-test-$job_id\",
            \"steps\": [
                {
                    \"name\": \"work\",
                    \"run\": \"echo Stress test $job_id && sleep \$((RANDOM % 10 + 1))\"
                }
            ]
        }")
    
    local job_uuid=$(echo "$response" | jq -r '.job_id // empty')
    if [[ -n "$job_uuid" ]]; then
        echo "Submitted job $job_id: $job_uuid"
        
        # Monitor job completion
        while true; do
            local status=$(curl -s http://localhost:8080/api/jobs/$job_uuid/status | jq -r '.status // "unknown"')
            case "$status" in
                "completed")
                    ((SUCCESS_COUNT++))
                    echo "Job $job_id completed successfully"
                    break
                    ;;
                "failed"|"error")
                    ((FAILURE_COUNT++))
                    echo "Job $job_id failed"
                    break
                    ;;
                "unknown")
                    echo "Job $job_id status unknown, retrying..."
                    sleep 1
                    ;;
                *)
                    sleep 1
                    ;;
            esac
        done
    else
        ((FAILURE_COUNT++))
        echo "Failed to submit job $job_id"
    fi
}

# Submit jobs continuously
while [[ $(date +%s) -lt $END_TIME ]]; do
    # Maintain concurrent job count
    ACTIVE_JOBS=$(jobs -r | wc -l)
    
    if [[ $ACTIVE_JOBS -lt $CONCURRENT_JOBS ]]; then
        ((JOB_COUNT++))
        submit_job $JOB_COUNT &
    fi
    
    sleep 0.1
done

# Wait for remaining jobs
wait

echo "Stress test completed:"
echo "  Total jobs: $JOB_COUNT"
echo "  Successful: $SUCCESS_COUNT"
echo "  Failed: $FAILURE_COUNT"
echo "  Success rate: $(echo "scale=2; $SUCCESS_COUNT * 100 / $JOB_COUNT" | bc -l)%"
```

#### Memory Leak Testing

```bash
#!/bin/bash
# memory-leak-test.sh

echo "Testing for memory leaks..."

# Get initial memory usage
INITIAL_MEMORY=$(docker stats --no-stream --format "{{.MemUsage}}" rustci-dind 2>/dev/null | cut -d'/' -f1 | sed 's/[^0-9.]//g')

echo "Initial memory usage: ${INITIAL_MEMORY}MB"

# Run many jobs
for i in $(seq 1 100); do
    curl -s -X POST http://localhost:8080/api/jobs \
        -H "Content-Type: application/json" \
        -d "{
            \"name\": \"memory-test-$i\",
            \"steps\": [
                {
                    \"name\": \"memory-work\",
                    \"run\": \"echo Memory test $i && dd if=/dev/zero of=/tmp/test$i bs=1M count=10 && rm /tmp/test$i\"
                }
            ]
        }" > /dev/null
    
    if [[ $((i % 10)) -eq 0 ]]; then
        echo "Submitted $i jobs..."
        sleep 5  # Allow jobs to complete
        
        CURRENT_MEMORY=$(docker stats --no-stream --format "{{.MemUsage}}" rustci-dind 2>/dev/null | cut -d'/' -f1 | sed 's/[^0-9.]//g')
        echo "Current memory usage: ${CURRENT_MEMORY}MB"
    fi
done

# Wait for all jobs to complete
sleep 30

# Get final memory usage
FINAL_MEMORY=$(docker stats --no-stream --format "{{.MemUsage}}" rustci-dind 2>/dev/null | cut -d'/' -f1 | sed 's/[^0-9.]//g')

echo "Final memory usage: ${FINAL_MEMORY}MB"

# Calculate memory increase
MEMORY_INCREASE=$(echo "$FINAL_MEMORY - $INITIAL_MEMORY" | bc -l)
echo "Memory increase: ${MEMORY_INCREASE}MB"

if (( $(echo "$MEMORY_INCREASE > 100" | bc -l) )); then
    echo "⚠️  Potential memory leak detected (increase > 100MB)"
else
    echo "✅ No significant memory leak detected"
fi
```

## Automated Testing

### Continuous Integration

```yaml
# .github/workflows/runner-tests.yml
name: Runner Tests

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

jobs:
  runner-tests:
    runs-on: ubuntu-latest
    
    services:
      docker:
        image: docker:dind
        options: --privileged
        
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          
      - name: Build RustCI
        run: cargo build --release
        
      - name: Start RustCI Server
        run: |
          cargo run &
          sleep 10
          
      - name: Setup DIND Environment
        run: ./scripts/runners/setup-dind-environment.sh start
        
      - name: Run Comprehensive Tests
        run: ./scripts/runners/comprehensive-runner-test.sh
        
      - name: Run Integration Tests
        run: ./scripts/runners/dind-server-runner-simulation.sh test
        
      - name: Cleanup
        if: always()
        run: |
          ./scripts/runners/setup-dind-environment.sh cleanup
          ./scripts/runners/dind-server-runner-simulation.sh cleanup
```

### Test Reporting

```bash
#!/bin/bash
# generate-test-report.sh

REPORT_DIR="test-reports"
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
REPORT_FILE="$REPORT_DIR/runner-test-report-$TIMESTAMP.html"

mkdir -p "$REPORT_DIR"

cat > "$REPORT_FILE" << 'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>RustCI Runner Test Report</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        .header { background: #f0f0f0; padding: 20px; border-radius: 5px; }
        .test-section { margin: 20px 0; }
        .pass { color: green; }
        .fail { color: red; }
        .skip { color: orange; }
        table { border-collapse: collapse; width: 100%; }
        th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }
        th { background-color: #f2f2f2; }
    </style>
</head>
<body>
    <div class="header">
        <h1>RustCI Runner Test Report</h1>
        <p>Generated: $(date)</p>
        <p>Environment: $(uname -a)</p>
    </div>
EOF

# Run tests and capture results
echo "Running comprehensive tests..."
./scripts/runners/comprehensive-runner-test.sh > /tmp/test-output.txt 2>&1

# Parse results and add to report
echo "    <div class='test-section'>" >> "$REPORT_FILE"
echo "        <h2>Test Results</h2>" >> "$REPORT_FILE"
echo "        <pre>$(cat /tmp/test-output.txt)</pre>" >> "$REPORT_FILE"
echo "    </div>" >> "$REPORT_FILE"

# Add performance metrics
echo "    <div class='test-section'>" >> "$REPORT_FILE"
echo "        <h2>Performance Metrics</h2>" >> "$REPORT_FILE"
echo "        <table>" >> "$REPORT_FILE"
echo "            <tr><th>Metric</th><th>Value</th></tr>" >> "$REPORT_FILE"

# Extract performance data from test output
grep "average execution time" /tmp/test-output.txt | while read line; do
    echo "            <tr><td>$line</td></tr>" >> "$REPORT_FILE"
done

echo "        </table>" >> "$REPORT_FILE"
echo "    </div>" >> "$REPORT_FILE"

cat >> "$REPORT_FILE" << 'EOF'
</body>
</html>
EOF

echo "Test report generated: $REPORT_FILE"

# Cleanup
rm -f /tmp/test-output.txt
```

## Troubleshooting

### Common Issues

#### 1. Runner Registration Failures

**Symptoms:**
- Runner not appearing in server
- Registration API returns errors

**Diagnosis:**
```bash
# Check server health
curl http://localhost:8080/health

# Check server logs
docker logs rustci-server

# Verify network connectivity
ping localhost
telnet localhost 8080
```

**Solutions:**
```bash
# Restart server
cargo run

# Check firewall settings
sudo ufw status

# Verify port availability
netstat -an | grep 8080
```

#### 2. Job Execution Failures

**Symptoms:**
- Jobs stuck in pending state
- Jobs fail immediately
- No job output

**Diagnosis:**
```bash
# Check runner status
curl http://localhost:8080/api/runners/{runner_id}/status

# Check runner health
curl http://localhost:8080/api/runners/{runner_id}/health

# Check job logs
curl http://localhost:8080/api/jobs/{job_id}/logs
```

**Solutions:**
```bash
# Restart runner
docker restart rustci-runner

# Check Docker daemon
docker info

# Verify runner configuration
docker exec rustci-runner cat /config/runner.yaml
```

#### 3. DIND Issues

**Symptoms:**
- DIND container won't start
- Docker commands fail inside DIND
- Network connectivity issues

**Diagnosis:**
```bash
# Check DIND container status
docker ps | grep dind

# Check DIND logs
docker logs rustci-dind

# Test Docker inside DIND
docker exec rustci-dind docker info
```

**Solutions:**
```bash
# Restart DIND environment
./scripts/runners/setup-dind-environment.sh restart

# Check Docker daemon configuration
docker exec rustci-dind cat /etc/docker/daemon.json

# Verify privileged mode
docker inspect rustci-dind | grep Privileged
```

### Debug Commands

```bash
# Server debugging
curl -v http://localhost:8080/health
curl http://localhost:8080/api/runners | jq '.'

# Runner debugging
docker logs rustci-runner
docker exec rustci-runner ps aux
docker exec rustci-runner netstat -tlnp

# DIND debugging
docker exec rustci-dind docker version
docker exec rustci-dind docker ps -a
docker exec rustci-dind docker system df

# Network debugging
docker network ls
docker network inspect rustci-network
```

### Performance Debugging

```bash
# Monitor resource usage
docker stats

# Check system resources
free -h
df -h
iostat 1

# Profile job execution
time curl -X POST http://localhost:8080/api/jobs \
  -H "Content-Type: application/json" \
  -d '{"name": "profile-test", "steps": [{"name": "test", "run": "echo test"}]}'
```

This comprehensive testing guide provides all the tools and procedures needed to thoroughly test RustCI runners in various scenarios and environments.