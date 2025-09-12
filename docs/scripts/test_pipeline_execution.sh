#!/bin/bash

# Test script to verify pipeline execution fix

echo "🧪 Testing Pipeline Execution Fix"
echo "=================================="

# Check if server is running
if ! curl -s http://localhost:8000/health > /dev/null; then
    echo "❌ Server is not running on localhost:8000"
    exit 1
fi

echo "✅ Server is running"

# Try to authenticate with GitHub OAuth (you'll need to do this manually in browser)
echo "🔐 Please authenticate via GitHub OAuth:"
echo "   Open: http://localhost:8000/api/sessions/oauth/github"
echo ""
echo "After authentication, copy the JWT token from your browser's developer tools"
echo "and set it as TOKEN environment variable:"
echo "   export TOKEN='your-jwt-token-here'"
echo ""

if [ -z "$TOKEN" ]; then
    echo "⚠️  No TOKEN environment variable set"
    echo "   Please authenticate first and set TOKEN variable"
    exit 1
fi

echo "✅ Token found, proceeding with test"

# Upload pipeline
echo "📤 Uploading pipeline..."
PIPELINE_RESPONSE=$(curl -s -X POST http://localhost:8000/api/ci/pipelines/upload \
  -H "Content-Type: multipart/form-data" \
  -H "Authorization: Bearer $TOKEN" \
  -F "file=@nodejs-deployment-pipeline.yaml")

PIPELINE_ID=$(echo "$PIPELINE_RESPONSE" | jq -r '.id // empty')

if [ -z "$PIPELINE_ID" ] || [ "$PIPELINE_ID" = "null" ]; then
    echo "❌ Failed to upload pipeline"
    echo "Response: $PIPELINE_RESPONSE"
    exit 1
fi

echo "✅ Pipeline uploaded successfully: $PIPELINE_ID"

# Trigger pipeline
echo "🚀 Triggering pipeline execution..."
TRIGGER_RESPONSE=$(curl -s -X POST "http://localhost:8000/api/ci/pipelines/$PIPELINE_ID/trigger" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "trigger_type": "manual",
    "branch": "main",
    "environment": {
      "TEST_MODE": "true"
    }
  }')

EXECUTION_ID=$(echo "$TRIGGER_RESPONSE" | jq -r '.execution_id // empty')

if [ -z "$EXECUTION_ID" ] || [ "$EXECUTION_ID" = "null" ]; then
    echo "❌ Failed to trigger pipeline"
    echo "Response: $TRIGGER_RESPONSE"
    exit 1
fi

echo "✅ Pipeline triggered successfully: $EXECUTION_ID"

# Wait a moment for execution
echo "⏳ Waiting for execution to complete..."
sleep 10

# Check execution status
echo "📊 Checking execution status..."
EXECUTION_RESPONSE=$(curl -s "http://localhost:8000/api/ci/executions/$EXECUTION_ID" \
  -H "Authorization: Bearer $TOKEN")

echo "📋 Execution Result:"
echo "$EXECUTION_RESPONSE" | jq .

# Extract key metrics
STATUS=$(echo "$EXECUTION_RESPONSE" | jq -r '.status // empty')
DURATION=$(echo "$EXECUTION_RESPONSE" | jq -r '.duration // empty')
LOGS_COUNT=$(echo "$EXECUTION_RESPONSE" | jq -r '.logs | length // 0')
STAGES_COUNT=$(echo "$EXECUTION_RESPONSE" | jq -r '.stages | length // 0')

echo ""
echo "🔍 Analysis:"
echo "   Status: $STATUS"
echo "   Duration: ${DURATION}ms"
echo "   Logs: $LOGS_COUNT entries"
echo "   Stages: $STAGES_COUNT processed"

if [ "$DURATION" -gt 1000 ]; then
    echo "✅ SUCCESS: Execution took realistic time (${DURATION}ms > 1000ms)"
else
    echo "❌ ISSUE: Execution completed too quickly (${DURATION}ms)"
fi

if [ "$LOGS_COUNT" -gt 0 ]; then
    echo "✅ SUCCESS: Execution produced logs"
else
    echo "❌ ISSUE: No execution logs found"
fi

if [ "$STAGES_COUNT" -gt 0 ]; then
    echo "✅ SUCCESS: Stages were processed"
else
    echo "❌ ISSUE: No stages were processed"
fi

echo ""
echo "🔍 Check server logs for detailed execution trace:"
echo "   tail -50 server.log | grep 'EXECUTOR DEBUG'"