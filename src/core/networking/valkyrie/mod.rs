//! Valkyrie Protocol Implementation
//!
//! The Valkyrie Protocol is RustCI's flagship distributed communication system
//! that enables secure, high-performance, and fault-tolerant communication
//! between control planes and distributed nodes.

// Central type definitions - imported first to ensure consistency
pub mod types;

pub mod adapters;
pub mod engine;
pub mod lockfree;
pub mod message;
pub mod message_integration;
pub mod observability;
pub mod performance_bench;
pub mod registry;
pub mod routing;
pub mod security;
pub mod simd_processor;
pub mod streaming;
pub mod transport;
pub mod zero_copy;

// Re-export central types (canonical definitions)
pub use types::{
    ClientMessage,
    // Core identifiers
    ConnectionId,
    // Error types
    ConversionError,
    CorrelationId,
    Duration,
    EndpointId,
    // Type aliases for backward compatibility
    EngineMessage,
    Message,
    MessageHeader,
    MessagePriority,
    SecurityLevel,
    StreamId,
    // Transport types
    TransportCapabilities,
    // Core message types (several come from message.rs)
    ValkyrieMessage,
};

// Types now re-exported from types.rs above

// Re-export main components from engine
pub use engine::{
    BroadcastResult, ConnectionHandle, Endpoint, EngineStats, Listener, MessageHandler,
    ProtocolState, ValkyrieConfig, ValkyrieEngine, ValkyrieEvent,
};

// Re-export additional types from types.rs
pub use types::{
    CompressionAlgorithm, CompressionPreference, CustomSelector, DestinationType, EncryptionCipher,
    FileTransferOperation, FileTransferPayload, GeographicSelector, LoadBalancingStrategy,
    MessageFlags, MessagePayload, MessageSignature, MessageType, ProtocolExtension, ProtocolInfo,
    ProtocolVersion, ReliabilityLevel, RoutingHints, RoutingInfo, ServiceSelector,
    SignatureAlgorithm, StreamOperation, StreamPayload, StructuredPayload, TraceContext,
};

// Re-export additional message system components (non-conflicting)
pub use message::{MessageValidationError, MessageValidator};

// Re-export message integration components
pub use message_integration::{
    EnhancedMessageProcessor, MessageBatchProcessor, ProcessorMetrics, QoSPolicyConfig,
};

// Re-export transport components (non-conflicting)
pub use transport::{
    BackoffStrategy as TransportBackoffStrategy, CompressionConfig,
    ConnectionPool as TransportConnectionPool, FailoverConfig, QuicTransport, SelectionState,
    TcpTransport, TransportHealth, TransportManager, TransportSelectionStrategy, TransportSelector,
    UnixSocketTransport, WebSocketTransport,
};

// Re-export security components
pub use security::{
    AccessPolicy, Aes256GcmEngine, AnalysisResult, AuditConfig, AuditMetrics, AuthCapabilities,
    AuthConfig, AuthCredentialData, AuthCredentials, AuthMethod, AuthProvider, AuthProviderMetrics,
    AuthResult, AuthSubject, CaCertificate, CertificateConfig, CertificateInfo, CertificateManager,
    CertificateMetrics, CertificateRequest, CertificateStatus, ChaCha20Poly1305Engine,
    ClassicalAlgorithm, ComplianceReport, ConditionOperator,
    CorrelationContext as SecurityCorrelationContext,
    CorrelationTracker as SecurityCorrelationTracker, DilithiumVariant, EncryptionCapabilities,
    EncryptionContext, EncryptionEngine, EncryptionMethod, EncryptionMetrics, HsmConfig, IdsConfig,
    IdsMetrics, IntegrityChain, IntrusionDetectionSystem, JwtAuthProvider, JwtClaims, JwtConfig,
    KyberVariant, LdapConfig, MtlsAuthProvider, OAuth2Config, Permission, PermissionCondition,
    PolicyEffect, PolicyRule, PostQuantumConfig, PostQuantumCrypto, PostQuantumEngine,
    PostQuantumMetrics, RbacConfig, RbacManager, RbacMetrics, RevocationInfo, RevocationReason,
    Role, SecurityAuditEntry, SecurityAuditLogger, SecurityConfig, SecurityEvent,
    SecurityEventType, SecurityHealthStatus, SecurityManager, SecurityMetrics, SecuritySeverity,
    SpiffeAuthProvider, SpiffeConfig, ThreatAssessment, ThreatLevel, ThreatType,
};

// Re-export streaming components
pub use streaming::{
    BandwidthAllocationAlgorithm, BandwidthAllocator, CongestionAlgorithm,
    CongestionControlAlgorithm, CongestionController, CongestionMetrics, CongestionState,
    DefaultStreamEventHandler, FlowControlConfig, FlowControlEvent, FlowControlManager, FlowWindow,
    LatencyPercentiles, MultiplexerConfig, MultiplexerStatistics, PriorityScheduler, QosManager,
    QosMetrics, QosPolicy, QosRequirements, QueueStatistics, RecoveryStrategy, ScheduledMessage,
    SchedulerConfig, SchedulerMetrics, SchedulingAlgorithm, SchedulingTask, ServiceLevelAgreement,
    SlaMonitor, SlaViolation, SlaViolationType, Stream, StreamConfig, StreamEvent,
    StreamEventHandler, StreamMetrics, StreamMultiplexer, StreamPriority, StreamState, StreamType,
    TaskState, ViolationSeverity,
};

// Re-export zero-copy optimization components
pub use zero_copy::{
    PoolStats, ProcessingStats, SimdBuffer, SimdDataProcessor, ZeroCopyBufferPool, ZeroCopyFile,
    ZeroCopySerializer,
};

// Re-export SIMD processor components
pub use simd_processor::{
    MatchStats, MessageProcessingStats, SimdMessageProcessor, SimdPattern, SimdPatternMatcher,
};

// Re-export performance benchmark components
pub use performance_bench::{BenchmarkResults, PerformanceBenchmark};

// Re-export lock-free data structure components
pub use lockfree::{
    AcqRelCounter, AtomicCounter, BackoffStrategy as LockFreeBackoffStrategy, BufferPool,
    CacheLinePadded, ConcurrentHashMap, ConnectionPool as LockFreeConnectionPool,
    ConnectionRegistry, CounterError, CounterStats, EliminationStack, HazardPointer,
    LockFreeConfig, LockFreeMap, LockFreeMetrics, LockFreePool, LockFreeQueue, LockFreeRegistry,
    LockFreeRingBuffer, LockFreeStack, MapEntry, MapError, MapStats, MemoryOrdering, MpmcQueue,
    MpmcRingBuffer, MpscQueue, NodeRegistry, ObjectPool, PoolError, PoolNode,
    PoolStats as LockFreePoolStats, QueueError, QueueNode, QueueStats, RegistryError,
    RegistryStats, RelaxedCounter, RingBufferError, RingBufferStats, SeqCstCounter,
    ServiceRegistry as LockFreeServiceRegistry, SkipListMap, SpscQueue, SpscRingBuffer, StackError,
    StackNode, StackStats, TreiberStack,
};

// Re-export observability components
pub use observability::{
    CompatibilityConfig, CorrelationId as ObservabilityCorrelationId,
    CorrelationTracker as ObservabilityCorrelationTracker, DashboardConfig,
    ExternalObservabilityConfig, ExternalObservabilityIntegration, ExternalObservabilityStatus,
    FeatureDetector, GrafanaIntegration, HealthCheck, HealthMonitor, HealthStatus,
    IntegrationStatus, JaegerIntegration, LegacyObservabilityAdapter, LogLevel, MetricType,
    MetricValue, MetricsCollector, MetricsDashboard, ObservabilityConfig, ObservabilityError,
    ObservabilityManager, ObservabilityStatus, OpenTelemetryIntegration, PrometheusIntegration,
    ProtocolVersionNegotiator, SpanLog, StructuredLogger, TraceSpan,
};

// Re-export adapter components
pub use adapters::{
    AdapterBuilder, AdapterCapabilities, AdapterConfig, AdapterInfo, AdapterMetrics,
    AdapterRequirements, AdapterSelectionStrategy, DockerAdapterBuilder, HttpAdapterBuilder,
    KubernetesAdapterBuilder, LatencyOptimizedStrategy, PerformanceBasedStrategy,
    QuicTcpAdapterBuilder, RedisAdapterBuilder, ReliabilityBasedStrategy,
    ThroughputOptimizedStrategy, UniversalAdapterFactory,
};

// Re-export registry components
pub use registry::{
    CircuitBreaker, CircuitBreakerState, ClusterConfig, ClusterState, ConsensusManager,
    ConsensusProtocol, DependencyType, DiscoveryQuery, EndpointState, HealthCheckConfig,
    HealthCheckType, HealthMonitor as RegistryHealthMonitor, HealthStatus as RegistryHealthStatus,
    LoadBalancer, LogEntry, LogEntryType, NodeInfo, NodeRole, NodeStatus, QueryType,
    ResourceUtilization, ServiceCapabilities, ServiceDependency, ServiceDiscovery, ServiceEndpoint,
    ServiceEntry, ServiceMetadata, ServiceRegistry, ServiceType,
};

// Re-export routing components
pub use routing::{
    adaptive::FeatureExtractor as RoutingFeatureExtractor,
    AdaptiveError,
    // Adaptive routing
    AdaptiveRoutingEngine,
    AlgorithmManager,
    BandwidthManager,
    CacheConfig,
    CacheStats,
    ConfigError,
    // Configuration
    ConfigurationManager,
    ConsistentHashingStrategy,

    DiscoveryAgent,
    DistanceCalculator,

    GeographicManager,
    Hop,
    LoadBalancerManager,
    LoadBalancingAlgorithm,
    LoadBalancingError,
    // Load balancing
    LoadBalancingStrategy as RoutingLoadBalancingStrategy,
    MetricCollector,
    ModelManager,
    NetworkLink,
    NetworkNode,
    NetworkTopology,
    PatternRecognizer,
    PolicyEngine,
    PolicyEvaluationResult,

    PredictionEngine,
    PriorityManager,
    QoSError,
    QoSMetricsSnapshot,

    // QoS routing
    QoSRouter,
    QoSScheduler,
    RegionManager,
    RequestContext,
    RoundRobinStrategy,
    Route,
    // Caching
    RouteCache,
    RouteCacheKey,

    RouteQualityPrediction,

    RoutingAlgorithm,
    // Core routing types
    RoutingContext,
    RoutingError,

    // Metrics
    RoutingMetricsCollector,
    RoutingMetricsSnapshot,
    RoutingPolicy,
    RoutingRule,
    RoutingStrategy,
    SLAEnforcer,
    SecurityEventType as RoutingSecurityEventType,
    ServiceEndpoint as RoutingServiceEndpoint,
    TopologyError,
    // Topology management
    TopologyManager,
    TrafficShaper,
    WeightedRoundRobinStrategy,
};
