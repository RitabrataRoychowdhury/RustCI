# Service Deployment with RustCI System

Your CI system has advanced deployment capabilities! Here are the different ways to deploy and run services:

## 🚀 **Current Capabilities**

Your CI system supports:
- ✅ **Local directory deployment**
- ✅ **Docker container deployment** 
- ✅ **Local service management** (process spawning)
- ✅ **Port management and allocation**
- ✅ **Service health monitoring**
- ✅ **Hybrid deployments** (both directory + container)

## 🔧 **Option 1: Use Built-in Service Deployment**

Instead of manual SSH, use the CI system's service deployment:

```bash
curl --location 'http://localhost:8000/api/ci/pipelines' \
--header 'Content-Type: application/json' \
--data-raw '{
    "yaml_content": "name: \"RustCI Service Deploy\"\ndescription: \"Deploy RustCI as a managed service\"\ntriggers:\n  - trigger_type: manual\n    config: {}\nstages:\n  - name: \"Source\"\n    steps:\n      - name: \"clone-repository\"\n        step_type: github\n        config:\n          repository_url: \"https://github.com/RitabrataRoychowdhury/RustCI.git\"\n          branch: \"main\"\n  - name: \"Build\"\n    steps:\n      - name: \"build-rust\"\n        step_type: shell\n        config:\n          command: \"cargo build --release\"\n  - name: \"Deploy\"\n    steps:\n      - name: \"deploy-service\"\n        step_type: custom\n        config:\n          plugin_name: \"local-service\"\nenvironment:\n  RUST_ENV: \"production\"\n  PORTS: \"8000:8000\"\n  MONGODB_URI: \"mongodb://localhost:27017\"\n  JWT_SECRET: \"your-jwt-secret\"\ntimeout: 3600\nretry_count: 0"
}'
```

## 🐳 **Option 2: Docker Container Deployment**

Deploy as a Docker container with proper service management:

```bash
curl --location 'http://localhost:8000/api/ci/pipelines' \
--header 'Content-Type: application/json' \
--data-raw '{
    "yaml_content": "name: \"RustCI Docker Deploy\"\ndescription: \"Deploy RustCI in Docker container\"\ntriggers:\n  - trigger_type: manual\n    config: {}\nstages:\n  - name: \"Source\"\n    steps:\n      - name: \"clone-repository\"\n        step_type: github\n        config:\n          repository_url: \"https://github.com/RitabrataRoychowdhury/RustCI.git\"\n          branch: \"main\"\n  - name: \"Build\"\n    steps:\n      - name: \"build-rust\"\n        step_type: shell\n        config:\n          command: \"cargo build --release\"\n  - name: \"Deploy\"\n    steps:\n      - name: \"deploy-docker\"\n        step_type: docker\n        config:\n          image: \"rustci-app\"\n          dockerfile: \"Dockerfile\"\nenvironment:\n  PORTS: \"8000:8000\"\n  DISTROLESS: \"true\"\n  MONGODB_URI: \"mongodb://host.docker.internal:27017\"\n  JWT_SECRET: \"your-jwt-secret\"\ntimeout: 3600\nretry_count: 0"
}'
```

## 🔄 **Option 3: Hybrid Deployment**

Deploy both as directory and service:

```bash
curl --location 'http://localhost:8000/api/ci/pipelines' \
--header 'Content-Type: application/json' \
--data-raw '{
    "yaml_content": "name: \"RustCI Hybrid Deploy\"\ndescription: \"Deploy RustCI with hybrid approach\"\ntriggers:\n  - trigger_type: manual\n    config: {}\nstages:\n  - name: \"Source\"\n    steps:\n      - name: \"clone-repository\"\n        step_type: github\n        config:\n          repository_url: \"https://github.com/RitabrataRoychowdhury/RustCI.git\"\n          branch: \"main\"\n  - name: \"Build\"\n    steps:\n      - name: \"build-rust\"\n        step_type: shell\n        config:\n          command: \"cargo build --release\"\n  - name: \"Deploy\"\n    steps:\n      - name: \"deploy-hybrid\"\n        step_type: custom\n        config:\n          deployment_type: \"hybrid\"\nenvironment:\n  PORTS: \"8000:8000,8001:8001\"\n  RUST_ENV: \"production\"\ntimeout: 3600\nretry_count: 0"
}'
```

## 🛠️ **Option 4: Enhanced SSH Deployment with Service Management**

If you prefer SSH deployment but want proper service management:

```bash
curl --location 'http://localhost:8000/api/ci/pipelines' \
--header 'Content-Type: application/json' \
--data-raw '{
    "yaml_content": "name: \"RustCI SSH Service Deploy\"\ndescription: \"Deploy RustCI via SSH with proper service management\"\ntriggers:\n  - trigger_type: manual\n    config: {}\nstages:\n  - name: \"Build and Deploy\"\n    steps:\n      - name: \"fetch-code\"\n        step_type: shell\n        config:\n          command: \"rm -rf /tmp/rustci && git clone https://github.com/RitabrataRoychowdhury/RustCI.git /tmp/rustci\"\n      - name: \"build-rustci\"\n        step_type: shell\n        config:\n          command: \"cd /tmp/rustci && cargo build --release\"\n      - name: \"stop-existing-service\"\n        step_type: shell\n        config:\n          command: \"sshpass -p abc123 ssh -o StrictHostKeyChecking=no -p 2222 user@localhost '\''pkill -f RustAutoDevOps || true'\''\"\n      - name: \"upload-binary\"\n        step_type: shell\n        config:\n          command: \"sshpass -p abc123 scp -P 2222 /tmp/rustci/target/release/RustAutoDevOps user@localhost:/home/user/\"\n      - name: \"create-service-script\"\n        step_type: shell\n        config:\n          command: \"sshpass -p abc123 ssh -o StrictHostKeyChecking=no -p 2222 user@localhost '\''cat > /home/user/start-rustci.sh << EOF\\n#!/bin/bash\\nexport MONGODB_URI=\\\"mongodb://localhost:27017\\\"\\nexport JWT_SECRET=\\\"your-jwt-secret\\\"\\nexport PORT=8000\\ncd /home/user\\n./RustAutoDevOps > rustci.log 2>&1 &\\necho \\$! > rustci.pid\\nEOF'\''\"\n      - name: \"make-executable\"\n        step_type: shell\n        config:\n          command: \"sshpass -p abc123 ssh -o StrictHostKeyChecking=no -p 2222 user@localhost '\''chmod +x /home/user/start-rustci.sh'\''\"\n      - name: \"start-service\"\n        step_type: shell\n        config:\n          command: \"sshpass -p abc123 ssh -o StrictHostKeyChecking=no -p 2222 user@localhost '\''./start-rustci.sh'\''\"\n      - name: \"verify-service\"\n        step_type: shell\n        config:\n          command: \"sleep 5 && sshpass -p abc123 ssh -o StrictHostKeyChecking=no -p 2222 user@localhost '\''ps aux | grep RustAutoDevOps | grep -v grep'\''\"\nenvironment: {}\ntimeout: 3600\nretry_count: 0"
}'
```

## 📊 **Service Management Features**

Your CI system provides:

### **Automatic Port Allocation**
```yaml
environment:
  PORTS: "0:8000"  # Auto-allocate host port
```

### **Service Health Monitoring**
```yaml
health_check:
  endpoint: "/api/healthchecker"
  timeout_seconds: 30
  retries: 3
```

### **Process Management**
- ✅ **Process spawning** with proper PID tracking
- ✅ **Service status monitoring**
- ✅ **Automatic restart** on failure
- ✅ **Log collection** and management

### **Container Management**
- ✅ **Docker image building**
- ✅ **Container lifecycle management**
- ✅ **Port mapping** and networking
- ✅ **Volume mounting** for persistence

## 🔍 **Monitor Your Deployments**

After deployment, you can monitor services:

```bash
# Check deployment status
curl -X GET http://localhost:8000/api/ci/executions/{EXECUTION_ID}

# List all running services (if using built-in deployment)
curl -X GET http://localhost:8000/api/ci/services

# Check service health
curl -X GET http://localhost:8000/api/healthchecker
```

## 🎯 **Recommended Approach**

For production deployment, I recommend **Option 2 (Docker)** or **Option 1 (Built-in Service)** because:

- ✅ **Better isolation** and security
- ✅ **Automatic service management**
- ✅ **Health monitoring** built-in
- ✅ **Port management** handled automatically
- ✅ **Easier scaling** and updates
- ✅ **Proper logging** and monitoring

Your CI system is much more powerful than just binary uploads - it's a full deployment platform! 🚀