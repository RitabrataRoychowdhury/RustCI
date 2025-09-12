# Universal YAML Compatibility Guide

**Zero learning curve - Use any CI/CD YAML format with RustCI**

## 🎯 Overview

RustCI provides **universal YAML compatibility** allowing DevOps teams to use their existing pipeline configurations without modification. No need to learn new syntax or migrate existing workflows.

## 📋 Supported Formats

### ✅ Fully Supported Formats

| Platform                | Support Level | Auto-Detection | Conversion |
| ----------------------- | ------------- | -------------- | ---------- |
| **GitHub Actions**      | 100%          | ✅             | ✅         |
| **GitLab CI**           | 100%          | ✅             | ✅         |
| **Jenkins Pipeline**    | 95%           | ✅             | ✅         |
| **Azure DevOps**        | 95%           | ✅             | ✅         |
| **CircleCI**            | 90%           | ✅             | ✅         |
| **Travis CI**           | 90%           | ✅             | ✅         |
| **Bitbucket Pipelines** | 85%           | ✅             | ✅         |
| **Native RustCI**       | 100%          | ✅             | N/A        |

## 🔄 Format Examples & Conversions

### GitHub Actions → RustCI

**Original GitHub Actions:**

```yaml
name: CI
on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

env:
  NODE_VERSION: "18"
  REGISTRY: ghcr.io

jobs:
  test:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        node-version: [16, 18, 20]
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: ${{ matrix.node-version }}
          cache: "npm"

      - name: Install dependencies
        run: npm ci

      - name: Run tests
        run: npm test

      - name: Upload coverage
        uses: codecov/codecov-action@v3
        if: matrix.node-version == '18'

  build:
    needs: test
    runs-on: ubuntu-latest
    if: github.ref == 'refs/heads/main'
    steps:
      - uses: actions/checkout@v4
      - name: Build Docker image
        run: |
          docker build -t ${{ env.REGISTRY }}/myapp:${{ github.sha }} .
          docker push ${{ env.REGISTRY }}/myapp:${{ github.sha }}
```

**Auto-converted to RustCI:**

```yaml
version: "1.0"
name: "CI"

triggers:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

variables:
  NODE_VERSION: "18"
  REGISTRY: "ghcr.io"

stages:
  - name: test
    runner: docker
    image: ubuntu:latest
    matrix:
      node-version: [16, 18, 20]
    steps:
      - name: Checkout
        action: checkout@v4

      - name: Setup Node.js
        action: setup-node@v4
        with:
          node-version: ${{ matrix.node-version }}
          cache: "npm"

      - name: Install dependencies
        run: npm ci

      - name: Run tests
        run: npm test

      - name: Upload coverage
        action: codecov@v3
        condition: matrix.node-version == '18'

  - name: build
    depends_on: [test]
    runner: docker
    condition: branch == 'main'
    steps:
      - action: checkout@v4
      - name: Build Docker image
        run: |
          docker build -t ${{ env.REGISTRY }}/myapp:${{ env.BUILD_SHA }} .
          docker push ${{ env.REGISTRY }}/myapp:${{ env.BUILD_SHA }}
```

### GitLab CI → RustCI

**Original GitLab CI:**

```yaml
stages:
  - build
  - test
  - security
  - deploy

variables:
  DOCKER_DRIVER: overlay2
  DOCKER_TLS_CERTDIR: "/certs"

.docker_template: &docker_template
  image: docker:20.10.16
  services:
    - docker:20.10.16-dind
  before_script:
    - docker login -u $CI_REGISTRY_USER -p $CI_REGISTRY_PASSWORD $CI_REGISTRY

build:
  <<: *docker_template
  stage: build
  script:
    - docker build -t $CI_REGISTRY_IMAGE:$CI_COMMIT_SHA .
    - docker push $CI_REGISTRY_IMAGE:$CI_COMMIT_SHA
  artifacts:
    reports:
      dotenv: build.env
  only:
    - main
    - merge_requests

test:unit:
  stage: test
  image: node:18
  cache:
    paths:
      - node_modules/
  script:
    - npm install
    - npm run test:unit
  coverage: '/Coverage: \d+\.\d+%/'
  artifacts:
    reports:
      junit: junit.xml
      coverage_report:
        coverage_format: cobertura
        path: coverage/cobertura-coverage.xml

test:integration:
  stage: test
  image: node:18
  services:
    - postgres:13
    - redis:6
  variables:
    POSTGRES_DB: testdb
    POSTGRES_USER: test
    POSTGRES_PASSWORD: test
  script:
    - npm install
    - npm run test:integration
  only:
    - main

security:
  stage: security
  image: securecodewarrior/docker-image-scanner
  script:
    - scan-image $CI_REGISTRY_IMAGE:$CI_COMMIT_SHA
  allow_failure: true

deploy:staging:
  stage: deploy
  image: alpine/helm:latest
  script:
    - helm upgrade --install myapp ./helm-chart
      --set image.tag=$CI_COMMIT_SHA
      --namespace staging
  environment:
    name: staging
    url: https://staging.myapp.com
  only:
    - main

deploy:production:
  stage: deploy
  image: alpine/helm:latest
  script:
    - helm upgrade --install myapp ./helm-chart
      --set image.tag=$CI_COMMIT_SHA
      --namespace production
  environment:
    name: production
    url: https://myapp.com
  when: manual
  only:
    - main
```

**Auto-converted to RustCI:**

```yaml
version: "1.0"
name: "GitLab CI Pipeline"

variables:
  DOCKER_DRIVER: overlay2
  DOCKER_TLS_CERTDIR: "/certs"

templates:
  docker_template:
    runner: docker
    image: docker:20.10.16
    services:
      - docker:20.10.16-dind
    before_script:
      - docker login -u $CI_REGISTRY_USER -p $CI_REGISTRY_PASSWORD $CI_REGISTRY

stages:
  - name: build
    extends: docker_template
    steps:
      - name: Build and push
        run: |
          docker build -t $CI_REGISTRY_IMAGE:$BUILD_SHA .
          docker push $CI_REGISTRY_IMAGE:$BUILD_SHA
    artifacts:
      - path: build.env
        type: dotenv
    condition: branch in ['main'] or is_merge_request

  - name: test_unit
    runner: docker
    image: node:18
    cache:
      paths: [node_modules/]
    steps:
      - name: Unit tests
        run: |
          npm install
          npm run test:unit
    artifacts:
      - path: junit.xml
        type: junit
      - path: coverage/cobertura-coverage.xml
        type: coverage

  - name: test_integration
    runner: docker
    image: node:18
    services:
      - postgres:13
      - redis:6
    variables:
      POSTGRES_DB: testdb
      POSTGRES_USER: test
      POSTGRES_PASSWORD: test
    steps:
      - name: Integration tests
        run: |
          npm install
          npm run test:integration
    condition: branch == 'main'

  - name: security
    runner: docker
    image: securecodewarrior/docker-image-scanner
    steps:
      - name: Security scan
        run: scan-image $CI_REGISTRY_IMAGE:$BUILD_SHA
        allow_failure: true

  - name: deploy_staging
    runner: kubernetes
    image: alpine/helm:latest
    steps:
      - name: Deploy to staging
        run: |
          helm upgrade --install myapp ./helm-chart \
            --set image.tag=$BUILD_SHA \
            --namespace staging
    environment:
      name: staging
      url: https://staging.myapp.com
    condition: branch == 'main'

  - name: deploy_production
    runner: kubernetes
    image: alpine/helm:latest
    steps:
      - name: Deploy to production
        run: |
          helm upgrade --install myapp ./helm-chart \
            --set image.tag=$BUILD_SHA \
            --namespace production
    environment:
      name: production
      url: https://myapp.com
    manual: true
    condition: branch == 'main'
```

### Jenkins Pipeline → RustCI

**Original Jenkinsfile:**

```groovy
pipeline {
    agent any

    environment {
        DOCKER_REGISTRY = 'registry.company.com'
        APP_NAME = 'myapp'
        KUBECONFIG = credentials('kubeconfig')
    }

    stages {
        stage('Checkout') {
            steps {
                checkout scm
            }
        }

        stage('Build') {
            parallel {
                stage('Backend') {
                    agent {
                        docker {
                            image 'maven:3.8-openjdk-17'
                            args '-v /root/.m2:/root/.m2'
                        }
                    }
                    steps {
                        sh 'mvn clean compile'
                        sh 'mvn package -DskipTests'
                    }
                    post {
                        always {
                            archiveArtifacts artifacts: 'target/*.jar'
                        }
                    }
                }

                stage('Frontend') {
                    agent {
                        docker {
                            image 'node:18'
                        }
                    }
                    steps {
                        sh 'npm install'
                        sh 'npm run build'
                    }
                    post {
                        always {
                            archiveArtifacts artifacts: 'dist/**'
                        }
                    }
                }
            }
        }

        stage('Test') {
            parallel {
                stage('Unit Tests') {
                    steps {
                        sh 'mvn test'
                    }
                    post {
                        always {
                            junit 'target/surefire-reports/*.xml'
                            publishHTML([
                                allowMissing: false,
                                alwaysLinkToLastBuild: true,
                                keepAll: true,
                                reportDir: 'target/site/jacoco',
                                reportFiles: 'index.html',
                                reportName: 'Coverage Report'
                            ])
                        }
                    }
                }

                stage('Integration Tests') {
                    steps {
                        sh 'mvn verify -Dskip.unit.tests=true'
                    }
                }
            }
        }

        stage('Docker Build') {
            when {
                branch 'main'
            }
            steps {
                script {
                    def image = docker.build("${DOCKER_REGISTRY}/${APP_NAME}:${BUILD_NUMBER}")
                    docker.withRegistry("https://${DOCKER_REGISTRY}", 'docker-registry-credentials') {
                        image.push()
                        image.push('latest')
                    }
                }
            }
        }

        stage('Deploy') {
            when {
                branch 'main'
            }
            steps {
                sh """
                    helm upgrade --install ${APP_NAME} ./helm-chart \\
                        --set image.tag=${BUILD_NUMBER} \\
                        --namespace production \\
                        --wait
                """
            }
        }
    }

    post {
        always {
            cleanWs()
        }
        success {
            slackSend(
                channel: '#deployments',
                color: 'good',
                message: "✅ Pipeline succeeded for ${env.JOB_NAME} - ${env.BUILD_NUMBER}"
            )
        }
        failure {
            slackSend(
                channel: '#deployments',
                color: 'danger',
                message: "❌ Pipeline failed for ${env.JOB_NAME} - ${env.BUILD_NUMBER}"
            )
        }
    }
}
```

**Auto-converted to RustCI:**

```yaml
version: "1.0"
name: "Jenkins Pipeline"

variables:
  DOCKER_REGISTRY: "registry.company.com"
  APP_NAME: "myapp"
  KUBECONFIG: "${KUBECONFIG_SECRET}"

stages:
  - name: checkout
    runner: any
    steps:
      - name: Checkout code
        action: checkout

  - name: build
    parallel: true
    substages:
      - name: backend
        runner: docker
        image: maven:3.8-openjdk-17
        volumes:
          - /root/.m2:/root/.m2
        steps:
          - name: Compile
            run: mvn clean compile
          - name: Package
            run: mvn package -DskipTests
        artifacts:
          - path: target/*.jar
            retention: 30d

      - name: frontend
        runner: docker
        image: node:18
        steps:
          - name: Install dependencies
            run: npm install
          - name: Build frontend
            run: npm run build
        artifacts:
          - path: dist/**
            retention: 30d

  - name: test
    depends_on: [build]
    parallel: true
    substages:
      - name: unit_tests
        runner: docker
        image: maven:3.8-openjdk-17
        steps:
          - name: Run unit tests
            run: mvn test
        artifacts:
          - path: target/surefire-reports/*.xml
            type: junit
          - path: target/site/jacoco/index.html
            type: coverage

      - name: integration_tests
        runner: docker
        image: maven:3.8-openjdk-17
        steps:
          - name: Run integration tests
            run: mvn verify -Dskip.unit.tests=true

  - name: docker_build
    depends_on: [test]
    condition: branch == 'main'
    runner: docker
    steps:
      - name: Build and push Docker image
        run: |
          docker build -t ${DOCKER_REGISTRY}/${APP_NAME}:${BUILD_NUMBER} .
          docker push ${DOCKER_REGISTRY}/${APP_NAME}:${BUILD_NUMBER}
          docker tag ${DOCKER_REGISTRY}/${APP_NAME}:${BUILD_NUMBER} ${DOCKER_REGISTRY}/${APP_NAME}:latest
          docker push ${DOCKER_REGISTRY}/${APP_NAME}:latest

  - name: deploy
    depends_on: [docker_build]
    condition: branch == 'main'
    runner: kubernetes
    steps:
      - name: Deploy to production
        run: |
          helm upgrade --install ${APP_NAME} ./helm-chart \
            --set image.tag=${BUILD_NUMBER} \
            --namespace production \
            --wait

notifications:
  slack:
    webhook: "${SLACK_WEBHOOK}"
    channel: "#deployments"
    on_success:
      message: "✅ Pipeline succeeded for ${PIPELINE_NAME} - ${BUILD_NUMBER}"
    on_failure:
      message: "❌ Pipeline failed for ${PIPELINE_NAME} - ${BUILD_NUMBER}"

cleanup:
  always: true
```

## 🔧 Advanced Features

### Variable Substitution

RustCI supports all common variable formats:

```yaml
# GitHub Actions style
- run: echo "Building version ${{ github.sha }}"

# GitLab CI style
- run: echo "Building version $CI_COMMIT_SHA"

# Jenkins style
- run: echo "Building version ${BUILD_NUMBER}"

# Azure DevOps style
- run: echo "Building version $(Build.BuildNumber)"

# Native RustCI style
- run: echo "Building version ${BUILD_SHA}"
```

### Conditional Execution

```yaml
# GitHub Actions conditions
if: github.ref == 'refs/heads/main'

# GitLab CI conditions
only:
  - main

# Jenkins conditions
when:
  branch: "main"

# Native RustCI conditions
condition: branch == 'main'
```

### Matrix Builds

```yaml
# GitHub Actions matrix
strategy:
  matrix:
    os: [ubuntu-latest, windows-latest, macos-latest]
    node: [16, 18, 20]

# GitLab CI parallel
parallel:
  matrix:
    - OS: [ubuntu, windows, macos]
      NODE: [16, 18, 20]

# Native RustCI matrix
matrix:
  os: [ubuntu-latest, windows-latest, macos-latest]
  node: [16, 18, 20]
```

## 🚀 Migration Tools

### Automatic Conversion API

```bash
# Convert GitHub Actions to RustCI
curl -X POST http://localhost:8080/api/v1/convert \
  -H "Content-Type: application/json" \
  -d '{
    "from": "github-actions",
    "to": "rustci",
    "yaml": "..."
  }'

# Batch convert multiple files
curl -X POST http://localhost:8080/api/v1/convert/batch \
  -F "files=@.github/workflows/ci.yml" \
  -F "files=@.github/workflows/deploy.yml" \
  -F "format=rustci"
```

### CLI Migration Tool

```bash
# Install RustCI CLI
curl -sSL https://get.rustci.dev | sh

# Convert single file
rustci convert --from github-actions --to rustci .github/workflows/ci.yml

# Convert entire directory
rustci convert --from gitlab-ci --to rustci --recursive .gitlab-ci/

# Validate converted files
rustci validate pipeline.yml
```

### Migration Scripts

```bash
#!/bin/bash
# migrate-to-rustci.sh

echo "🔄 Migrating CI/CD pipelines to RustCI..."

# Detect current CI system
if [ -d ".github/workflows" ]; then
    echo "📁 Found GitHub Actions workflows"
    rustci convert --from github-actions --to rustci .github/workflows/*.yml
elif [ -f ".gitlab-ci.yml" ]; then
    echo "📁 Found GitLab CI configuration"
    rustci convert --from gitlab-ci --to rustci .gitlab-ci.yml
elif [ -f "Jenkinsfile" ]; then
    echo "📁 Found Jenkins pipeline"
    rustci convert --from jenkins --to rustci Jenkinsfile
elif [ -f "azure-pipelines.yml" ]; then
    echo "📁 Found Azure DevOps pipeline"
    rustci convert --from azure-devops --to rustci azure-pipelines.yml
fi

echo "✅ Migration completed!"
echo "📝 Review the generated pipeline.yml file"
echo "🚀 Deploy with: rustci deploy pipeline.yml"
```

## 📊 Compatibility Matrix

### Feature Support

| Feature                   | GitHub Actions | GitLab CI | Jenkins | Azure DevOps | RustCI |
| ------------------------- | -------------- | --------- | ------- | ------------ | ------ |
| **Basic Stages**          | ✅             | ✅        | ✅      | ✅           | ✅     |
| **Parallel Jobs**         | ✅             | ✅        | ✅      | ✅           | ✅     |
| **Matrix Builds**         | ✅             | ✅        | ✅      | ✅           | ✅     |
| **Conditional Execution** | ✅             | ✅        | ✅      | ✅           | ✅     |
| **Artifacts**             | ✅             | ✅        | ✅      | ✅           | ✅     |
| **Caching**               | ✅             | ✅        | ✅      | ✅           | ✅     |
| **Secrets Management**    | ✅             | ✅        | ✅      | ✅           | ✅     |
| **Environment Variables** | ✅             | ✅        | ✅      | ✅           | ✅     |
| **Docker Support**        | ✅             | ✅        | ✅      | ✅           | ✅     |
| **Kubernetes Support**    | ⚠️             | ✅        | ✅      | ⚠️           | ✅     |
| **Custom Actions**        | ✅             | ⚠️        | ✅      | ✅           | ✅     |
| **Notifications**         | ⚠️             | ✅        | ✅      | ✅           | ✅     |

**Legend:**

- ✅ Full support
- ⚠️ Partial support
- ❌ Not supported

### Performance Comparison

| Platform           | Startup Time | Build Speed | Resource Usage | Scalability   |
| ------------------ | ------------ | ----------- | -------------- | ------------- |
| **GitHub Actions** | ~30s         | Medium      | Medium         | High          |
| **GitLab CI**      | ~20s         | Medium      | Medium         | High          |
| **Jenkins**        | ~45s         | Slow        | High           | Medium        |
| **Azure DevOps**   | ~25s         | Medium      | Medium         | High          |
| **RustCI**         | **~5s**      | **Fast**    | **Low**        | **Very High** |

## 🎯 Best Practices

### 1. Gradual Migration

```bash
# Start with a simple pipeline
rustci convert --from github-actions --simple .github/workflows/test.yml

# Gradually add complexity
rustci convert --from github-actions --advanced .github/workflows/deploy.yml
```

### 2. Validation Before Deployment

```bash
# Always validate converted pipelines
rustci validate pipeline.yml

# Run dry-run before actual deployment
rustci run --dry-run pipeline.yml
```

### 3. Maintain Compatibility

```yaml
# Keep original files during transition
# .github/workflows/ci.yml (original)
# pipeline.yml (RustCI version)

# Use feature flags for gradual rollout
stages:
  - name: test
    condition: env.USE_RUSTCI == 'true'
```

### 4. Team Training

```bash
# Generate documentation for your team
rustci docs generate --format=markdown --output=docs/

# Create training pipelines
rustci examples create --type=training
```

## 🔍 Troubleshooting

### Common Conversion Issues

#### Issue: Unsupported Action

```yaml
# Original (GitHub Actions)
- uses: some-custom-action@v1

# Solution: Convert to equivalent commands
- name: Custom action equivalent
  run: |
    # Equivalent shell commands
    echo "Performing custom action"
```

#### Issue: Complex Conditionals

```yaml
# Original (GitLab CI)
only:
  variables:
    - $CI_COMMIT_BRANCH == "main"
    - $DEPLOY_ENV == "production"

# Converted (RustCI)
condition: branch == 'main' and env.DEPLOY_ENV == 'production'
```

#### Issue: Platform-Specific Features

```yaml
# Original (Jenkins)
when {
  allOf {
    branch 'main'
    environment name: 'production'
  }
}

# Converted (RustCI)
condition: branch == 'main' and environment == 'production'
```

### Validation Errors

```bash
# Check syntax errors
rustci validate --strict pipeline.yml

# Check for missing dependencies
rustci validate --check-deps pipeline.yml

# Validate against schema
rustci validate --schema=v1.0 pipeline.yml
```

---

## 📞 Support

Need help with YAML conversion or compatibility issues?

- **Documentation**: https://docs.rustci.dev/yaml-compatibility
- **Conversion Tool**: https://convert.rustci.dev
- **Community Forum**: https://community.rustci.dev
- **Enterprise Support**: yaml-support@rustci.dev

**Zero learning curve guaranteed!** 🎯
