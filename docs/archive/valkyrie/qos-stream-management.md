# QoS-Aware Stream Management

The QoS-Aware Stream Management system provides intelligent message routing with Quality of Service guarantees, priority-based classification, and adaptive bandwidth allocation for optimal performance in distributed systems.

## Overview

The system consists of three main components:

1. **QoS Stream Router** - Intelligent message routing with priority queues
2. **Message Classifier** - ML-powered message classification with pattern recognition
3. **Bandwidth Allocator** - Adaptive bandwidth management with congestion control

## Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   QoS Stream    │    │    Message       │    │   Bandwidth     │
│     Router      │◄──►│   Classifier     │◄──►│   Allocator     │
└─────────────────┘    └──────────────────┘    └─────────────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│ Priority Queues │    │ ML & Patterns    │    │ Congestion      │
│ Flow Control    │    │ Rule Engine      │    │ Control         │
│ Load Balancing  │    │ Caching          │    │ Throttling      │
└─────────────────┘    └──────────────────┘    └─────────────────┘
```

## QoS Classes

The system defines five QoS classes with different priority levels:

| QoS Class | Priority | Use Case | Latency Target | Bandwidth Share |
|-----------|----------|----------|----------------|-----------------|
| Critical | 0 (Highest) | Health checks, system alerts | <10ms | 30% |
| System | 1 | System management, configuration | <50ms | 25% |
| JobExecution | 2 | CI/CD jobs, pipeline execution | <100ms | 25% |
| DataTransfer | 3 | File transfers, artifacts | <1s | 15% |
| LogsMetrics | 4 (Lowest) | Logs, metrics, monitoring | <5s | 5% |

## Components

### QoS Stream Router

The router provides intelligent message routing with the following features:

#### Priority Queue Management
- Separate queues for each QoS class
- Priority-based message processing
- Deadline-aware scheduling
- Congestion-aware flow control

#### Routing Strategies
- **Direct Routing**: Route to specific adapter
- **Load Balanced**: Distribute across multiple adapters
- **Service-based**: Route based on service name
- **Broadcast**: Send to all adapters

#### Configuration
```rust
let config = QoSRouterConfig {
    max_queue_size: 10000,
    processing_interval: Duration::from_millis(5),
    congestion_threshold: 0.8,
    max_concurrent_streams: 1000,
    adaptive_routing: true,
    load_balancing: true,
};
```

### Message Classifier

The classifier uses multiple techniques to determine the appropriate QoS class:

#### Classification Methods
1. **Rule-based Classification**: Static rules based on message properties
2. **Machine Learning**: Adaptive classification using training data
3. **Pattern Recognition**: Identify recurring message patterns
4. **Caching**: Cache classification results for performance

#### Classification Rules
```rust
ClassificationRule {
    name: "Critical Health Checks".to_string(),
    condition: ClassificationCondition::MessageType(AdapterMessageType::HealthCheck),
    target_class: QoSClass::Critical,
    weight: 1.0,
    priority: 0,
    enabled: true,
}
```

#### Machine Learning Features
- **Feature Extraction**: Message type, priority, size, metadata, timing
- **Training Data**: Continuous learning from classification feedback
- **Model Types**: Linear models, decision trees, neural networks
- **Accuracy Tracking**: Monitor and improve classification accuracy

### Bandwidth Allocator

The allocator provides intelligent bandwidth management:

#### Allocation Strategy
- **Guaranteed Bandwidth**: Minimum bandwidth per QoS class
- **Burst Allowance**: Temporary bandwidth increases
- **Fair Sharing**: Distribute unused bandwidth fairly
- **Adaptive Allocation**: Adjust based on usage patterns

#### Congestion Control
- **Congestion Detection**: Monitor bandwidth utilization, queue depths, latency
- **Throttling Policies**: Different policies per QoS class
- **Recovery Strategies**: Adaptive recovery from congestion
- **Load Shedding**: Drop low-priority traffic when necessary

#### Configuration
```rust
let config = BandwidthConfig {
    total_bandwidth: 1_000_000_000, // 1 Gbps
    adaptive_allocation: true,
    burst_allowance: true,
    burst_multiplier: 2.0,
    throttling_enabled: true,
    congestion_detection: true,
    class_ratios: {
        let mut ratios = HashMap::new();
        ratios.insert(QoSClass::Critical, 0.3);
        ratios.insert(QoSClass::System, 0.25);
        ratios.insert(QoSClass::JobExecution, 0.25);
        ratios.insert(QoSClass::DataTransfer, 0.15);
        ratios.insert(QoSClass::LogsMetrics, 0.05);
        ratios
    },
};
```

## Usage Examples

### Basic Message Routing

```rust
use RustAutoDevOps::core::networking::valkyrie::streaming::*;

// Initialize router
let router = QoSStreamRouter::new(QoSRouterConfig::default());

// Create message
let message = AdapterMessage {
    id: uuid::Uuid::new_v4(),
    message_type: AdapterMessageType::Request,
    payload: b"job execution request".to_vec(),
    priority: MessagePriority::High,
    // ... other fields
};

// Define QoS parameters
let qos_params = QoSParams {
    max_latency: Some(Duration::from_millis(100)),
    reliability: 0.95,
    priority: MessagePriority::High,
};

// Route message
let result = router.route_message(
    message,
    qos_params,
    RouteDestination::Service("job-executor".to_string())
).await?;

println!("Routed to QoS class: {}", result.qos_class);
```

### Bandwidth Allocation

```rust
// Initialize allocator
let allocator = BandwidthAllocator::new();

// Allocate bandwidth for stream
let allocation = allocator.allocate_bandwidth(
    stream_id,
    QoSClass::JobExecution,
    50_000_000, // 50 Mbps
    &qos_params,
).await?;

// Update usage during transfer
allocator.update_usage(
    stream_id,
    1_000_000, // 1MB transferred
    Duration::from_millis(100),
).await?;

// Release when done
allocator.release_bandwidth(stream_id).await?;
```

### Custom Classification

```rust
// Initialize classifier with ML enabled
let config = ClassifierConfig {
    enable_ml: true,
    enable_patterns: true,
    learning_rate: 0.01,
    ..Default::default()
};
let classifier = MessageClassifier::with_config(config);

// Classify message
let qos_class = classifier.classify(&message, &qos_params).await?;
```

## Performance Characteristics

### Latency Targets
- **Classification**: <1ms average, <5ms p99
- **Routing**: <10ms average, <50ms p99
- **Bandwidth Allocation**: <5ms average, <20ms p99

### Throughput Targets
- **Message Routing**: >100,000 messages/second
- **Bandwidth Management**: >10 Gbps aggregate
- **Classification**: >50,000 classifications/second

### Scalability
- **Concurrent Streams**: Up to 10,000 active streams
- **Queue Capacity**: 100,000 messages per QoS class
- **Adapter Support**: Unlimited adapters with load balancing

## Monitoring and Metrics

### Router Metrics
```rust
let metrics = router.metrics().await;
println!("Total messages: {}", metrics.total_messages);
println!("Average latency: {:?}", metrics.avg_routing_latency);
println!("Congestion events: {}", metrics.congestion_events);
```

### Classifier Metrics
```rust
let metrics = classifier.metrics().await;
println!("Classification accuracy: {:.1}%", metrics.ml_accuracy * 100.0);
println!("Cache hit rate: {:.1}%", metrics.cache_hit_rate * 100.0);
```

### Bandwidth Metrics
```rust
let metrics = allocator.metrics().await;
println!("Bandwidth utilization: {:.1}%", metrics.total_utilization * 100.0);
println!("Active streams: {}", metrics.active_streams);
println!("Efficiency: {:.1}%", metrics.efficiency * 100.0);
```

## Advanced Features

### Adaptive Learning
The system continuously learns from message patterns and user feedback:

- **Classification Improvement**: ML models adapt to new message types
- **Bandwidth Optimization**: Allocation adjusts to usage patterns
- **Route Optimization**: Routing decisions improve based on performance

### Congestion Management
Sophisticated congestion control prevents system overload:

- **Early Detection**: Proactive congestion detection
- **Graceful Degradation**: Maintain service for high-priority traffic
- **Recovery Strategies**: Intelligent recovery from congestion events

### Integration Points
The QoS system integrates with:

- **Universal Adapter Factory**: Automatic adapter selection
- **Service Registry**: Service-based routing
- **Monitoring Systems**: Real-time metrics and alerting
- **Security Layer**: QoS-aware security policies

## Best Practices

### Message Design
- Use appropriate message types for automatic classification
- Include relevant metadata for classification rules
- Set realistic QoS parameters based on requirements

### Performance Optimization
- Enable caching for frequently classified messages
- Use burst allowance for temporary high-bandwidth needs
- Monitor metrics and adjust configurations accordingly

### Troubleshooting
- Check classification accuracy if messages are misrouted
- Monitor congestion metrics during high load
- Verify bandwidth allocations match usage patterns

## Configuration Reference

### QoSRouterConfig
```rust
pub struct QoSRouterConfig {
    pub max_queue_size: usize,           // Default: 10000
    pub processing_interval: Duration,    // Default: 10ms
    pub congestion_threshold: f64,        // Default: 0.8
    pub max_concurrent_streams: usize,    // Default: 1000
    pub health_check_interval: Duration,  // Default: 30s
    pub metrics_interval: Duration,       // Default: 10s
    pub adaptive_routing: bool,           // Default: true
    pub load_balancing: bool,            // Default: true
}
```

### ClassifierConfig
```rust
pub struct ClassifierConfig {
    pub enable_ml: bool,                 // Default: true
    pub enable_patterns: bool,           // Default: true
    pub enable_caching: bool,            // Default: true
    pub cache_ttl: Duration,             // Default: 5min
    pub max_cache_size: usize,           // Default: 10000
    pub learning_rate: f64,              // Default: 0.01
    pub pattern_threshold: f64,          // Default: 0.7
    pub ml_threshold: f64,               // Default: 0.8
    pub adaptive_learning: bool,         // Default: true
}
```

### BandwidthConfig
```rust
pub struct BandwidthConfig {
    pub total_bandwidth: u64,            // Default: 1 Gbps
    pub class_ratios: HashMap<QoSClass, f64>, // See above
    pub adaptive_allocation: bool,        // Default: true
    pub burst_allowance: bool,           // Default: true
    pub burst_multiplier: f64,           // Default: 2.0
    pub tracking_interval: Duration,      // Default: 1s
    pub throttling_enabled: bool,        // Default: true
    pub congestion_detection: bool,      // Default: true
}
```

## Future Enhancements

- **Multi-tenant QoS**: Per-tenant QoS policies and isolation
- **Geographic Routing**: Location-aware routing decisions
- **Predictive Scaling**: Proactive resource allocation
- **Advanced ML Models**: Deep learning for classification
- **Real-time Analytics**: Live performance dashboards