# Runner Configuration Guide

This guide covers comprehensive configuration options for all RustCI runner types, including examples, best practices, and troubleshooting.

## Overview

RustCI runners support flexible configuration through YAML files, environment variables, and API parameters. Configuration covers:

- **Runner Identity**: Name, type, and metadata
- **Connection Settings**: Host, port, and network configuration
- **Resource Limits**: CPU, memory, and disk constraints
- **Security Settings**: Authentication, isolation, and permissions
- **Performance Tuning**: Concurrency, timeouts, and optimization
- **Logging and Monitoring**: Log levels, metrics, and health checks

## Configuration Hierarchy

Configuration is applied in the following order (later sources override earlier ones):

1. **Default Values**: Built-in defaults
2. **Configuration File**: YAML configuration file
3. **Environment Variables**: System environment variables
4. **Command Line Arguments**: CLI flags and options
5. **API Parameters**: Runtime API configuration

## Common Configuration

### Basic Runner Configuration

```yaml
# config/runner.yaml
runner:
  # Runner identification
  name: "production-docker-runner-01"
  type: "docker"  # docker, kubernetes, native, local, valkyrie
  
  # Connection settings
  host: "0.0.0.0"
  port: 8081
  
  # Server connection
  server:
    url: "http://localhost:8080"
    api_key: "${RUSTCI_API_KEY}"
    timeout: 30s
    retry_attempts: 3
    retry_delay: 5s
  
  # Resource limits
  resources:
    max_concurrent_jobs: 5
    cpu_limit: "2000m"      # 2 CPU cores
    memory_limit: "4Gi"     # 4GB RAM
    disk_limit: "20Gi"      # 20GB disk
    
  # Capabilities
  capabilities:
    - "docker"
    - "isolation"
    - "deployment"
    - "security-scan"
  
  # Tags for runner selection
  tags:
    - "production"
    - "docker"
    - "linux"
    - "amd64"
  
  # Metadata
  metadata:
    region: "us-west-2"
    zone: "us-west-2a"
    environment: "production"
    team: "platform"

# Logging configuration
logging:
  level: "info"           # debug, info, warn, error
  format: "json"          # json, text
  output: "stdout"        # stdout, file, syslog
  file_path: "/var/log/rustci-runner.log"
  max_size: "100MB"
  max_files: 5
  compress: true

# Health check configuration
health:
  enabled: true
  interval: 30s
  timeout: 10s
  endpoint: "/health"
  
# Metrics configuration
metrics:
  enabled: true
  port: 9091
  path: "/metrics"
  interval: 15s
```

### Environment Variables

```bash
# Core settings
export RUSTCI_RUNNER_NAME="my-runner"
export RUSTCI_RUNNER_TYPE="docker"
export RUSTCI_RUNNER_HOST="0.0.0.0"
export RUSTCI_RUNNER_PORT="8081"

# Server connection
export RUSTCI_SERVER_URL="http://localhost:8080"
export RUSTCI_API_KEY="your-api-key-here"

# Resource limits
export RUSTCI_MAX_CONCURRENT_JOBS="5"
export RUSTCI_CPU_LIMIT="2000m"
export RUSTCI_MEMORY_LIMIT="4Gi"

# Logging
export RUSTCI_LOG_LEVEL="info"
export RUSTCI_LOG_FORMAT="json"

# Security
export RUSTCI_ENABLE_TLS="true"
export RUSTCI_TLS_CERT_PATH="/etc/ssl/certs/runner.crt"
export RUSTCI_TLS_KEY_PATH="/etc/ssl/private/runner.key"
```

## Runner Type-Specific Configuration

### Docker Runner Configuration

```yaml
# config/docker-runner.yaml
runner:
  name: "docker-runner-01"
  type: "docker"
  host: "0.0.0.0"
  port: 8081

# Docker-specific configuration
docker:
  # Docker daemon connection
  host: "unix:///var/run/docker.sock"  # or tcp://localhost:2376
  api_version: "1.41"
  timeout: 60s
  
  # Container defaults
  default_image: "ubuntu:22.04"
  privileged: false
  network_mode: "bridge"
  
  # Volume mounts
  volumes:
    - "/tmp:/tmp:rw"
    - "/var/run/docker.sock:/var/run/docker.sock:ro"
  
  # Environment variables for containers
  environment:
    - "CI=true"
    - "RUSTCI_RUNNER=docker"
  
  # Resource limits for containers
  resources:
    cpu_limit: "1000m"
    memory_limit: "2Gi"
    memory_swap_limit: "2Gi"
    pids_limit: 1024
  
  # Security settings
  security:
    user: "1000:1000"
    read_only_root_fs: false
    no_new_privileges: true
    security_opt:
      - "no-new-privileges:true"
      - "seccomp:unconfined"
  
  # Registry configuration
  registries:
    - name: "docker.io"
      username: "${DOCKER_USERNAME}"
      password: "${DOCKER_PASSWORD}"
    - name: "ghcr.io"
      username: "${GITHUB_USERNAME}"
      password: "${GITHUB_TOKEN}"
  
  # Cleanup settings
  cleanup:
    containers: true
    images: false
    volumes: true
    networks: true
    interval: "1h"
    max_age: "24h"

# Job execution settings
execution:
  timeout: 3600s          # 1 hour default timeout
  shell: "/bin/bash"
  working_directory: "/workspace"
  
  # Step execution
  step_timeout: 1800s     # 30 minutes per step
  allow_failure: false
  continue_on_error: false
  
  # Artifact handling
  artifacts:
    enabled: true
    base_path: "/workspace/artifacts"
    max_size: "1Gi"
    retention: "7d"
```

### Kubernetes Runner Configuration

```yaml
# config/kubernetes-runner.yaml
runner:
  name: "k8s-runner-01"
  type: "kubernetes"
  host: "0.0.0.0"
  port: 8081

# Kubernetes-specific configuration
kubernetes:
  # Cluster connection
  config_path: "~/.kube/config"  # or in-cluster config
  context: "production-cluster"
  namespace: "rustci-runners"
  
  # Pod defaults
  pod:
    image: "ubuntu:22.04"
    image_pull_policy: "IfNotPresent"
    restart_policy: "Never"
    
    # Resource requests and limits
    resources:
      requests:
        cpu: "100m"
        memory: "256Mi"
      limits:
        cpu: "2000m"
        memory: "4Gi"
    
    # Security context
    security_context:
      run_as_user: 1000
      run_as_group: 1000
      fs_group: 1000
      run_as_non_root: true
    
    # Node selection
    node_selector:
      kubernetes.io/arch: "amd64"
      node-type: "runner"
    
    # Tolerations
    tolerations:
      - key: "runner-dedicated"
        operator: "Equal"
        value: "true"
        effect: "NoSchedule"
    
    # Affinity rules
    affinity:
      node_affinity:
        preferred_during_scheduling_ignored_during_execution:
          - weight: 100
            preference:
              match_expressions:
                - key: "zone"
                  operator: "In"
                  values: ["us-west-2a", "us-west-2b"]
  
  # Service account
  service_account:
    name: "rustci-runner"
    create: true
    annotations:
      eks.amazonaws.com/role-arn: "arn:aws:iam::123456789012:role/RustCIRunnerRole"
  
  # Persistent volumes
  volumes:
    - name: "workspace"
      type: "emptyDir"
      mount_path: "/workspace"
      size: "10Gi"
    - name: "cache"
      type: "persistentVolumeClaim"
      mount_path: "/cache"
      storage_class: "fast-ssd"
      size: "50Gi"
  
  # Secrets and config maps
  secrets:
    - name: "docker-registry"
      mount_path: "/etc/docker/config.json"
      sub_path: "config.json"
  
  config_maps:
    - name: "runner-config"
      mount_path: "/etc/rustci"

# Job execution settings
execution:
  timeout: 3600s
  cleanup_policy: "always"  # always, on_success, on_failure, never
  
  # Pod lifecycle
  active_deadline_seconds: 3600
  backoff_limit: 3
  ttl_seconds_after_finished: 300
```

### Native Runner Configuration

```yaml
# config/native-runner.yaml
runner:
  name: "native-runner-01"
  type: "native"
  host: "0.0.0.0"
  port: 8081

# Native runner specific configuration
native:
  # Process execution
  shell: "/bin/bash"
  working_directory: "/tmp/rustci-jobs"
  
  # User and group for job execution
  user: "rustci"
  group: "rustci"
  
  # Environment isolation
  isolation:
    level: "process"        # none, process, chroot, namespace
    create_new_session: true
    set_process_group: true
    
    # Namespace isolation (Linux only)
    namespaces:
      pid: true
      net: false
      mount: true
      user: false
      ipc: true
      uts: true
    
    # Resource limits (cgroups)
    cgroups:
      enabled: true
      cpu_quota: 200000      # 2 CPU cores (200000/100000)
      cpu_period: 100000
      memory_limit: 4294967296  # 4GB in bytes
      pids_limit: 1024
  
  # File system restrictions
  filesystem:
    read_only_paths:
      - "/etc"
      - "/usr"
      - "/bin"
      - "/sbin"
    
    writable_paths:
      - "/tmp"
      - "/var/tmp"
      - "/workspace"
    
    mount_tmpfs:
      - "/tmp"
      - "/var/tmp"
  
  # Network restrictions
  network:
    allow_outbound: true
    allowed_hosts:
      - "github.com"
      - "registry.npmjs.org"
      - "pypi.org"
    blocked_ports:
      - 22    # SSH
      - 3389  # RDP
      - 5432  # PostgreSQL
      - 3306  # MySQL

# Security settings
security:
  # Command filtering
  command_filter:
    enabled: true
    blocked_commands:
      - "rm"
      - "rmdir"
      - "dd"
      - "mkfs"
      - "fdisk"
    
    blocked_patterns:
      - "rm -rf /"
      - ":(){ :|:& };:"  # Fork bomb
      - "curl.*|.*sh"    # Pipe to shell
  
  # Environment variable filtering
  env_filter:
    preserve:
      - "PATH"
      - "HOME"
      - "USER"
      - "LANG"
    
    block:
      - "AWS_*"
      - "GOOGLE_*"
      - "*_PASSWORD"
      - "*_SECRET"
      - "*_KEY"

# Job execution settings
execution:
  timeout: 3600s
  max_output_size: "100MB"
  
  # Process limits
  max_processes: 100
  max_open_files: 1024
  max_file_size: "1GB"
  
  # Working directory management
  workspace:
    create_per_job: true
    cleanup_after: true
    base_path: "/tmp/rustci-jobs"
    max_size: "10GB"
```

### Valkyrie Runner Configuration

```yaml
# config/valkyrie-runner.yaml
runner:
  name: "valkyrie-runner-01"
  type: "valkyrie"
  host: "0.0.0.0"
  port: 8081

# Valkyrie-specific configuration
valkyrie:
  # Valkyrie engine settings
  engine:
    protocol_version: "1.0"
    compression: "lz4"
    encryption: "aes256"
    
  # Node configuration
  node:
    id: "runner-node-01"
    region: "us-west-2"
    zone: "us-west-2a"
    
  # Cluster membership
  cluster:
    discovery:
      method: "dns"         # dns, consul, etcd, static
      endpoints:
        - "rustci-cluster.local"
      interval: 30s
    
    # Gossip protocol
    gossip:
      port: 7946
      interval: 1s
      timeout: 10s
      
  # Message routing
  routing:
    strategy: "consistent_hash"  # round_robin, least_connections, consistent_hash
    hash_key: "job_id"
    
  # Performance settings
  performance:
    max_connections: 1000
    connection_pool_size: 100
    message_buffer_size: 10000
    batch_size: 100
    flush_interval: 100ms
    
  # Reliability
  reliability:
    retry_attempts: 3
    retry_backoff: "exponential"  # linear, exponential
    circuit_breaker:
      enabled: true
      failure_threshold: 5
      recovery_timeout: 30s
      
  # Observability
  observability:
    tracing:
      enabled: true
      sampling_rate: 0.1
      jaeger_endpoint: "http://jaeger:14268/api/traces"
    
    metrics:
      enabled: true
      prometheus_endpoint: "http://prometheus:9090"
      custom_metrics:
        - "job_execution_time"
        - "message_throughput"
        - "error_rate"

# Distributed execution settings
distributed:
  # Job distribution
  distribution:
    strategy: "load_balanced"   # random, round_robin, load_balanced, affinity
    affinity_rules:
      - key: "job_type"
        values: ["build", "test"]
        weight: 100
    
  # State synchronization
  state_sync:
    enabled: true
    interval: 5s
    conflict_resolution: "last_write_wins"  # last_write_wins, vector_clock
    
  # Fault tolerance
  fault_tolerance:
    replication_factor: 3
    consistency_level: "quorum"  # one, quorum, all
    read_repair: true
    
# Job execution settings
execution:
  # Execution modes
  mode: "distributed"     # local, distributed, hybrid
  
  # Resource allocation
  resource_allocation:
    strategy: "best_fit"  # first_fit, best_fit, worst_fit
    consider_affinity: true
    
  # Job scheduling
  scheduling:
    queue_type: "priority"  # fifo, priority, fair_share
    priority_levels: 5
    preemption: false
```

## Advanced Configuration

### Security Configuration

```yaml
# Security settings for all runner types
security:
  # TLS/SSL configuration
  tls:
    enabled: true
    cert_file: "/etc/ssl/certs/runner.crt"
    key_file: "/etc/ssl/private/runner.key"
    ca_file: "/etc/ssl/certs/ca.crt"
    min_version: "1.2"
    cipher_suites:
      - "TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384"
      - "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256"
  
  # Authentication
  authentication:
    method: "jwt"           # jwt, api_key, mutual_tls
    jwt:
      secret: "${JWT_SECRET}"
      algorithm: "HS256"
      expiration: "1h"
    api_key:
      header: "X-API-Key"
      validation_endpoint: "/auth/validate"
  
  # Authorization
  authorization:
    enabled: true
    rbac:
      roles:
        - name: "runner"
          permissions:
            - "job:execute"
            - "status:report"
            - "logs:write"
        - name: "admin"
          permissions:
            - "*"
  
  # Input validation
  input_validation:
    enabled: true
    max_job_size: "10MB"
    max_step_count: 100
    max_command_length: 10000
    
    # Command sanitization
    sanitization:
      remove_dangerous_chars: true
      escape_shell_chars: true
      validate_commands: true
  
  # Audit logging
  audit:
    enabled: true
    log_file: "/var/log/rustci-audit.log"
    events:
      - "job_start"
      - "job_complete"
      - "job_fail"
      - "config_change"
      - "auth_failure"
```

### Performance Tuning

```yaml
# Performance optimization settings
performance:
  # Concurrency settings
  concurrency:
    max_concurrent_jobs: 10
    job_queue_size: 100
    worker_threads: 4
    
  # Connection pooling
  connection_pool:
    max_connections: 50
    min_connections: 5
    connection_timeout: 30s
    idle_timeout: 300s
    
  # Caching
  cache:
    enabled: true
    type: "redis"           # memory, redis, memcached
    redis:
      host: "redis:6379"
      password: "${REDIS_PASSWORD}"
      db: 0
      max_connections: 10
    
    # Cache policies
    policies:
      job_results:
        ttl: "1h"
        max_size: "100MB"
      docker_images:
        ttl: "24h"
        max_size: "10GB"
  
  # Resource optimization
  resources:
    # Memory management
    memory:
      gc_threshold: "80%"
      max_heap_size: "2GB"
      
    # Disk management
    disk:
      cleanup_threshold: "85%"
      cleanup_interval: "1h"
      temp_dir_size: "5GB"
      
    # CPU optimization
    cpu:
      nice_level: 0
      cpu_affinity: "auto"   # auto, manual, none
      
  # I/O optimization
  io:
    buffer_size: "64KB"
    sync_writes: false
    use_direct_io: false
```

### Monitoring and Observability

```yaml
# Monitoring configuration
monitoring:
  # Health checks
  health_checks:
    enabled: true
    interval: 30s
    timeout: 10s
    
    checks:
      - name: "server_connectivity"
        type: "http"
        endpoint: "${RUSTCI_SERVER_URL}/health"
        
      - name: "docker_daemon"
        type: "docker"
        command: "docker info"
        
      - name: "disk_space"
        type: "disk"
        path: "/tmp"
        threshold: "90%"
        
      - name: "memory_usage"
        type: "memory"
        threshold: "85%"
  
  # Metrics collection
  metrics:
    enabled: true
    port: 9091
    path: "/metrics"
    
    # Prometheus configuration
    prometheus:
      enabled: true
      push_gateway: "http://prometheus-pushgateway:9091"
      job_name: "rustci-runner"
      instance: "${HOSTNAME}"
      
    # Custom metrics
    custom_metrics:
      - name: "job_execution_duration"
        type: "histogram"
        buckets: [0.1, 0.5, 1.0, 2.5, 5.0, 10.0]
        
      - name: "active_jobs"
        type: "gauge"
        
      - name: "job_success_rate"
        type: "counter"
  
  # Distributed tracing
  tracing:
    enabled: true
    service_name: "rustci-runner"
    
    # Jaeger configuration
    jaeger:
      endpoint: "http://jaeger:14268/api/traces"
      sampling_rate: 0.1
      
    # OpenTelemetry configuration
    opentelemetry:
      endpoint: "http://otel-collector:4317"
      protocol: "grpc"
  
  # Log aggregation
  log_aggregation:
    enabled: true
    
    # Fluentd configuration
    fluentd:
      host: "fluentd"
      port: 24224
      tag: "rustci.runner"
      
    # ELK stack configuration
    elasticsearch:
      hosts: ["elasticsearch:9200"]
      index: "rustci-logs"
      
# Alerting configuration
alerting:
  enabled: true
  
  # Alert rules
  rules:
    - name: "high_failure_rate"
      condition: "job_failure_rate > 0.1"
      duration: "5m"
      severity: "warning"
      
    - name: "runner_down"
      condition: "up == 0"
      duration: "1m"
      severity: "critical"
      
    - name: "high_memory_usage"
      condition: "memory_usage > 0.9"
      duration: "2m"
      severity: "warning"
  
  # Notification channels
  notifications:
    - type: "slack"
      webhook_url: "${SLACK_WEBHOOK_URL}"
      channel: "#alerts"
      
    - type: "email"
      smtp_server: "smtp.company.com"
      recipients: ["ops@company.com"]
      
    - type: "pagerduty"
      integration_key: "${PAGERDUTY_KEY}"
```

## Configuration Examples

### Development Environment

```yaml
# config/dev-runner.yaml
runner:
  name: "dev-runner"
  type: "docker"
  host: "localhost"
  port: 8081
  
  server:
    url: "http://localhost:8080"
    timeout: 10s
  
  resources:
    max_concurrent_jobs: 2
    cpu_limit: "1000m"
    memory_limit: "2Gi"

docker:
  privileged: true
  cleanup:
    interval: "30m"
    max_age: "2h"

logging:
  level: "debug"
  format: "text"

security:
  tls:
    enabled: false
  authentication:
    method: "none"

monitoring:
  health_checks:
    enabled: true
    interval: 60s
  metrics:
    enabled: false
```

### Production Environment

```yaml
# config/prod-runner.yaml
runner:
  name: "prod-runner-${HOSTNAME}"
  type: "kubernetes"
  host: "0.0.0.0"
  port: 8081
  
  server:
    url: "${RUSTCI_SERVER_URL}"
    api_key: "${RUSTCI_API_KEY}"
    timeout: 30s
    retry_attempts: 5
  
  resources:
    max_concurrent_jobs: 20
    cpu_limit: "8000m"
    memory_limit: "16Gi"
  
  tags:
    - "production"
    - "kubernetes"
    - "high-capacity"

kubernetes:
  namespace: "rustci-prod"
  pod:
    resources:
      requests:
        cpu: "500m"
        memory: "1Gi"
      limits:
        cpu: "4000m"
        memory: "8Gi"
    
    security_context:
      run_as_non_root: true
      read_only_root_fs: true

security:
  tls:
    enabled: true
    cert_file: "/etc/ssl/certs/runner.crt"
    key_file: "/etc/ssl/private/runner.key"
  
  authentication:
    method: "jwt"
    jwt:
      secret: "${JWT_SECRET}"
  
  authorization:
    enabled: true
  
  audit:
    enabled: true

monitoring:
  health_checks:
    enabled: true
    interval: 30s
  
  metrics:
    enabled: true
    prometheus:
      enabled: true
      push_gateway: "${PROMETHEUS_PUSHGATEWAY_URL}"
  
  tracing:
    enabled: true
    jaeger:
      endpoint: "${JAEGER_ENDPOINT}"

alerting:
  enabled: true
  notifications:
    - type: "slack"
      webhook_url: "${SLACK_WEBHOOK_URL}"
    - type: "pagerduty"
      integration_key: "${PAGERDUTY_KEY}"

logging:
  level: "info"
  format: "json"
  log_aggregation:
    enabled: true
    elasticsearch:
      hosts: ["${ELASTICSEARCH_HOSTS}"]
```

### High-Performance Environment

```yaml
# config/hpc-runner.yaml
runner:
  name: "hpc-runner-${NODE_ID}"
  type: "valkyrie"
  host: "0.0.0.0"
  port: 8081
  
  resources:
    max_concurrent_jobs: 50
    cpu_limit: "32000m"
    memory_limit: "64Gi"

valkyrie:
  performance:
    max_connections: 5000
    connection_pool_size: 500
    message_buffer_size: 100000
    batch_size: 1000
    flush_interval: 10ms
  
  cluster:
    gossip:
      interval: 100ms
      
distributed:
  fault_tolerance:
    replication_factor: 5
    consistency_level: "quorum"

performance:
  concurrency:
    max_concurrent_jobs: 50
    worker_threads: 16
  
  connection_pool:
    max_connections: 200
    min_connections: 50
  
  cache:
    enabled: true
    type: "redis"
    policies:
      job_results:
        ttl: "30m"
        max_size: "1GB"
  
  resources:
    memory:
      max_heap_size: "32GB"
    cpu:
      cpu_affinity: "manual"
      affinity_mask: "0-31"
  
  io:
    buffer_size: "1MB"
    use_direct_io: true

monitoring:
  metrics:
    enabled: true
    custom_metrics:
      - name: "throughput_ops_per_second"
        type: "gauge"
      - name: "latency_p99"
        type: "histogram"
```

## Configuration Validation

### Validation Script

```bash
#!/bin/bash
# validate-config.sh

CONFIG_FILE="${1:-config/runner.yaml}"

echo "Validating runner configuration: $CONFIG_FILE"

# Check if file exists
if [[ ! -f "$CONFIG_FILE" ]]; then
    echo "❌ Configuration file not found: $CONFIG_FILE"
    exit 1
fi

# Validate YAML syntax
if ! yq eval '.' "$CONFIG_FILE" > /dev/null 2>&1; then
    echo "❌ Invalid YAML syntax"
    exit 1
fi

echo "✅ YAML syntax is valid"

# Validate required fields
REQUIRED_FIELDS=(
    ".runner.name"
    ".runner.type"
    ".runner.host"
    ".runner.port"
)

for field in "${REQUIRED_FIELDS[@]}"; do
    if ! yq eval "$field" "$CONFIG_FILE" > /dev/null 2>&1; then
        echo "❌ Missing required field: $field"
        exit 1
    fi
done

echo "✅ Required fields are present"

# Validate runner type
RUNNER_TYPE=$(yq eval '.runner.type' "$CONFIG_FILE")
VALID_TYPES=("docker" "kubernetes" "native" "local" "valkyrie")

if [[ ! " ${VALID_TYPES[@]} " =~ " ${RUNNER_TYPE} " ]]; then
    echo "❌ Invalid runner type: $RUNNER_TYPE"
    echo "   Valid types: ${VALID_TYPES[*]}"
    exit 1
fi

echo "✅ Runner type is valid: $RUNNER_TYPE"

# Validate port range
PORT=$(yq eval '.runner.port' "$CONFIG_FILE")
if [[ $PORT -lt 1024 || $PORT -gt 65535 ]]; then
    echo "❌ Invalid port number: $PORT (must be 1024-65535)"
    exit 1
fi

echo "✅ Port number is valid: $PORT"

# Type-specific validation
case "$RUNNER_TYPE" in
    "docker")
        if ! yq eval '.docker' "$CONFIG_FILE" > /dev/null 2>&1; then
            echo "⚠️  Docker configuration section missing"
        fi
        ;;
    "kubernetes")
        if ! yq eval '.kubernetes' "$CONFIG_FILE" > /dev/null 2>&1; then
            echo "⚠️  Kubernetes configuration section missing"
        fi
        ;;
    "valkyrie")
        if ! yq eval '.valkyrie' "$CONFIG_FILE" > /dev/null 2>&1; then
            echo "⚠️  Valkyrie configuration section missing"
        fi
        ;;
esac

echo "✅ Configuration validation completed successfully"
```

### Configuration Testing

```bash
#!/bin/bash
# test-config.sh

CONFIG_FILE="${1:-config/runner.yaml}"

echo "Testing runner configuration: $CONFIG_FILE"

# Start runner with config
./rustci runner --config "$CONFIG_FILE" --dry-run

# Test server connectivity
SERVER_URL=$(yq eval '.runner.server.url // "http://localhost:8080"' "$CONFIG_FILE")
if curl -s "$SERVER_URL/health" > /dev/null; then
    echo "✅ Server connectivity test passed"
else
    echo "❌ Server connectivity test failed"
fi

# Test resource limits
MAX_JOBS=$(yq eval '.runner.resources.max_concurrent_jobs // 5' "$CONFIG_FILE")
echo "✅ Max concurrent jobs: $MAX_JOBS"

CPU_LIMIT=$(yq eval '.runner.resources.cpu_limit // "1000m"' "$CONFIG_FILE")
echo "✅ CPU limit: $CPU_LIMIT"

MEMORY_LIMIT=$(yq eval '.runner.resources.memory_limit // "2Gi"' "$CONFIG_FILE")
echo "✅ Memory limit: $MEMORY_LIMIT"

echo "Configuration test completed"
```

This comprehensive configuration guide provides all the options and examples needed to configure RustCI runners for any environment, from development to high-performance production deployments.