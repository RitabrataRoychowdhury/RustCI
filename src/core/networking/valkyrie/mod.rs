//! Valkyrie Protocol Implementation
//!
//! The Valkyrie Protocol is RustCI's flagship distributed communication system
//! that enables secure, high-performance, and fault-tolerant communication
//! between control planes and distributed nodes.

// Central type definitions - imported first to ensure consistency
pub mod types;

pub mod engine;
pub mod message;
pub mod message_integration;
pub mod transport;
pub mod security;
pub mod streaming;
pub mod zero_copy;
pub mod simd_processor;
pub mod performance_bench;
pub mod lockfree;
pub mod observability;

// Re-export central types (canonical definitions)
pub use types::{
    // Core message types (several come from message.rs)
    ValkyrieMessage, MessageHeader, MessagePriority,
    SecurityLevel,
    // Transport types
    TransportCapabilities,
    // Type aliases for backward compatibility
    EngineMessage, ClientMessage, Message,
    // Core identifiers
    ConnectionId, StreamId, EndpointId, CorrelationId, Duration,
    // Error types
    ConversionError,
};

// Types now re-exported from types.rs above

// Re-export main components from engine
pub use engine::{
    ValkyrieEngine, ValkyrieConfig, ConnectionHandle, Listener,
    MessageHandler, BroadcastResult, ValkyrieEvent,
    EngineStats, ProtocolState, Endpoint
};

// Re-export additional types from types.rs
pub use types::{
    DestinationType, CompressionAlgorithm, CompressionPreference, EncryptionCipher,
    ReliabilityLevel, LoadBalancingStrategy, ProtocolVersion,
    ProtocolExtension, RoutingHints, ServiceSelector,
    GeographicSelector, CustomSelector, SignatureAlgorithm, 
    StructuredPayload, StreamPayload, FileTransferPayload, 
    StreamOperation, FileTransferOperation, MessageType, MessagePayload, 
    MessageFlags, RoutingInfo, MessageSignature, TraceContext, ProtocolInfo
};

// Re-export additional message system components (non-conflicting)
pub use message::{
    MessageValidator, MessageValidationError
};

// Re-export message integration components
pub use message_integration::{
    EnhancedMessageProcessor, MessageBatchProcessor, QoSPolicyConfig,
    ProcessorMetrics
};

// Re-export transport components (non-conflicting)
pub use transport::{
    TransportSelectionStrategy, ConnectionPool as TransportConnectionPool,
    FailoverConfig, CompressionConfig,
    BackoffStrategy as TransportBackoffStrategy,
    HealthCheckConfig, TransportManager,
    TcpTransport, QuicTransport, WebSocketTransport, UnixSocketTransport,
    TransportSelector, TransportHealth, SelectionState
};

// Re-export security components
pub use security::{
    SecurityManager, SecurityConfig, SecurityMetrics, SecurityHealthStatus,
    AuthMethod, EncryptionMethod, SecurityEventType, SecuritySeverity,
    AuthProvider, AuthCredentials, AuthCredentialData, AuthResult, AuthSubject,
    AuthCapabilities, AuthProviderMetrics, AuthConfig,
    JwtAuthProvider, JwtConfig, JwtClaims, MtlsAuthProvider,
    SpiffeAuthProvider, SpiffeConfig, OAuth2Config, LdapConfig,
    EncryptionEngine, EncryptionContext, EncryptionCapabilities, EncryptionMetrics,
    PostQuantumCrypto, PostQuantumConfig, PostQuantumMetrics,
    KyberVariant, DilithiumVariant, ClassicalAlgorithm,
    Aes256GcmEngine, ChaCha20Poly1305Engine, PostQuantumEngine,
    RbacManager, RbacConfig, RbacMetrics, Role, Permission, AccessPolicy,
    PermissionCondition, ConditionOperator, PolicyRule, PolicyEffect,
    IntrusionDetectionSystem, IdsConfig, IdsMetrics, SecurityEvent,
    ThreatAssessment, ThreatType, ThreatLevel, AnalysisResult,
    SecurityAuditLogger, AuditConfig, SecurityAuditEntry, AuditMetrics,
    IntegrityChain, CorrelationTracker as SecurityCorrelationTracker, CorrelationContext as SecurityCorrelationContext, ComplianceReport,
    CertificateManager, CertificateConfig, CertificateInfo, CertificateStatus,
    CaCertificate, RevocationInfo, RevocationReason, CertificateMetrics,
    CertificateRequest, HsmConfig
};

// Re-export streaming components
pub use streaming::{
    StreamMultiplexer, MultiplexerConfig, MultiplexerStatistics,
    Stream, StreamConfig, StreamMetrics, StreamState, StreamType, StreamPriority,
    StreamEvent, StreamEventHandler, DefaultStreamEventHandler,
    FlowWindow, FlowControlEvent, CongestionState, CongestionAlgorithm,
    FlowControlManager, FlowControlConfig, FlowControlMetrics,
    CongestionDetector, CongestionDetectionAlgorithm, WindowAdapter, WindowAdaptationAlgorithm,
    PerformanceSample, AdaptationState, AdaptationDirection,
    PriorityScheduler, SchedulingAlgorithm, SchedulerConfig, SchedulerMetrics,
    ScheduledMessage, QosRequirements, SchedulingTask, TaskState,
    BandwidthAllocator, BandwidthAllocationAlgorithm, QosManager, QosPolicy, QosMetrics,
    LatencyPercentiles, SlaMonitor, ServiceLevelAgreement, SlaViolation,
    SlaViolationType, ViolationSeverity, QueueStatistics,
    CongestionController, CongestionControlAlgorithm, CongestionMetrics, RecoveryStrategy
};

// Re-export zero-copy optimization components
pub use zero_copy::{
    SimdBuffer, ZeroCopyBufferPool, PoolStats, SimdDataProcessor, ProcessingStats,
    ZeroCopyFile, ZeroCopySerializer
};

// Re-export SIMD processor components
pub use simd_processor::{
    SimdMessageProcessor, MessageProcessingStats, SimdPatternMatcher,
    SimdPattern, MatchStats
};

// Re-export performance benchmark components
pub use performance_bench::{
    PerformanceBenchmark, BenchmarkResults
};

// Re-export lock-free data structure components
pub use lockfree::{
    LockFreeMetrics, LockFreeConfig, MemoryOrdering, BackoffStrategy as LockFreeBackoffStrategy, CacheLinePadded, HazardPointer,
    LockFreeQueue, QueueStats, QueueError, QueueNode, MpscQueue, SpscQueue, MpmcQueue,
    LockFreeStack, StackStats, StackError, StackNode, TreiberStack, EliminationStack,
    LockFreeMap, MapStats, MapError, MapEntry, ConcurrentHashMap, SkipListMap,
    LockFreeRingBuffer, RingBufferStats, RingBufferError, SpscRingBuffer, MpmcRingBuffer,
    AtomicCounter, CounterStats, CounterError, RelaxedCounter, SeqCstCounter, AcqRelCounter,
    LockFreePool, PoolStats as LockFreePoolStats, PoolError, PoolNode, ObjectPool, BufferPool, ConnectionPool as LockFreeConnectionPool,
    LockFreeRegistry, RegistryStats, RegistryError, NodeRegistry, ServiceRegistry, ConnectionRegistry
};

// Re-export observability components
pub use observability::{
    ObservabilityManager, ObservabilityConfig, ObservabilityStatus, ObservabilityError,
    MetricsCollector, MetricValue, MetricType,
    StructuredLogger, LogLevel, LogEntry,
    HealthMonitor, HealthStatus, HealthCheck,
    MetricsDashboard, DashboardConfig,
    CorrelationTracker as ObservabilityCorrelationTracker, CorrelationId as ObservabilityCorrelationId,
    TraceSpan, SpanLog,
    ExternalObservabilityIntegration, ExternalObservabilityConfig, ExternalObservabilityStatus,
    PrometheusIntegration, OpenTelemetryIntegration, JaegerIntegration, GrafanaIntegration,
    IntegrationStatus, LegacyObservabilityAdapter, CompatibilityConfig,
    ProtocolVersionNegotiator, FeatureDetector
};