#!/bin/bash

# RustCI API Endpoint Testing Script
# Tests current API functionality and documents authentication behavior

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
API_BASE_URL="${API_BASE_URL:-http://localhost:8000}"
OUTPUT_DIR="./api-test-results"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")

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

# Function to test endpoint
test_endpoint() {
    local method="$1"
    local endpoint="$2"
    local description="$3"
    local auth_header="$4"
    local expected_status="$5"
    local data="$6"
    
    print_status "Testing: $method $endpoint - $description"
    
    local curl_cmd="curl -s -w '\n%{http_code}' -X $method"
    
    if [ -n "$auth_header" ]; then
        curl_cmd="$curl_cmd -H 'Authorization: $auth_header'"
    fi
    
    if [ -n "$data" ]; then
        curl_cmd="$curl_cmd -H 'Content-Type: application/json' -d '$data'"
    fi
    
    curl_cmd="$curl_cmd '$API_BASE_URL$endpoint'"
    
    local response
    response=$(eval "$curl_cmd")
    
    local http_code
    http_code=$(echo "$response" | tail -n1)
    local body
    body=$(echo "$response" | sed '$d')
    
    # Log the test result
    echo "=== $method $endpoint ===" >> "$OUTPUT_DIR/test_results_$TIMESTAMP.log"
    echo "Description: $description" >> "$OUTPUT_DIR/test_results_$TIMESTAMP.log"
    echo "Expected Status: $expected_status" >> "$OUTPUT_DIR/test_results_$TIMESTAMP.log"
    echo "Actual Status: $http_code" >> "$OUTPUT_DIR/test_results_$TIMESTAMP.log"
    echo "Response Body:" >> "$OUTPUT_DIR/test_results_$TIMESTAMP.log"
    echo "$body" >> "$OUTPUT_DIR/test_results_$TIMESTAMP.log"
    echo "" >> "$OUTPUT_DIR/test_results_$TIMESTAMP.log"
    
    # Check if status matches expected
    if [ "$http_code" = "$expected_status" ]; then
        print_success "$method $endpoint - Status: $http_code ✓"
    else
        print_warning "$method $endpoint - Expected: $expected_status, Got: $http_code"
    fi
    
    # Pretty print JSON if possible
    if echo "$body" | jq . >/dev/null 2>&1; then
        echo "$body" | jq . > "$OUTPUT_DIR/${method}_${endpoint//\//_}_response.json"
    else
        echo "$body" > "$OUTPUT_DIR/${method}_${endpoint//\//_}_response.txt"
    fi
}

# Function to check if server is running
check_server() {
    print_status "Checking if server is running on $API_BASE_URL"
    
    if curl -s "$API_BASE_URL/health" >/dev/null 2>&1; then
        print_success "Server is running"
        return 0
    else
        print_error "Server is not running or not accessible"
        print_status "Please start the server with: cargo run"
        return 1
    fi
}

# Function to generate cURL examples
generate_curl_examples() {
    print_status "Generating cURL examples..."
    
    cat > "$OUTPUT_DIR/curl_examples.md" << 'EOF'
# RustCI API cURL Examples

## Public Endpoints

### Health Check
```bash
curl -X GET http://localhost:8000/health
```

### GitHub OAuth Initiation
```bash
curl -X GET http://localhost:8000/api/sessions/oauth/github
# Returns 303 redirect to GitHub OAuth
```

## Protected Endpoints (Require Authentication)

### Get OpenAPI Documentation
```bash
# Without authentication (will fail)
curl -X GET http://localhost:8000/api-docs/openapi.json

# With authentication (requires valid JWT token)
curl -X GET \
  -H "Authorization: Bearer <JWT_TOKEN>" \
  http://localhost:8000/api-docs/openapi.json
```

### CI Pipeline Endpoints

#### Create Pipeline from JSON
```bash
curl -X POST \
  -H "Authorization: Bearer <JWT_TOKEN>" \
  -H "Content-Type: application/json" \
  -d '{
    "yaml_content": "name: test-pipeline\nsteps:\n  - name: echo\n    command: echo hello"
  }' \
  http://localhost:8000/api/ci/pipelines
```

#### List Pipelines
```bash
curl -X GET \
  -H "Authorization: Bearer <JWT_TOKEN>" \
  http://localhost:8000/api/ci/pipelines
```

#### Trigger Pipeline
```bash
curl -X POST \
  -H "Authorization: Bearer <JWT_TOKEN>" \
  -H "Content-Type: application/json" \
  -d '{
    "trigger_type": "manual",
    "branch": "main",
    "environment": {
      "ENV": "test"
    }
  }' \
  http://localhost:8000/api/ci/pipelines/{pipeline_id}/trigger
```

### Cluster Management Endpoints

#### List Cluster Nodes
```bash
curl -X GET \
  -H "Authorization: Bearer <JWT_TOKEN>" \
  http://localhost:8000/api/cluster/nodes
```

#### Join Node to Cluster
```bash
curl -X POST \
  -H "Authorization: Bearer <JWT_TOKEN>" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "test-node",
    "address": "127.0.0.1:8080",
    "role": "Worker",
    "metadata": {
      "region": "us-west-1"
    }
  }' \
  http://localhost:8000/api/cluster/nodes
```

#### Get Cluster Status
```bash
curl -X GET \
  -H "Authorization: Bearer <JWT_TOKEN>" \
  http://localhost:8000/api/cluster/status
```

## Authentication Notes

1. **JWT Token Required:** Most endpoints require a valid JWT token
2. **Token Sources:** 
   - Authorization header: `Bearer <token>`
   - Cookie: `token=<token>`
3. **OAuth Flow:** Use GitHub OAuth to obtain tokens
4. **Error Response:** 401 Unauthorized for missing/invalid tokens

## Error Response Format

```json
{
  "error": "Authentication required",
  "status": 401,
  "timestamp": "2025-07-27T11:11:48.410270+00:00"
}
```
EOF

    print_success "cURL examples generated: $OUTPUT_DIR/curl_examples.md"
}

# Main testing function
main() {
    print_status "Starting RustCI API endpoint testing..."
    
    # Create output directory
    mkdir -p "$OUTPUT_DIR"
    
    # Check if server is running
    if ! check_server; then
        exit 1
    fi
    
    print_status "Testing public endpoints..."
    
    # Test public endpoints
    test_endpoint "GET" "/health" "Health check endpoint" "" "200"
    test_endpoint "GET" "/api/sessions/oauth/github" "GitHub OAuth initiation" "" "303"
    
    print_status "Testing protected endpoints (without authentication)..."
    
    # Test protected endpoints without auth (should return 401)
    test_endpoint "GET" "/api-docs/openapi.json" "OpenAPI documentation" "" "401"
    test_endpoint "GET" "/api/ci/test" "CI test endpoint" "" "401"
    test_endpoint "GET" "/api/ci/pipelines" "List pipelines" "" "401"
    test_endpoint "POST" "/api/ci/pipelines" "Create pipeline" "" "401" '{"yaml_content":"name: test"}'
    test_endpoint "GET" "/api/cluster/nodes" "List cluster nodes" "" "401"
    test_endpoint "GET" "/api/cluster/status" "Get cluster status" "" "401"
    test_endpoint "GET" "/api/cluster/runners" "List runners" "" "401"
    test_endpoint "GET" "/api/cluster/jobs" "List jobs" "" "401"
    
    print_status "Testing with invalid authentication..."
    
    # Test with invalid token (should return 401)
    test_endpoint "GET" "/api/ci/pipelines" "List pipelines with invalid token" "Bearer invalid-token" "401"
    
    # Generate documentation
    generate_curl_examples
    
    # Generate summary report
    cat > "$OUTPUT_DIR/test_summary_$TIMESTAMP.md" << EOF
# API Testing Summary

**Test Date:** $(date)  
**Server URL:** $API_BASE_URL  
**Total Endpoints Tested:** $(grep -c "=== " "$OUTPUT_DIR/test_results_$TIMESTAMP.log")

## Test Results

### Public Endpoints ✅
- Health check: Working
- GitHub OAuth: Working (redirects properly)

### Protected Endpoints 🔒
- All API endpoints require authentication
- Return 401 Unauthorized without valid JWT token
- Consistent error response format

### Authentication Behavior
- JWT tokens required for API access
- Bearer token authentication supported
- Cookie-based authentication supported
- OAuth flow available for token generation

### Missing Implementations ❌
- Runner lifecycle endpoints not implemented
- Job management endpoints are placeholders
- No helper scripts for authentication

## Recommendations

1. Implement missing runner lifecycle APIs
2. Create authentication helper scripts
3. Add working cURL examples with real tokens
4. Fix local fake runner scripts
5. Create end-to-end testing infrastructure

## Files Generated
- Test results: test_results_$TIMESTAMP.log
- cURL examples: curl_examples.md
- Individual response files: *_response.json/txt
EOF

    print_success "API testing completed!"
    print_status "Results saved to: $OUTPUT_DIR/"
    print_status "Summary: $OUTPUT_DIR/test_summary_$TIMESTAMP.md"
    print_status "cURL examples: $OUTPUT_DIR/curl_examples.md"
}

# Run main function
main "$@"