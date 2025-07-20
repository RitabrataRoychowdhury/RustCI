# Pipeline Examples

This directory contains comprehensive examples of RustCI pipelines for various use cases and deployment scenarios.

## Quick Examples

### Hello World Pipeline

```yaml
name: "Hello World"
description: "Simple introduction to RustCI"

triggers:
  - trigger_type: manual
    config: {}

stages:
  - name: "Greet"
    steps:
      - name: "hello"
        step_type: shell
        config:
          command: "echo 'Hello from RustCI!'"

environment: {}
timeout: 300
retry_count: 0
```

### Multi-Stage Build Pipeline

```yaml
name: "Multi-Stage Build"
description: "Build, test, and deploy application"

triggers:
  - trigger_type: git_push
    config:
      branch_patterns: ["main", "develop"]

stages:
  - name: "Build"
    steps:
      - name: "compile"
        step_type: shell
        config:
          command: "cargo build --release"

  - name: "Test"
    parallel: true
    steps:
      - name: "unit-tests"
        step_type: shell
        config:
          command: "cargo test"
      
      - name: "integration-tests"
        step_type: shell
        config:
          command: "cargo test --test integration"

  - name: "Deploy"
    condition: "${BRANCH} == 'main'"
    steps:
      - name: "deploy-production"
        step_type: docker
        config:
          image: "my-app:${BUILD_TAG}"
          command: "deploy.sh production"

environment:
  BUILD_TAG: "${CI_COMMIT_SHORT_SHA}"
  RUST_LOG: "info"

timeout: 1800
retry_count: 1
```

## Docker Examples

### Node.js Application

```yaml
name: "Node.js Docker Build"
description: "Build and deploy Node.js application with Docker"

triggers:
  - trigger_type: manual
    config: {}

stages:
  - name: "Build"
    steps:
      - name: "install-dependencies"
        step_type: docker
        config:
          image: "node:18-alpine"
          command: "npm ci"
          working_directory: "/app"
        timeout: 300

      - name: "build-application"
        step_type: docker
        config:
          image: "node:18-alpine"
          script: |
            npm run build
            npm run test
          working_directory: "/app"
          environment:
            NODE_ENV: "production"
        timeout: 600

  - name: "Package"
    steps:
      - name: "build-docker-image"
        step_type: docker
        config:
          dockerfile: "Dockerfile"
          image: "my-node-app:latest"
          build_context: "."
        timeout: 300

environment:
  NODE_ENV: "production"
  PORT: "3000"

timeout: 1200
retry_count: 1
```

### Python Application

```yaml
name: "Python Docker Pipeline"
description: "Build Python application with Docker"

stages:
  - name: "Test"
    steps:
      - name: "run-tests"
        step_type: docker
        config:
          image: "python:3.11-slim"
          script: |
            pip install -r requirements.txt
            python -m pytest tests/
            flake8 src/
          working_directory: "/app"
        timeout: 600

  - name: "Build"
    steps:
      - name: "build-image"
        step_type: docker
        config:
          dockerfile: "Dockerfile"
          image: "my-python-app:${VERSION}"
          build_context: "."
          environment:
            PYTHONPATH: "/app"
        timeout: 300

environment:
  VERSION: "1.0.0"
  PYTHON_ENV: "production"
```

### Rust Application

```yaml
name: "Rust Docker Build"
description: "Build Rust application with multi-stage Docker"

stages:
  - name: "Build"
    steps:
      - name: "cargo-build"
        step_type: docker
        config:
          image: "rust:1.70"
          script: |
            cargo build --release
            cargo test --release
            strip target/release/my-app
          working_directory: "/workspace"
          environment:
            CARGO_TARGET_DIR: "/workspace/target"
        timeout: 900

  - name: "Package"
    steps:
      - name: "create-image"
        step_type: docker
        config:
          dockerfile: "Dockerfile.multistage"
          image: "my-rust-app:${BUILD_TAG}"
          build_context: "."
        timeout: 300

environment:
  BUILD_TAG: "${CI_COMMIT_SHORT_SHA}"
  RUST_LOG: "info"
```

## Kubernetes Examples

### Basic Kubernetes Job

```yaml
name: "Kubernetes Job Example"
description: "Simple Kubernetes job execution"

triggers:
  - trigger_type: manual
    config: {}

stages:
  - name: "Deploy"
    steps:
      - name: "run-job"
        step_type: kubernetes
        config:
          image: "alpine:latest"
          command: "echo 'Running in Kubernetes' && sleep 30"
          namespace: "default"
        timeout: 120

environment:
  KUBECONFIG: "/etc/kubernetes/config"
```

### Advanced Kubernetes Deployment

```yaml
name: "Advanced Kubernetes Deployment"
description: "Production-ready Kubernetes deployment with resources"

stages:
  - name: "Build"
    steps:
      - name: "build-image"
        step_type: docker
        config:
          dockerfile: "Dockerfile"
          image: "my-app:${BUILD_TAG}"
          build_context: "."

  - name: "Deploy to Staging"
    steps:
      - name: "deploy-staging"
        step_type: kubernetes
        config:
          image: "my-app:${BUILD_TAG}"
          script: |
            kubectl apply -f k8s/staging/
            kubectl set image deployment/my-app my-app=my-app:${BUILD_TAG}
            kubectl rollout status deployment/my-app --timeout=300s
          namespace: "staging"
          parameters:
            resource_requests:
              cpu: "100m"
              memory: "128Mi"
            resource_limits:
              cpu: "500m"
              memory: "512Mi"
            service_account: "deployment-sa"
        timeout: 600

  - name: "Deploy to Production"
    condition: "${BRANCH} == 'main'"
    steps:
      - name: "deploy-production"
        step_type: kubernetes
        config:
          image: "kubectl:latest"
          script: |
            kubectl apply -f k8s/production/
            kubectl set image deployment/my-app my-app=my-app:${BUILD_TAG}
            kubectl rollout status deployment/my-app --timeout=600s
            kubectl get pods -l app=my-app
          namespace: "production"
          parameters:
            use_pvc: true
            storage_class: "fast-ssd"
            storage_size: "20Gi"
            resource_requests:
              cpu: "200m"
              memory: "256Mi"
            resource_limits:
              cpu: "1"
              memory: "1Gi"
            service_account: "production-sa"
        timeout: 900

environment:
  BUILD_TAG: "${CI_COMMIT_SHORT_SHA}"
  KUBECTL_VERSION: "1.28"
```

### Kubernetes with PVC

```yaml
name: "Kubernetes with Persistent Storage"
description: "Job with persistent volume claim"

stages:
  - name: "Process Data"
    steps:
      - name: "data-processing"
        step_type: kubernetes
        config:
          image: "python:3.11"
          script: |
            echo "Processing data with persistent storage"
            python process_data.py --input /data/input --output /data/output
            echo "Data processing completed"
          namespace: "data-processing"
          parameters:
            use_pvc: true
            storage_class: "standard"
            storage_size: "50Gi"
            resource_requests:
              cpu: "500m"
              memory: "1Gi"
            resource_limits:
              cpu: "2"
              memory: "4Gi"
        timeout: 1800

environment:
  DATA_PATH: "/data"
  PROCESSING_MODE: "batch"
```

## Multi-Cloud Examples

### AWS ECS Deployment

```yaml
name: "AWS ECS Deployment"
description: "Deploy to Amazon ECS"

stages:
  - name: "Build"
    steps:
      - name: "build-and-push"
        step_type: docker
        config:
          dockerfile: "Dockerfile"
          image: "my-ecr-repo.amazonaws.com/my-app:${BUILD_TAG}"
          script: |
            docker build -t my-app:${BUILD_TAG} .
            aws ecr get-login-password --region us-west-2 | docker login --username AWS --password-stdin my-ecr-repo.amazonaws.com
            docker tag my-app:${BUILD_TAG} my-ecr-repo.amazonaws.com/my-app:${BUILD_TAG}
            docker push my-ecr-repo.amazonaws.com/my-app:${BUILD_TAG}

  - name: "Deploy"
    steps:
      - name: "update-ecs-service"
        step_type: aws
        config:
          service: "ecs"
          action: "update-service"
          parameters:
            cluster: "production"
            service: "my-app-service"
            task_definition: "my-app:${BUILD_TAG}"
            desired_count: 3

environment:
  BUILD_TAG: "${CI_COMMIT_SHORT_SHA}"
  AWS_REGION: "us-west-2"
```

### Azure Container Instances

```yaml
name: "Azure Container Deployment"
description: "Deploy to Azure Container Instances"

stages:
  - name: "Deploy"
    steps:
      - name: "create-container-group"
        step_type: azure
        config:
          service: "container-instances"
          action: "create-or-update"
          parameters:
            resource_group: "production-rg"
            container_group: "my-app-group"
            image: "myregistry.azurecr.io/my-app:${BUILD_TAG}"
            cpu: 2
            memory: 4
            ports: [80, 443]
            environment_variables:
              NODE_ENV: "production"
              PORT: "80"

environment:
  BUILD_TAG: "${CI_COMMIT_SHORT_SHA}"
  AZURE_SUBSCRIPTION: "your-subscription-id"
```

### Google Cloud Run

```yaml
name: "Google Cloud Run Deployment"
description: "Deploy to Google Cloud Run"

stages:
  - name: "Build and Deploy"
    steps:
      - name: "deploy-cloud-run"
        step_type: gcp
        config:
          service: "cloud-run"
          action: "deploy"
          parameters:
            service_name: "my-app-service"
            image: "gcr.io/my-project/my-app:${BUILD_TAG}"
            region: "us-central1"
            platform: "managed"
            allow_unauthenticated: true
            memory: "1Gi"
            cpu: "1000m"
            max_instances: 10
            environment_variables:
              NODE_ENV: "production"

environment:
  BUILD_TAG: "${CI_COMMIT_SHORT_SHA}"
  GCP_PROJECT: "my-project-id"
```

## Git Integration Examples

### GitHub Release Pipeline

```yaml
name: "GitHub Release Pipeline"
description: "Create GitHub release with artifacts"

triggers:
  - trigger_type: git_tag
    config:
      tag_patterns: ["v*"]

stages:
  - name: "Build"
    steps:
      - name: "build-artifacts"
        step_type: shell
        config:
          script: |
            cargo build --release
            tar -czf my-app-${TAG}.tar.gz -C target/release my-app
            sha256sum my-app-${TAG}.tar.gz > my-app-${TAG}.tar.gz.sha256

  - name: "Release"
    steps:
      - name: "create-github-release"
        step_type: github
        config:
          action: "create-release"
          repository: "user/my-app"
          tag: "${TAG}"
          name: "Release ${TAG}"
          body: |
            ## Changes in ${TAG}
            
            - Automated release from CI/CD pipeline
            - Built from commit ${CI_COMMIT_SHA}
          draft: false
          prerelease: false
          assets:
            - "my-app-${TAG}.tar.gz"
            - "my-app-${TAG}.tar.gz.sha256"

environment:
  TAG: "${CI_TAG}"
  GITHUB_TOKEN: "${GITHUB_TOKEN}"
```

### Multi-Repository Sync

```yaml
name: "Repository Sync"
description: "Sync code between GitHub and GitLab"

triggers:
  - trigger_type: git_push
    config:
      branch_patterns: ["main"]

stages:
  - name: "Sync"
    steps:
      - name: "sync-to-gitlab"
        step_type: gitlab
        config:
          action: "mirror"
          source_repo: "https://github.com/user/my-app"
          target_repo: "https://gitlab.com/user/my-app"
          branch: "main"
          force_push: false

      - name: "update-mirror-status"
        step_type: github
        config:
          action: "create-status"
          repository: "user/my-app"
          sha: "${CI_COMMIT_SHA}"
          state: "success"
          description: "Successfully mirrored to GitLab"
          context: "ci/mirror"

environment:
  GITHUB_TOKEN: "${GITHUB_TOKEN}"
  GITLAB_TOKEN: "${GITLAB_TOKEN}"
```

## Complex Workflow Examples

### Microservices Deployment

```yaml
name: "Microservices Deployment"
description: "Deploy multiple microservices"

stages:
  - name: "Build Services"
    parallel: true
    steps:
      - name: "build-api-service"
        step_type: docker
        config:
          dockerfile: "services/api/Dockerfile"
          image: "my-api:${BUILD_TAG}"
          build_context: "services/api"

      - name: "build-worker-service"
        step_type: docker
        config:
          dockerfile: "services/worker/Dockerfile"
          image: "my-worker:${BUILD_TAG}"
          build_context: "services/worker"

      - name: "build-frontend"
        step_type: docker
        config:
          dockerfile: "frontend/Dockerfile"
          image: "my-frontend:${BUILD_TAG}"
          build_context: "frontend"

  - name: "Deploy to Staging"
    steps:
      - name: "deploy-all-services"
        step_type: kubernetes
        config:
          image: "kubectl:latest"
          script: |
            # Update image tags in manifests
            sed -i "s/{{BUILD_TAG}}/${BUILD_TAG}/g" k8s/staging/*.yaml
            
            # Apply all manifests
            kubectl apply -f k8s/staging/
            
            # Wait for deployments
            kubectl rollout status deployment/api-service -n staging --timeout=300s
            kubectl rollout status deployment/worker-service -n staging --timeout=300s
            kubectl rollout status deployment/frontend -n staging --timeout=300s
            
            # Run health checks
            kubectl get pods -n staging
          namespace: "staging"
        timeout: 900

  - name: "Integration Tests"
    steps:
      - name: "run-e2e-tests"
        step_type: kubernetes
        config:
          image: "cypress/included:latest"
          script: |
            npm ci
            npm run test:e2e -- --config baseUrl=http://frontend.staging.svc.cluster.local
          namespace: "staging"
        timeout: 600

  - name: "Deploy to Production"
    condition: "${BRANCH} == 'main' && ${TESTS_PASSED} == 'true'"
    steps:
      - name: "production-deployment"
        step_type: kubernetes
        config:
          image: "kubectl:latest"
          script: |
            # Blue-green deployment strategy
            sed -i "s/{{BUILD_TAG}}/${BUILD_TAG}/g" k8s/production/*.yaml
            kubectl apply -f k8s/production/
            
            # Gradual rollout
            kubectl patch deployment api-service -p '{"spec":{"strategy":{"rollingUpdate":{"maxSurge":1,"maxUnavailable":0}}}}'
            kubectl rollout status deployment/api-service -n production --timeout=600s
            
            # Health check before proceeding
            kubectl exec -n production deployment/api-service -- curl -f http://localhost:8080/health
          namespace: "production"
        timeout: 1200

environment:
  BUILD_TAG: "${CI_COMMIT_SHORT_SHA}"
  TESTS_PASSED: "false"  # Set by previous stages
```

### Database Migration Pipeline

```yaml
name: "Database Migration Pipeline"
description: "Handle database migrations with rollback capability"

stages:
  - name: "Backup Database"
    steps:
      - name: "create-backup"
        step_type: kubernetes
        config:
          image: "postgres:15"
          script: |
            pg_dump -h $DB_HOST -U $DB_USER -d $DB_NAME > /backup/backup-$(date +%Y%m%d-%H%M%S).sql
            echo "Backup created successfully"
          namespace: "database"
          parameters:
            use_pvc: true
            storage_size: "10Gi"
          environment:
            DB_HOST: "postgres.database.svc.cluster.local"
            DB_USER: "migration_user"
            DB_NAME: "production_db"
        timeout: 600

  - name: "Run Migrations"
    steps:
      - name: "apply-migrations"
        step_type: kubernetes
        config:
          image: "migrate/migrate:latest"
          script: |
            migrate -path /migrations -database $DATABASE_URL up
            echo "Migrations applied successfully"
          namespace: "database"
          environment:
            DATABASE_URL: "postgres://user:pass@postgres.database.svc.cluster.local/production_db?sslmode=disable"
        timeout: 300

  - name: "Verify Migration"
    steps:
      - name: "run-verification"
        step_type: kubernetes
        config:
          image: "postgres:15"
          script: |
            psql $DATABASE_URL -c "SELECT version();"
            psql $DATABASE_URL -c "SELECT * FROM schema_migrations ORDER BY version DESC LIMIT 5;"
            echo "Migration verification completed"
          namespace: "database"
        timeout: 120

environment:
  DATABASE_URL: "postgres://user:pass@postgres.database.svc.cluster.local/production_db?sslmode=disable"
  MIGRATION_PATH: "/migrations"
```

## Webhook Integration Examples

### GitHub Webhook Pipeline

```yaml
name: "GitHub Webhook Pipeline"
description: "Triggered by GitHub push events"

triggers:
  - trigger_type: webhook
    config:
      webhook_url: "/webhook/github-push"

stages:
  - name: "Validate Push"
    steps:
      - name: "check-branch"
        step_type: shell
        config:
          script: |
            if [ "$CI_BRANCH" != "main" ] && [ "$CI_BRANCH" != "develop" ]; then
              echo "Skipping deployment for branch: $CI_BRANCH"
              exit 0
            fi
            echo "Processing push to $CI_BRANCH"

  - name: "Deploy"
    condition: "${CI_BRANCH} == 'main' || ${CI_BRANCH} == 'develop'"
    steps:
      - name: "deploy-branch"
        step_type: kubernetes
        config:
          image: "kubectl:latest"
          script: |
            NAMESPACE="production"
            if [ "$CI_BRANCH" = "develop" ]; then
              NAMESPACE="staging"
            fi
            
            echo "Deploying to $NAMESPACE"
            kubectl set image deployment/my-app my-app=my-app:${CI_COMMIT_SHA} -n $NAMESPACE
            kubectl rollout status deployment/my-app -n $NAMESPACE --timeout=300s
        timeout: 600

environment: {}
```

## Testing Examples

### Comprehensive Test Pipeline

```yaml
name: "Comprehensive Test Suite"
description: "Multi-stage testing pipeline"

stages:
  - name: "Unit Tests"
    parallel: true
    steps:
      - name: "backend-tests"
        step_type: docker
        config:
          image: "rust:1.70"
          command: "cargo test --lib"
          working_directory: "/workspace"
        timeout: 600

      - name: "frontend-tests"
        step_type: docker
        config:
          image: "node:18"
          command: "npm test -- --coverage"
          working_directory: "/workspace/frontend"
        timeout: 300

  - name: "Integration Tests"
    steps:
      - name: "api-integration-tests"
        step_type: docker
        config:
          image: "rust:1.70"
          script: |
            cargo test --test integration
            cargo test --test api_tests
          working_directory: "/workspace"
        timeout: 900

  - name: "End-to-End Tests"
    steps:
      - name: "e2e-tests"
        step_type: kubernetes
        config:
          image: "cypress/included:latest"
          script: |
            npm ci
            npm run test:e2e
          namespace: "testing"
          parameters:
            resource_requests:
              cpu: "500m"
              memory: "1Gi"
        timeout: 1200

  - name: "Performance Tests"
    steps:
      - name: "load-tests"
        step_type: kubernetes
        config:
          image: "loadimpact/k6:latest"
          script: |
            k6 run --vus 50 --duration 5m /scripts/load-test.js
          namespace: "testing"
        timeout: 600

environment:
  TEST_DATABASE_URL: "postgres://test:test@test-db:5432/test_db"
  API_BASE_URL: "http://api.testing.svc.cluster.local:8000"
```

These examples demonstrate the flexibility and power of RustCI's pipeline system. You can mix and match different connectors, deployment strategies, and configurations to build pipelines that fit your specific needs.

## Usage Tips

1. **Start Simple**: Begin with basic pipelines and gradually add complexity
2. **Use Environment Variables**: Keep sensitive data in environment variables
3. **Parallel Execution**: Use parallel stages to speed up builds
4. **Conditional Deployment**: Use conditions to control when deployments happen
5. **Resource Management**: Set appropriate timeouts and resource limits
6. **Error Handling**: Plan for failures with retry counts and proper error messages
7. **Testing**: Include comprehensive testing at multiple stages
8. **Monitoring**: Add health checks and monitoring to your deployments

For more specific examples, check the `examples/` directory in the repository root.