//! Valkyrie High-Performance Routing Components
//!
//! This module contains high-performance routing implementations that integrate
//! with the core Valkyrie protocol for sub-microsecond routing decisions.

pub mod high_performance;
pub mod integration_example;
pub mod enhanced_registries;
pub mod registry_integration_example;
pub mod fuzzy_matching;
pub mod enhanced_service_discovery;

pub use high_performance::{
    HighPerformanceRoutingStrategy,
    HighPerformanceMessageRouter,
    RoutingPerformanceMetrics,
    RoutingStats,
};

pub use enhanced_registries::{
    EnhancedConnectionRegistry,
    EnhancedServiceRegistry,
    EnhancedConnectionInfo,
    EnhancedServiceEntry,
    ConnectionState,
    ServiceHealthStatus,
    ConnectionMetrics,
    ServiceMetrics,
    RegistryStats,
};

pub use fuzzy_matching::{
    FuzzyMatchingEngine,
    FuzzyMatch,
    FuzzyMatchingConfig,
    FuzzyMatchingMetrics,
    FuzzyMatchError,
};

pub use enhanced_service_discovery::{
    EnhancedServiceDiscovery,
    EnhancedServiceDiscoveryConfig,
    ServiceQuery,
    ServiceDiscoveryResult,
    RankedServiceEntry,
    RankingFactors,
    MatchInfo,
    MatchType,
    ServiceDiscoveryMetrics,
    ServiceDiscoveryError,
};