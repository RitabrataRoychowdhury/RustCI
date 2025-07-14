# CI/CD System Testing Commands

## 🚀 **Prerequisites**

1. **Start the server**:
```bash
cargo run
```

2. **Authenticate with GitHub** (get access token):
```bash
# Visit in browser to get GitHub token
curl http://localhost:8000/api/sessions/oauth/google
# Follow GitHub OAuth flow to get token
```

## 📋 **1. Health Check**

```bash
curl -X GET http://localhost:8000/api/healthchecker
```

## 🔧 **2. Create Pipeline**

### **Simple Node.js Pipeline**
```bash
curl -X POST http://localhost:8000/api/ci/pipelines \
  -H "Content-Type: application/json" \
  -d '{
    "yaml_content": "name: \"Node.js App\"\ndescription: \"Build and deploy Node.js application\"\ntriggers:\n  - trigger_type: manual\n    config: {}\nstages:\n  - name: \"Source\"\n    steps:\n      - name: \"clone-repository\"\n        step_type: \"github\"\n        config:\n          repository_url: \"https://github.com/RitabrataRoychowdhury/sample-nodejs-app\"\n          branch: \"main\"\n  - name: \"Build\"\n    steps:\n      - name: \"build-app\"\n        step_type: \"shell\"\n        config:\n          command: \"npm ci && npm run build\"\n  - name: \"Deploy\"\n    steps:\n      - name: \"deploy-local\"\n        step_type: \"custom\"\n        config:\n          plugin_name: \"local-deploy\"\nenvironment:\n  NODE_ENV: \"production\""
  }'
```

### **Python FastAPI Pipeline**
```bash
curl -X POST http://localhost:8000/api/ci/pipelines \
  -H "Content-Type: application/json" \
  -d '{
    "yaml_content": "name: \"Python API\"\ndescription: \"Build and deploy Python FastAPI\"\ntriggers:\n  - trigger_type: manual\n    config: {}\nstages:\n  - name: \"Source\"\n    steps:\n      - name: \"clone-repository\"\n        step_type: \"github\"\n        config:\n          repository_url: \"https://github.com/RitabrataRoychowdhury/sample-python-api\"\n          branch: \"main\"\n  - name: \"Build\"\n    steps:\n      - name: \"build-python\"\n        step_type: \"shell\"\n        config:\n          script: \"python -m venv venv\\nsource venv/bin/activate\\npip install -r requirements.txt\"\n  - name: \"Deploy\"\n    steps:\n      - name: \"deploy-docker\"\n        step_type: \"docker\"\n        config:\n          image: \"python-api\"\n          environment:\n            PORTS: \"8000:8000\"\nenvironment:\n  PYTHON_ENV: \"production\""
  }'
```

### **Rust Application Pipeline**
```bash
curl -X POST http://localhost:8000/api/ci/pipelines \
  -H "Content-Type: application/json" \
  -d '{
    "yaml_content": "name: \"Rust Service\"\ndescription: \"Build and deploy Rust microservice\"\ntriggers:\n  - trigger_type: manual\n    config: {}\nstages:\n  - name: \"Source\"\n    steps:\n      - name: \"clone-repository\"\n        step_type: \"github\"\n        config:\n          repository_url: \"https://github.com/RitabrataRoychowdhury/RustCI\"\n          branch: \"main\"\n  - name: \"Build\"\n    steps:\n      - name: \"build-rust\"\n        step_type: \"shell\"\n        config:\n          command: \"cargo build --release\"\n  - name: \"Deploy\"\n    steps:\n      - name: \"deploy-hybrid\"\n        step_type: \"custom\"\n        config:\n          plugin_name: \"hybrid-deploy\"\n          environment:\n            PORTS: \"8080:8080\"\n            DISTROLESS: \"true\"\nenvironment:\n  RUST_ENV: \"production\""
  }'
```

## 🚀 **3. Trigger Pipeline**

### **Manual Trigger (Basic)**
```bash
curl -X POST http://localhost:8000/api/ci/pipelines/{PIPELINE_ID}/trigger \
  -H "Content-Type: application/json" \
  -d '{
    "trigger_type": "manual",
    "environment": {
      "GITHUB_TOKEN": "YOUR_GITHUB_TOKEN_HERE"
    }
  }'
```

### **Manual Trigger with Repository Access**
```bash
curl -X POST http://localhost:8000/api/ci/pipelines/{PIPELINE_ID}/trigger \
  -H "Content-Type: application/json" \
  -d '{
    "trigger_type": "manual",
    "branch": "main",
    "repository": "https://github.com/RitabrataRoychowdhury/RustCI",
    "environment": {
      "GITHUB_TOKEN": "ghp_your_token_here",
      "PORTS": "3000:3000,8080:8080",
      "NODE_ENV": "production",
      "DISTROLESS": "true"
    }
  }'
```

### **Trigger with Specific Commit**
```bash
curl -X POST http://localhost:8000/api/ci/pipelines/{PIPELINE_ID}/trigger \
  -H "Content-Type: application/json" \
  -d '{
    "trigger_type": "manual",
    "branch": "main",
    "commit_hash": "abc123def456",
    "repository": "https://github.com/RitabrataRoychowdhury/RustCI",
    "environment": {
      "GITHUB_TOKEN": "ghp_your_token_here",
      "DEPLOYMENT_TYPE": "hybrid",
      "CACHE_ENABLED": "true"
    }
  }'
```

## 📊 **4. Monitor Execution**

### **Get Execution Status**
```bash
curl -X GET http://localhost:8000/api/ci/executions/{EXECUTION_ID}
```

### **List All Executions**
```bash
curl -X GET http://localhost:8000/api/ci/executions
```

### **List Executions for Specific Pipeline**
```bash
curl -X GET "http://localhost:8000/api/ci/executions?pipeline_id={PIPELINE_ID}"
```

## 🛑 **5. Cancel Execution**

```bash
curl -X DELETE http://localhost:8000/api/ci/executions/{EXECUTION_ID}/cancel
```

## 📋 **6. Pipeline Management**

### **List All Pipelines**
```bash
curl -X GET http://localhost:8000/api/ci/pipelines
```

### **Get Pipeline YAML**
```bash
curl -X GET http://localhost:8000/api/ci/pipelines/{PIPELINE_ID}/yaml
```

## 🪝 **7. Webhook Trigger**

### **GitHub Webhook Simulation**
```bash
curl -X POST http://localhost:8000/api/ci/pipelines/{PIPELINE_ID}/webhook \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "refs/heads/main",
    "after": "abc123def456789",
    "repository": {
      "full_name": "RitabrataRoychowdhury/RustCI",
      "clone_url": "https://github.com/RitabrataRoychowdhury/RustCI.git"
    },
    "pusher": {
      "name": "RitabrataRoychowdhury"
    },
    "commits": [
      {
        "id": "abc123def456789",
        "message": "Add new feature",
        "author": {
          "name": "Ritabrata Roychowdhury"
        }
      }
    ]
  }'
```

## 🧪 **8. Test Endpoints**

### **CI Engine Test**
```bash
curl -X GET http://localhost:8000/api/ci/test
```

### **Authentication Test**
```bash
curl -X GET http://localhost:8000/api/sessions/me \
  -H "Authorization: Bearer YOUR_JWT_TOKEN"
```

## 🔧 **9. Complete Workflow Example**

### **Step 1: Create Pipeline**
```bash
PIPELINE_RESPONSE=$(curl -s -X POST http://localhost:8000/api/ci/pipelines \
  -H "Content-Type: application/json" \
  -d '{
    "yaml_content": "name: \"Test App\"\nstages:\n  - name: \"Deploy\"\n    steps:\n      - name: \"deploy-test\"\n        step_type: \"shell\"\n        config:\n          command: \"echo Hello CI/CD\""
  }')

PIPELINE_ID=$(echo $PIPELINE_RESPONSE | jq -r '.id')
echo "Created Pipeline ID: $PIPELINE_ID"
```

### **Step 2: Trigger Pipeline**
```bash
EXECUTION_RESPONSE=$(curl -s -X POST http://localhost:8000/api/ci/pipelines/$PIPELINE_ID/trigger \
  -H "Content-Type: application/json" \
  -d '{
    "trigger_type": "manual",
    "environment": {
      "TEST_VAR": "test_value"
    }
  }')

EXECUTION_ID=$(echo $EXECUTION_RESPONSE | jq -r '.execution_id')
echo "Started Execution ID: $EXECUTION_ID"
```

### **Step 3: Monitor Execution**
```bash
curl -s -X GET http://localhost:8000/api/ci/executions/$EXECUTION_ID | jq '.'
```

## 🔑 **Environment Variables for Testing**

```bash
# Required for GitHub integration
export GITHUB_TOKEN="ghp_your_personal_access_token"

# Optional deployment settings
export DEPLOYMENT_DIR="/tmp/ci-deployments"
export DOCKER_REGISTRY="localhost:5000"
export CACHE_ENABLED="true"
export PARALLEL_BUILDS="true"
```

## 📝 **Notes**

1. **Replace `{PIPELINE_ID}` and `{EXECUTION_ID}`** with actual IDs from responses
2. **Get GitHub token** from: https://github.com/settings/tokens
3. **Token permissions needed**: `repo`, `user:email`
4. **Server must be running** on `http://localhost:8000`
5. **Docker daemon** should be running for Docker deployments
6. **Check logs** with `RUST_LOG=debug cargo run` for detailed output

## 🚨 **Troubleshooting**

### **Common Issues**
- **401 Unauthorized**: Check GitHub token validity
- **404 Not Found**: Verify pipeline/execution IDs
- **500 Internal Error**: Check server logs for details
- **Docker errors**: Ensure Docker daemon is running

### **Debug Commands**
```bash
# Check server health
curl http://localhost:8000/api/healthchecker

# Verify authentication
curl http://localhost:8000/api/sessions/oauth/google

# Test CI engine
curl http://localhost:8000/api/ci/test
```