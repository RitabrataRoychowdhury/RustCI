# System Architecture

This document provides a comprehensive overview of the RustCI system architecture, component interactions, and request flow patterns.

## System Overview

RustCI is a high-performance CI/CD platform built in Rust that provides pipeline execution, deployment management, and service orchestration capabilities. The system is designed for speed, reliability, and ease of use.

## Architecture Diagram

```mermaid
%%{ init: { 'flowchart': { 'curve': 'cardinal' } } }%%
flowchart TB
    %% ===== Client Layer =====
    subgraph CLIENT [Client Layer]
        CLI@{ shape: stadium, label: "CLI Tools" }
        WEB@{ shape: stadium, label: "Web Interface" }
        API_CLIENT@{ shape: stadium, label: "API Clients" }
        WEBHOOK@{ shape: stadium, label: "Webhook Sources" }
    end

    %% ===== API Gateway Layer =====
    subgraph AGW [API Gateway Layer]
        AXUM@{ shape: rect, label: "Axum Web Server" }
        AUTH@{ shape: diamond, label: "Authentication\nMiddleware" }
        CORS@{ shape: diamond, label: "CORS Middleware" }
        RATE@{ shape: hex, label: "Rate Limiting" }
    end

    %% ===== Application Layer =====
    subgraph APP [Application Layer]
        ROUTES@{ shape: rounded, label: "Route Handlers" }
        UPLOAD@{ shape: fr-rect, label: "File Upload Handler" }
        PIPELINE@{ shape: fr-rect, label: "Pipeline Handler" }
        EXEC@{ shape: fr-rect, label: "Execution Handler" }
    end

    %% ===== Business Logic Layer =====
    subgraph BIZ [Business Logic Layer]
        CI_ENGINE@{ shape: rect, label: "CI Engine" }
        PIPELINE_MGR@{ shape: rect, label: "Pipeline Manager" }
        EXEC_ENGINE@{ shape: rect, label: "Execution Engine" }
        CONNECTOR_MGR@{ shape: rect, label: "Connector Manager" }
        PROJECT_DETECT@{ shape: lean-r, label: "Project Type Detector" }
        SERVICE_REG@{ shape: fr-rect, label: "Service Registry" }
    end

    %% ===== Connector System =====
    subgraph CONN [Connector System]
        CONN_FACTORY@{ shape: procs, label: "Connector Factory" }
        DOCKER_CONN@{ shape: tag-rect, label: "Docker Connector" }
        K8S_CONN@{ shape: tag-rect, label: "Kubernetes Connector" }
        AWS_CONN@{ shape: tag-rect, label: "AWS Connector" }
        AZURE_CONN@{ shape: tag-rect, label: "Azure Connector" }
        GCP_CONN@{ shape: tag-rect, label: "GCP Connector" }
        GITHUB_CONN@{ shape: tag-rect, label: "GitHub Connector" }
        GITLAB_CONN@{ shape: tag-rect, label: "GitLab Connector" }
    end

    %% ===== Infrastructure Layer =====
    subgraph INFRA [Infrastructure Layer]
        FILE_SYS@{ shape: doc, label: "File System" }
        WORKSPACE@{ shape: doc, label: "Workspace Manager" }
        LIFECYCLE_HOOKS@{ shape: delay, label: "Lifecycle Hook\nManager" }
        YAML_GEN@{ shape: lin-doc, label: "YAML Generator" }
        VALIDATOR@{ shape: bow-rect, label: "Configuration Validator" }
    end

    %% ===== Data Layer =====
    subgraph DATA [Data Layer]
        MONGODB@{ shape: cyl, label: "MongoDB\nDatabase" }
        CACHE@{ shape: cyl, label: "Redis Cache" }
        LOGS@{ shape: lin-doc, label: "Log Storage" }
    end

    %% ===== External Services =====
    subgraph EXT [External Services]
        DOCKER_API@{ shape: bolt, label: "Docker API" }
        K8S_API@{ shape: bolt, label: "Kubernetes API" }
        AWS_API@{ shape: bolt, label: "AWS Services" }
        AZURE_API@{ shape: bolt, label: "Azure Services" }
        GCP_API@{ shape: bolt, label: "GCP Services" }
        GITHUB_API@{ shape: bolt, label: "GitHub API" }
        GITLAB_API@{ shape: bolt, label: "GitLab API" }
    end

    %% ==== Connections ===
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
    EXEC_ENGINE --> CONNECTOR_MGR
    CONNECTOR_MGR --> PROJECT_DETECT
    CONNECTOR_MGR --> SERVICE_REG

    PIPELINE_MGR --> MONGODB
    EXEC_ENGINE --> MONGODB
    SERVICE_REG --> MONGODB

    CONNECTOR_MGR --> CONN_FACTORY
    CONN_FACTORY --> DOCKER_CONN
    CONN_FACTORY --> K8S_CONN
    CONN_FACTORY --> AWS_CONN
    CONN_FACTORY --> AZURE_CONN
    CONN_FACTORY --> GCP_CONN
    CONN_FACTORY --> GITHUB_CONN
    CONN_FACTORY --> GITLAB_CONN

    DOCKER_CONN --> DOCKER_API
    K8S_CONN --> K8S_API
    AWS_CONN --> AWS_API
    AZURE_CONN --> AZURE_API
    GCP_CONN --> GCP_API
    GITHUB_CONN --> GITHUB_API
    GITLAB_CONN --> GITLAB_API

    K8S_CONN --> LIFECYCLE_HOOKS
    K8S_CONN --> YAML_GEN
    K8S_CONN --> VALIDATOR
    DOCKER_CONN --> VALIDATOR

    LIFECYCLE_HOOKS --> MONGODB
    CONNECTOR_MGR --> FILE_SYS
    CONNECTOR_MGR --> WORKSPACE
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

#### Connector Manager
- **Purpose**: Unified facade for managing all connector operations
- **Responsibilities**:
  - Factory registration and management
  - Connector caching and lifecycle
  - Step execution routing
  - Configuration validation
- **Design Patterns**:
  - Facade Pattern: Single interface for all connectors
  - Factory Pattern: Dynamic connector creation
  - Strategy Pattern: Pluggable execution strategies

## Connector System Architecture

The connector system is the core execution engine that provides pluggable, extensible pipeline step execution across different platforms and services.

### Design Patterns

#### Strategy Pattern
Each connector implements the `Connector` trait, allowing different execution strategies:

```rust
#[async_trait]
pub trait Connector: Send + Sync {
    async fn execute_step(&self, step: &Step, workspace: &Workspace, env: &HashMap<String, String>) -> Result<ExecutionResult>;
    fn connector_type(&self) -> ConnectorType;
    fn name(&self) -> &str;
    fn validate_config(&self, step: &Step) -> Result<()>;
    async fn pre_execute(&self, step: &Step) -> Result<()>;
    async fn post_execute(&self, step: &Step, result: &ExecutionResult) -> Result<()>;
}
```

#### Factory Pattern
The `ConnectorFactory` creates connector instances dynamically:

```rust
#[async_trait]
pub trait ConnectorFactory: Send + Sync {
    fn create_connector(&self, connector_type: ConnectorType) -> Result<Arc<dyn Connector>>;
    fn supports_type(&self, connector_type: &ConnectorType) -> bool;
    fn name(&self) -> &str;
}
```

#### Facade Pattern
The `ConnectorManager` provides a unified interface for all connector operations:

```rust
pub struct ConnectorManager {
    factories: HashMap<String, Arc<dyn ConnectorFactory>>,
    connector_cache: HashMap<ConnectorType, Arc<dyn Connector>>,
}
```

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
    participant ConnectorMgr as Connector Manager
    participant Connector
    participant External as External Service
    
    Client->>API: POST /ci/pipelines/{id}/trigger
    API->>Handler: Route to trigger_pipeline
    Handler->>Engine: Trigger execution
    Engine->>Executor: Start execution
    
    loop For each stage
        loop For each step in stage
            Executor->>ConnectorMgr: execute_step(step, workspace, env)
            ConnectorMgr->>ConnectorMgr: determine_connector_type(step)
            ConnectorMgr->>ConnectorMgr: get_connector(connector_type)
            ConnectorMgr->>Connector: validate_config(step)
            ConnectorMgr->>Connector: pre_execute(step)
            ConnectorMgr->>Connector: execute_step(step, workspace, env)
            Connector->>External: Execute operation
            External-->>Connector: Operation result
            Connector-->>ConnectorMgr: ExecutionResult
            ConnectorMgr->>Connector: post_execute(step, result)
            ConnectorMgr-->>Executor: ExecutionResult
        end
    end
    
    Executor-->>Engine: Execution complete
    Engine-->>Handler: Return execution ID
    Handler-->>API: JSON response
    API-->>Client: Execution started response
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