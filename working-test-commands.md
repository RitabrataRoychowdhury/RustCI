# Working CI/CD System Test Commands

## 🎯 **Your Pipeline ID**: `b78ca684-9fbc-42bc-a11f-22e9796102bd`

## 🚀 **1. Trigger Your Pipeline**

### **Basic Manual Trigger**
```bash
curl -X POST http://localhost:8000/api/ci/pipelines/b78ca684-9fbc-42bc-a11f-22e9796102bd/trigger \
  -H "Content-Type: application/json" \
  -d '{
    "trigger_type": "manual"
  }'
```

### **Trigger with Environment Variables**
```bash
curl -X POST http://localhost:8000/api/ci/pipelines/b78ca684-9fbc-42bc-a11f-22e9796102bd/trigger \
  -H "Content-Type: application/json" \
  -d '{
    "trigger_type": "manual",
    "environment": {
      "TEST_VAR": "hello_world",
      "NODE_ENV": "production"
    }
  }'
```

### **Trigger with GitHub Repository**
```bash
curl -X POST http://localhost:8000/api/ci/pipelines/b78ca684-9fbc-42bc-a11f-22e9796102bd/trigger \
  -H "Content-Type: application/json" \
  -d '{
    "trigger_type": "manual",
    "branch": "main",
    "repository": "https://github.com/RitabrataRoychowdhury/RustCI",
    "environment": {
      "GITHUB_TOKEN": "YOUR_GITHUB_TOKEN_HERE",
      "DEPLOYMENT_TYPE": "local"
    }
  }'
```

## 📊 **2. Monitor Execution**

### **List All Executions**
```bash
curl -X GET http://localhost:8000/api/ci/executions
```

### **List Executions for Your Pipeline**
```bash
curl -X GET "http://localhost:8000/api/ci/executions?pipeline_id=b78ca684-9fbc-42bc-a11f-22e9796102bd"
```

### **Get Specific Execution Status** (Replace {EXECUTION_ID} with actual ID from trigger response)
```bash
curl -X GET http://localhost:8000/api/ci/executions/{EXECUTION_ID}
```

## 🛑 **3. Cancel Execution** (if needed)
```bash
curl -X DELETE http://localhost:8000/api/ci/executions/{EXECUTION_ID}/cancel
```

## 📋 **4. Pipeline Management**

### **List All Pipelines**
```bash
curl -X GET http://localhost:8000/api/ci/pipelines
```

### **Get Your Pipeline YAML**
```bash
curl -X GET http://localhost:8000/api/ci/pipelines/b78ca684-9fbc-42bc-a11f-22e9796102bd/yaml
```

## 🪝 **5. Webhook Trigger**
```bash
curl -X POST http://localhost:8000/api/ci/pipelines/b78ca684-9fbc-42bc-a11f-22e9796102bd/webhook \
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
    }
  }'
```

## 🔧 **6. Create More Advanced Pipelines**

### **Node.js Build Pipeline**
```bash
curl -X POST http://localhost:8000/api/ci/pipelines \
  -H "Content-Type: application/json" \
  -d '{
    "yaml_content": "name: \"Node.js App\"\ndescription: \"Build and deploy Node.js application\"\ntriggers:\n  - trigger_type: manual\n    config: {}\nstages:\n  - name: \"Source\"\n    steps:\n      - name: \"clone-repository\"\n        step_type: github\n        config:\n          repository_url: \"https://github.com/RitabrataRoychowdhury/RustCI\"\n          branch: \"main\"\n  - name: \"Build\"\n    steps:\n      - name: \"build-app\"\n        step_type: shell\n        config:\n          command: \"echo Building Node.js application...\"\n  - name: \"Deploy\"\n    steps:\n      - name: \"deploy-local\"\n        step_type: custom\n        config:\n          plugin_name: \"local-deploy\"\nenvironment:\n  NODE_ENV: \"production\"\ntimeout: 3600\nretry_count: 0"
  }'
```

### **Docker Deployment Pipeline**
```bash
curl -X POST http://localhost:8000/api/ci/pipelines \
  -H "Content-Type: application/json" \
  -d '{
    "yaml_content": "name: \"Docker App\"\ndescription: \"Build and deploy with Docker\"\ntriggers:\n  - trigger_type: manual\n    config: {}\nstages:\n  - name: \"Source\"\n    steps:\n      - name: \"clone-repository\"\n        step_type: github\n        config:\n          repository_url: \"https://github.com/RitabrataRoychowdhury/RustCI\"\n          branch: \"main\"\n  - name: \"Build\"\n    steps:\n      - name: \"build-rust\"\n        step_type: shell\n        config:\n          command: \"cargo build --release\"\n  - name: \"Deploy\"\n    steps:\n      - name: \"deploy-docker\"\n        step_type: docker\n        config:\n          image: \"rust-app\"\n          dockerfile: \"Dockerfile\"\nenvironment:\n  RUST_ENV: \"production\"\n  PORTS: \"8080:8080\"\ntimeout: 3600\nretry_count: 1"
  }'
```

### **Python FastAPI Pipeline**
```bash
curl -X POST http://localhost:8000/api/ci/pipelines \
  -H "Content-Type: application/json" \
  -d '{
    "yaml_content": "name: \"Python API\"\ndescription: \"Build and deploy Python FastAPI\"\ntriggers:\n  - trigger_type: manual\n    config: {}\nstages:\n  - name: \"Source\"\n    steps:\n      - name: \"clone-repository\"\n        step_type: github\n        config:\n          repository_url: \"https://github.com/RitabrataRoychowdhury/sample-python-api\"\n          branch: \"main\"\n  - name: \"Build\"\n    steps:\n      - name: \"build-python\"\n        step_type: shell\n        config:\n          script: \"python -m venv venv && source venv/bin/activate && pip install -r requirements.txt\"\n  - name: \"Deploy\"\n    steps:\n      - name: \"deploy-local\"\n        step_type: custom\n        config:\n          plugin_name: \"local-service\"\nenvironment:\n  PYTHON_ENV: \"production\"\n  PORTS: \"8000:8000\"\ntimeout: 3600\nretry_count: 0"
  }'
```

## 🧪 **7. Test Endpoints**

### **Health Check**
```bash
curl -X GET http://localhost:8000/api/healthchecker
```

### **CI Engine Test**
```bash
curl -X GET http://localhost:8000/api/ci/test
```

### **Authentication Test** (if you have a JWT token)
```bash
curl -X GET http://localhost:8000/api/sessions/me \
  -H "Authorization: Bearer YOUR_JWT_TOKEN"
```

## 🔄 **8. Complete Workflow Example**

### **Step 1: Trigger Your Existing Pipeline**
```bash
EXECUTION_RESPONSE=$(curl -s -X POST http://localhost:8000/api/ci/pipelines/b78ca684-9fbc-42bc-a11f-22e9796102bd/trigger \
  -H "Content-Type: application/json" \
  -d '{
    "trigger_type": "manual",
    "environment": {
      "TEST_VAR": "deployment_test"
    }
  }')

echo "Response: $EXECUTION_RESPONSE"
EXECUTION_ID=$(echo $EXECUTION_RESPONSE | jq -r '.execution_id')
echo "Execution ID: $EXECUTION_ID"
```

### **Step 2: Monitor the Execution**
```bash
# Replace {EXECUTION_ID} with the actual ID from step 1
curl -s -X GET http://localhost:8000/api/ci/executions/{EXECUTION_ID} | jq '.'
```

### **Step 3: Check All Executions**
```bash
curl -s -X GET http://localhost:8000/api/ci/executions | jq '.'
```

## 🔑 **Environment Variables You Can Use**

```bash
# For GitHub integration
"GITHUB_TOKEN": "ghp_your_personal_access_token"

# For deployment settings
"DEPLOYMENT_DIR": "/tmp/ci-deployments"
"PORTS": "3000:3000,8080:8080"
"NODE_ENV": "production"
"PYTHON_ENV": "production"
"RUST_ENV": "production"
"DISTROLESS": "true"
"CACHE_ENABLED": "true"
```

## 📝 **Next Steps**

1. **Start with the basic trigger** to test your pipeline
2. **Check the execution status** to see if it completes successfully
3. **Create more complex pipelines** with GitHub cloning and Docker deployment
4. **Test with your actual repositories** using GitHub tokens

Your CI/CD system is ready for real deployment testing! 🚀