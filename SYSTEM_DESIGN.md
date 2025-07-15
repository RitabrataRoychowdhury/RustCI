# RustCI System Design Documentation

This document provides a comprehensive overview of the RustCI system architecture, component interactions, and request flow patterns.

## System Overview

RustCI is a high-performance CI/CD platform built in Rust that provides pipeline execution, deployment management, and service orchestration capabilities. The system is designed for speed, reliability, and ease of use.

## Architecture Diagram

```mermaid
graph TB
    subgraph "Client Layer"
        CLI[CLI Tools]
        WEB[Web Interface]
        API_CLIENT[API Clients]
        WEBHOOK[Webhook Sources]
    end
    
    subgraph "API Gateway Layer"
        AXUM[Axum Web Server]
        AUTH[Authentication Middleware]
        CORS[CORS Middleware]
        RATE[Rate Limiting]
    end
    
    subgraph "Application Layer"
        ROUTES[Route Handlers]
        UPLOAD[File Upload Handler]
        PIPELINE[Pipeline Handler]
        EXEC[Execution Handler]
    end
    
    subgraph "Business Logic Layer"
        CI_ENGINE[CI Engine]
        PIPELINE_MGR[Pipeline Manager]
        EXEC_ENGINE[Execution Engine]
        DEPLOY_MGR[Deployment Manager]
        PROJECT_DETECT[Project Type Detector]
        SERVICE_REG[Service Registry]
    end
    
    subgraph "Infrastructure Layer"
        FILE_SYS[File System]
        DOCKER[Docker Integration]
        GIT[Git Operations]
        PORT_MGR[Port Manager]
        WORKSPACE[Workspace Manager]
    end
    
    subgraph "Data Layer"
        MONGODB[MongoDB Database]
        CACHE[Redis Cache]
        LOGS[Log Storage]
    end
    
    subgraph "External Services"
        GITHUB[GitHub API]
        DOCKER_HUB[Docker Registry]
        SSH_TARGETS[SSH Targets]
        AGENTS[Frontend Agents]
    end
    
    CLI --> AXUM
    WEB --> AXUM
    API_CLIENT --> AXUM
    WEBHOOK --> AXUM
    
    AXUM --> AUTH
    AUTH --> CORS
    CORS --> RATE
    RATE --> ROUTES
    
    ROUTES --> UPLOAD
    ROUTES --> PIPELINE
    ROUTES --> EXEC
    
    UPLOAD --> CI_ENGINE
    PIPELINE --> CI_ENGINE
    EXEC --> CI_ENGINE
    
    CI_ENGINE --> PIPELINE_MGR
    CI_ENGINE --> EXEC_ENGINE
    EXEC_ENGINE --> DEPLOY_MGR
    DEPLOY_MGR --> PROJECT_DETECT
    DEPLOY_MGR --> SERVICE_REG
    
    PIPELINE_MGR --> MONGODB
    EXEC_ENGINE --> MONGODB
    SERVICE_REG --> MONGODB
    
    DEPLOY_MGR --> FILE_SYS
    DEPLOY_MGR --> DOCKER
    DEPLOY_MGR --> GIT
    DEPLOY_MGR --> PORT_MGR
    DEPLOY_MGR --> WORKSPACE
    
    DOCKER --> DOCKER_HUB
    GIT --> GITHUB
    DEPLOY_MGR --> SSH_TARGETS
    SERVICE_REG --> AGENTS
```

## Component Descriptions

### API Gateway Layer

#### Axum Web Server
- **Purpose**: HTTP server handling all incoming requests
- **Technology**: Axum framework with Tokio async runtime
- **Responsibilities**:
  - Request routing
  - HTTP/1.1 and HTTP/2 support
  - WebSocket connections (future)
  - Static file serving

#### Authentication Middleware
- **Purpose**: JWT-based authentication and authorization
- **Features**:
  - Token validation
  - User session management
  - OAuth integration (GitHub)
  - Role-based access control

#### CORS Middleware
- **Purpose**: Cross-origin resource sharing configuration
- **Configuration**:
  - Allowed origins
  - Allowed methods
  - Credential handling

### Application Layer

#### Route Handlers
- **Pipeline Routes**: `/api/ci/pipelines/*`
- **Execution Routes**: `/api/ci/executions/*`
- **Authentication Routes**: `/api/sessions/*`
- **Health Check**: `/api/healthchecker`

#### File Upload Handler
- **Purpose**: Handle multipart file uploads for YAML configurations
- **Features**:
  - File size validation (10MB limit)
  - MIME type checking
  - Temporary file management
  - YAML content validation

### Business Logic Layer

#### CI Engine
- **Purpose**: Core orchestration engine for CI/CD operations
- **Responsibilities**:
  - Pipeline lifecycle management
  - Execution coordination
  - Resource allocation
  - Error handling and recovery

```rust
pub struct CIEngine {
    pipeline_manager: Arc<PipelineManager>,
    execution_engine: Arc<ExecutionEngine>,
    deployment_manager: Arc<DeploymentManager>,
    database: Arc<Database>,
}
```

#### Pipeline Manager
- **Purpose**: Manage pipeline definitions and configurations
- **Operations**:
  - CRUD operations for pipelines
  - YAML parsing and validation
  - Pipeline versioning
  - Trigger management

#### Execution Engine
- **Purpose**: Execute pipeline stages and steps
- **Features**:
  - Parallel stage execution
  - Step dependency resolution
  - Real-time status updates
  - Log aggregation
  - Timeout handling

#### Deployment Manager
- **Purpose**: Handle various deployment strategies
- **Deployment Types**:
  - Local directory deployment
  - Docker container deployment
  - Local service deployment
  - Hybrid deployments

#### Project Type Detector
- **Purpose**: Automatically detect project types for deployment
- **Detection Rules**:
  - File pattern matching
  - Directory structure analysis
  - Content-based detection
  - Confidence scoring

#### Service Registry
- **Purpose**: Track and manage deployed services
- **Features**:
  - Service registration
  - Health monitoring
  - Agent communication preparation
  - Service discovery

### Infrastructure Layer

#### File System Manager
- **Purpose**: Manage workspace and artifact storage
- **Operations**:
  - Workspace creation and cleanup
  - Artifact copying and archiving
  - Temporary file management
  - Permission handling

#### Docker Integration
- **Purpose**: Docker container lifecycle management
- **Features**:
  - Image building
  - Container execution
  - Port mapping
  - Volume management
  - Registry operations

#### Git Operations
- **Purpose**: Source code management
- **Features**:
  - Repository cloning
  - Branch switching
  - Commit tracking
  - Authentication handling

## Request Flow Patterns

### Pipeline Creation Flow

```mermaid
sequenceDiagram
    participant Client
    participant API
    participant Handler
    participant Engine
    participant Manager
    participant DB
    
    Client->>API: POST /ci/pipelines
    API->>Handler: Route to create_pipeline
    Handler->>Handler: Validate YAML
    Handler->>Engine: Create pipeline
    Engine->>Manager: Store pipeline
    Manager->>DB: Insert pipeline record
    DB-->>Manager: Confirm insertion
    Manager-->>Engine: Return pipeline ID
    Engine-->>Handler: Return pipeline object
    Handler-->>API: JSON response
    API-->>Client: Pipeline created response
```

### Pipeline Execution Flow

```mermaid
sequenceDiagram
    participant Client
    participant API
    participant Handler
    participant Engine
    participant Executor
    participant Deployer
    
    Client->>API: POST /ci/pipelines/{id}/trigger
    API->>Handler: Route to trigger_pipeline
    Handler->>Engine: Trigger execution
    Engine->>Executor: Start execution
    
    loop For each stage
        Executor->>Executor: Execute stage steps
        Executor->>Deployer: Deploy if needed
        Deployer-->>Executor: Deployment result
    end
    
    Executor-->>Engine: Execution complete
    Engine-->>Handler: Return execution ID
    Handler-->>API: JSON response
    API-->>Client: Execution started response
```

### File Upload Flow

```mermaid
sequenceDiagram
    participant Client
    participant API
    participant Upload
    participant Validator
    participant Engine
    
    Client->>API: POST /ci/pipelines/upload (multipart)
    API->>Upload: Handle multipart data
    Upload->>Upload: Extract file content
    Upload->>Validator: Validate YAML
    Validator-->>Upload: Validation result
    Upload->>Engine: Create pipeline
    Engine-->>Upload: Pipeline created
    Upload-->>API: JSON response
    API-->>Client: Pipeline created response
```

## Data Models

### Core Entities

#### Pipeline
```rust
pub struct CIPipeline {
    pub id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub triggers: Vec<Trigger>,
    pub stages: Vec<Stage>,
    pub environment: HashMap<String, String>,
    pub timeout: u64,
    pub retry_count: u32,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
```

#### Execution
```rust
pub struct PipelineExecution {
    pub id: Uuid,
    pub pipeline_id: Uuid,
    pub status: ExecutionStatus,
    pub trigger_info: TriggerInfo,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration: Option<u64>,
    pub stages: Vec<StageExecution>,
    pub logs: Vec<String>,
}
```

#### Deployment Result
```rust
pub struct DeploymentResult {
    pub deployment_id: Uuid,
    pub deployment_type: DeploymentType,
    pub status: DeploymentStatus,
    pub artifacts: Vec<DeploymentArtifact>,
    pub services: Vec<DeployedService>,
    pub logs: Vec<String>,
}
```

## Database Schema

### Collections

#### pipelines
```json
{
  "_id": "ObjectId",
  "id": "UUID",
  "name": "String",
  "description": "String",
  "yaml_content": "String",
  "triggers": "Array",
  "stages": "Array",
  "environment": "Object",
  "timeout": "Number",
  "retry_count": "Number",
  "created_at": "Date",
  "updated_at": "Date"
}
```

#### executions
```json
{
  "_id": "ObjectId",
  "id": "UUID",
  "pipeline_id": "UUID",
  "status": "String",
  "trigger_info": "Object",
  "started_at": "Date",
  "finished_at": "Date",
  "duration": "Number",
  "stages": "Array",
  "logs": "Array",
  "environment": "Object"
}
```

#### services
```json
{
  "_id": "ObjectId",
  "id": "UUID",
  "name": "String",
  "service_type": "String",
  "endpoint": "String",
  "deployment_id": "UUID",
  "container_id": "String",
  "process_id": "Number",
  "status": "String",
  "ports": "Array",
  "last_heartbeat": "Date",
  "metadata": "Object"
}
```

## Security Architecture

### Authentication Flow
1. Client requests OAuth authorization
2. GitHub OAuth callback provides authorization code
3. Server exchanges code for access token
4. Server generates JWT token
5. Client uses JWT for subsequent requests

### Security Measures
- JWT token expiration and refresh
- Input validation and sanitization
- File upload size and type restrictions
- Docker container isolation
- Environment variable encryption
- Rate limiting and DDoS protection

## Performance Considerations

### Async Architecture
- Tokio async runtime for non-blocking I/O
- Connection pooling for database operations
- Concurrent pipeline execution
- Streaming responses for large data

### Caching Strategy
- Pipeline configuration caching
- Execution status caching
- Docker image layer caching
- Git repository caching

### Resource Management
- Workspace cleanup after execution
- Container lifecycle management
- Port allocation and deallocation
- Memory usage monitoring

## Scalability Design

### Horizontal Scaling
- Stateless application design
- Database connection pooling
- Load balancer compatibility
- Session storage externalization

### Vertical Scaling
- Efficient memory usage
- CPU-intensive task optimization
- I/O operation optimization
- Resource monitoring and alerting

## Error Handling Strategy

### Error Categories
1. **Validation Errors**: Invalid input data
2. **Authentication Errors**: Authorization failures
3. **External Service Errors**: GitHub, Docker, SSH failures
4. **Internal Errors**: Database, file system issues
5. **Timeout Errors**: Long-running operation failures

### Error Response Format
```json
{
  "error": "ErrorType",
  "message": "Human-readable message",
  "details": "Additional context",
  "suggestions": ["Actionable suggestions"],
  "timestamp": "ISO 8601 timestamp"
}
```

## Monitoring and Observability

### Logging
- Structured logging with tracing crate
- Log levels: ERROR, WARN, INFO, DEBUG, TRACE
- Request/response logging
- Performance metrics logging

### Metrics
- Request latency and throughput
- Pipeline execution times
- Resource utilization
- Error rates and types

### Health Checks
- Application health endpoint
- Database connectivity check
- External service availability
- Resource usage monitoring

## Future Architecture Enhancements

### Agent-Based Architecture
```mermaid
graph TB
    subgraph "RustCI Core"
        CORE[CI Engine]
        REGISTRY[Service Registry]
        COMM[Communication Hub]
    end
    
    subgraph "Remote Agents"
        AGENT1[Frontend Agent 1]
        AGENT2[Frontend Agent 2]
        AGENT3[Backend Agent 1]
    end
    
    subgraph "Deployed Services"
        SERVICE1[Frontend App 1]
        SERVICE2[Frontend App 2]
        SERVICE3[Backend API]
    end
    
    CORE --> REGISTRY
    REGISTRY --> COMM
    COMM <--> AGENT1
    COMM <--> AGENT2
    COMM <--> AGENT3
    
    AGENT1 <--> SERVICE1
    AGENT2 <--> SERVICE2
    AGENT3 <--> SERVICE3
```

### Microservices Evolution
- Pipeline Service
- Execution Service
- Deployment Service
- Notification Service
- Monitoring Service

### Cloud-Native Features
- Kubernetes integration
- Container orchestration
- Service mesh integration
- Cloud storage backends
- Multi-region deployment