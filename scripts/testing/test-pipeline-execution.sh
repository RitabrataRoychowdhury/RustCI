#!/bin/bash

# RustCI Pipeline Execution Test Script
# This script demonstrates the complete pipeline lifecycle

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
BASE_URL="http://localhost:8000"
PIPELINE_YAML="pipeline.yaml"

# Function to print colored output
print_status() {
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

# Function to make authenticated requests
make_request() {
    local method=$1
    local endpoint=$2
    local data=$3
    local content_type=${4:-"application/json"}
    
    if [ -n "$data" ]; then
        curl -s -X "$method" \
             -H "Authorization: Bearer $JWT_TOKEN" \
             -H "Content-Type: $content_type" \
             -d "$data" \
             "$BASE_URL$endpoint"
    else
        curl -s -X "$method" \
             -H "Authorization: Bearer $JWT_TOKEN" \
             "$BASE_URL$endpoint"
    fi
}

# Function to upload file
upload_file() {
    local endpoint=$1
    local file_path=$2
    
    curl -s -X POST \
         -H "Authorization: Bearer $JWT_TOKEN" \
         -F "file=@$file_path" \
         "$BASE_URL$endpoint"
}

print_status "🚀 Starting RustCI Pipeline Execution Test"

# Step 1: Health Check
print_status "1. Checking server health..."
HEALTH_RESPONSE=$(curl -s "$BASE_URL/health")
if echo "$HEALTH_RESPONSE" | grep -q '"status":"healthy"'; then
    print_success "Server is healthy"
else
    print_error "Server health check failed"
    echo "$HEALTH_RESPONSE"
    exit 1
fi

# Step 2: Authentication (Manual - requires browser)
print_warning "2. Authentication required:"
print_status "Please visit: $BASE_URL/api/sessions/oauth/github"
print_status "After authentication, you'll get a JWT token."
print_status "Please enter your JWT token:"
read -r JWT_TOKEN

if [ -z "$JWT_TOKEN" ]; then
    print_error "JWT token is required"
    exit 1
fi

# Step 3: Verify authentication
print_status "3. Verifying authentication..."
USER_INFO=$(make_request GET "/api/sessions/me")
if echo "$USER_INFO" | grep -q '"id"'; then
    print_success "Authentication verified"
    echo "User: $(echo "$USER_INFO" | jq -r '.name // .email // "Unknown"')"
else
    print_error "Authentication failed"
    echo "$USER_INFO"
    exit 1
fi

# Step 4: Register a runner
print_status "4. Registering a test runner..."
RUNNER_DATA='{
    "name": "test-local-runner",
    "runner_type": "local",
    "capacity": 2,
    "tags": ["rust", "linux", "test"],
    "metadata": {
        "region": "local",
        "environment": "test"
    }
}'

RUNNER_RESPONSE=$(make_request POST "/api/runners" "$RUNNER_DATA")
if echo "$RUNNER_RESPONSE" | grep -q '"id"'; then
    RUNNER_ID=$(echo "$RUNNER_RESPONSE" | jq -r '.id')
    print_success "Runner registered: $RUNNER_ID"
else
    print_error "Runner registration failed"
    echo "$RUNNER_RESPONSE"
    exit 1
fi

# Step 5: Create pipeline from YAML file
print_status "5. Creating pipeline from YAML file..."
if [ ! -f "$PIPELINE_YAML" ]; then
    print_error "Pipeline YAML file not found: $PIPELINE_YAML"
    exit 1
fi

PIPELINE_RESPONSE=$(upload_file "/api/ci/pipelines/upload" "$PIPELINE_YAML")
if echo "$PIPELINE_RESPONSE" | grep -q '"id"'; then
    PIPELINE_ID=$(echo "$PIPELINE_RESPONSE" | jq -r '.id')
    print_success "Pipeline created: $PIPELINE_ID"
    echo "Pipeline name: $(echo "$PIPELINE_RESPONSE" | jq -r '.name')"
else
    print_error "Pipeline creation failed"
    echo "$PIPELINE_RESPONSE"
    exit 1
fi

# Step 6: Alternative - Create pipeline from JSON
print_status "6. Creating pipeline from JSON (alternative method)..."
YAML_CONTENT=$(cat "$PIPELINE_YAML")
PIPELINE_JSON_DATA=$(jq -n --arg yaml "$YAML_CONTENT" '{yaml_content: $yaml}')

PIPELINE_JSON_RESPONSE=$(make_request POST "/api/ci/pipelines" "$PIPELINE_JSON_DATA")
if echo "$PIPELINE_JSON_RESPONSE" | grep -q '"id"'; then
    PIPELINE_JSON_ID=$(echo "$PIPELINE_JSON_RESPONSE" | jq -r '.id')
    print_success "Pipeline created from JSON: $PIPELINE_JSON_ID"
else
    print_warning "Pipeline creation from JSON failed (this is expected if YAML parsing isn't implemented)"
fi

# Step 7: List all pipelines
print_status "7. Listing all pipelines..."
PIPELINES_LIST=$(make_request GET "/api/ci/pipelines")
PIPELINE_COUNT=$(echo "$PIPELINES_LIST" | jq '. | length')
print_success "Found $PIPELINE_COUNT pipelines"

# Step 8: Get pipeline YAML
print_status "8. Retrieving pipeline YAML..."
PIPELINE_YAML_RESPONSE=$(make_request GET "/api/ci/pipelines/$PIPELINE_ID/yaml")
if [ ${#PIPELINE_YAML_RESPONSE} -gt 10 ]; then
    print_success "Pipeline YAML retrieved (${#PIPELINE_YAML_RESPONSE} characters)"
else
    print_warning "Pipeline YAML retrieval failed or returned empty"
fi

# Step 9: Trigger pipeline execution
print_status "9. Triggering pipeline execution..."
TRIGGER_DATA='{
    "trigger_type": "manual",
    "branch": "main",
    "commit_hash": "abc123def456",
    "repository": "test/repo",
    "environment": {
        "BUILD_ENV": "test",
        "DEBUG": "true"
    }
}'

EXECUTION_RESPONSE=$(make_request POST "/api/ci/pipelines/$PIPELINE_ID/trigger" "$TRIGGER_DATA")
if echo "$EXECUTION_RESPONSE" | grep -q '"execution_id"'; then
    EXECUTION_ID=$(echo "$EXECUTION_RESPONSE" | jq -r '.execution_id')
    print_success "Pipeline execution triggered: $EXECUTION_ID"
else
    print_error "Pipeline execution failed"
    echo "$EXECUTION_RESPONSE"
    exit 1
fi

# Step 10: Monitor execution status
print_status "10. Monitoring execution status..."
for i in {1..10}; do
    EXECUTION_STATUS=$(make_request GET "/api/ci/executions/$EXECUTION_ID")
    STATUS=$(echo "$EXECUTION_STATUS" | jq -r '.status // "unknown"')
    print_status "Execution status: $STATUS (check $i/10)"
    
    if [ "$STATUS" = "completed" ] || [ "$STATUS" = "failed" ]; then
        break
    fi
    
    sleep 2
done

# Step 11: List all executions
print_status "11. Listing all executions..."
EXECUTIONS_LIST=$(make_request GET "/api/ci/executions")
EXECUTION_COUNT=$(echo "$EXECUTIONS_LIST" | jq '. | length')
print_success "Found $EXECUTION_COUNT executions"

# Step 12: List executions for specific pipeline
print_status "12. Listing executions for pipeline $PIPELINE_ID..."
PIPELINE_EXECUTIONS=$(make_request GET "/api/ci/executions?pipeline_id=$PIPELINE_ID")
PIPELINE_EXECUTION_COUNT=$(echo "$PIPELINE_EXECUTIONS" | jq '. | length')
print_success "Found $PIPELINE_EXECUTION_COUNT executions for this pipeline"

# Step 13: Trigger job on runner
print_status "13. Triggering job on runner..."
JOB_DATA='{
    "name": "test-build-job",
    "steps": [
        {
            "name": "setup",
            "command": "echo",
            "args": ["Setting up build environment"],
            "timeout": 30
        },
        {
            "name": "build",
            "command": "echo",
            "args": ["Building application"],
            "timeout": 60
        },
        {
            "name": "test",
            "command": "echo",
            "args": ["Running tests"],
            "timeout": 120
        }
    ],
    "priority": "normal",
    "timeout": 300,
    "metadata": {
        "pipeline_id": "'$PIPELINE_ID'",
        "execution_id": "'$EXECUTION_ID'"
    }
}'

JOB_RESPONSE=$(make_request POST "/api/runners/$RUNNER_ID/jobs" "$JOB_DATA")
if echo "$JOB_RESPONSE" | grep -q '"id"'; then
    JOB_ID=$(echo "$JOB_RESPONSE" | jq -r '.id')
    print_success "Job triggered on runner: $JOB_ID"
else
    print_error "Job trigger failed"
    echo "$JOB_RESPONSE"
fi

# Step 14: Fetch job logs
if [ -n "$JOB_ID" ]; then
    print_status "14. Fetching job logs..."
    JOB_LOGS=$(make_request GET "/api/runners/$RUNNER_ID/jobs/$JOB_ID/logs")
    print_success "Job logs retrieved:"
    echo "$JOB_LOGS"
    
    # Step 15: Fetch job logs with parameters
    print_status "15. Fetching job logs with tail parameter..."
    JOB_LOGS_TAIL=$(make_request GET "/api/runners/$RUNNER_ID/jobs/$JOB_ID/logs?tail=50")
    print_success "Job logs (tail 50) retrieved"
    
    # Step 16: Try to fetch job artifacts
    print_status "16. Attempting to fetch job artifacts..."
    JOB_ARTIFACTS=$(make_request GET "/api/runners/$RUNNER_ID/jobs/$JOB_ID/artifacts")
    if echo "$JOB_ARTIFACTS" | grep -q '"message"'; then
        print_warning "No artifacts found (expected for test job)"
    else
        print_success "Job artifacts retrieved"
    fi
fi

# Step 17: Get runner status
print_status "17. Getting runner status..."
RUNNER_STATUS=$(make_request GET "/api/runners/$RUNNER_ID/status")
if echo "$RUNNER_STATUS" | grep -q '"status"'; then
    STATUS=$(echo "$RUNNER_STATUS" | jq -r '.status')
    print_success "Runner status: $STATUS"
else
    print_warning "Runner status retrieval failed"
fi

# Step 18: List all runners
print_status "18. Listing all runners..."
RUNNERS_LIST=$(make_request GET "/api/runners")
RUNNER_COUNT=$(echo "$RUNNERS_LIST" | jq '. | length')
print_success "Found $RUNNER_COUNT runners"

# Step 19: Cancel execution (if still running)
if [ "$STATUS" = "running" ]; then
    print_status "19. Cancelling execution..."
    CANCEL_RESPONSE=$(make_request DELETE "/api/ci/executions/$EXECUTION_ID/cancel")
    if echo "$CANCEL_RESPONSE" | grep -q '"message"'; then
        print_success "Execution cancelled"
    else
        print_warning "Execution cancellation failed"
    fi
fi

# Step 20: Webhook test (optional)
print_status "20. Testing webhook trigger..."
WEBHOOK_DATA='{
    "ref": "refs/heads/main",
    "after": "def456abc789",
    "repository": {
        "full_name": "test/webhook-repo"
    },
    "pusher": {
        "name": "test-user"
    }
}'

WEBHOOK_RESPONSE=$(make_request POST "/api/ci/pipelines/$PIPELINE_ID/webhook" "$WEBHOOK_DATA")
if echo "$WEBHOOK_RESPONSE" | grep -q '"execution_id"'; then
    WEBHOOK_EXECUTION_ID=$(echo "$WEBHOOK_RESPONSE" | jq -r '.execution_id')
    print_success "Webhook triggered execution: $WEBHOOK_EXECUTION_ID"
else
    print_warning "Webhook trigger failed (may not be implemented)"
fi

# Step 21: Cleanup - Deregister runner
print_status "21. Cleaning up - deregistering runner..."
DEREGISTER_RESPONSE=$(make_request DELETE "/api/runners/$RUNNER_ID")
if echo "$DEREGISTER_RESPONSE" | grep -q '"message"'; then
    print_success "Runner deregistered successfully"
else
    print_warning "Runner deregistration failed"
fi

print_success "🎉 Pipeline execution test completed!"
print_status ""
print_status "Summary:"
print_status "- Pipeline ID: $PIPELINE_ID"
print_status "- Execution ID: $EXECUTION_ID"
print_status "- Runner ID: $RUNNER_ID"
if [ -n "$JOB_ID" ]; then
    print_status "- Job ID: $JOB_ID"
fi
print_status ""
print_status "You can access the Swagger UI at: $BASE_URL/swagger-ui"
print_status "OpenAPI spec at: $BASE_URL/api-docs/openapi.json"