// Configuration Validation System for Valkyrie High-Performance Routing
// Provides comprehensive validation of configuration parameters with detailed error reporting

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigValidationError {
    #[error("Invalid performance budget: {0}. Must be between 1µs and 1ms")]
    InvalidPerformanceBudget(String),
    
    #[error("Invalid throughput target: {0}. Must be between 1,000 and 10,000,000 ops/sec")]
    InvalidThroughputTarget(u64),
    
    #[error("Invalid memory configuration: {0}")]
    InvalidMemoryConfig(String),
    
    #[error("Invalid CPU configuration: {0}")]
    InvalidCpuConfig(String),
    
    #[error("Invalid network configuration: {0}")]
    InvalidNetworkConfig(String),
    
    #[error("Invalid cache configuration: {0}")]
    InvalidCacheConfig(String),
    
    #[error("Configuration conflict: {0}")]
    ConfigConflict(String),
    
    #[error("Missing required configuration: {0}")]
    MissingRequired(String),
    
    #[error("Invalid file path: {0}")]
    InvalidFilePath(String),
}

pub type ValidationResult<T> = Result<T, ConfigValidationError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub recommendations: Vec<String>,
    pub performance_score: u8, // 0-100
}

pub struct ConfigValidator {
    strict_mode: bool,
    performance_requirements: PerformanceRequirements,
}

#[derive(Debug, Clone)]
pub struct PerformanceRequirements {
    pub max_latency_us: u64,
    pub min_throughput_ops: u64,
    pub max_memory_gb: f64,
    pub max_cpu_cores: usize,
}

impl Default for PerformanceRequirements {
    fn default() -> Self {
        Self {
            max_latency_us: 82,           // P99 < 82µs requirement
            min_throughput_ops: 1_000_000, // 1M ops/sec requirement
            max_memory_gb: 8.0,
            max_cpu_cores: 16,
        }
    }
}

impl ConfigValidator {
    pub fn new(strict_mode: bool) -> Self {
        Self {
            strict_mode,
            performance_requirements: PerformanceRequirements::default(),
        }
    }

    pub fn with_requirements(mut self, requirements: PerformanceRequirements) -> Self {
        self.performance_requirements = requirements;
        self
    }

    pub fn validate_config(&self, config: &crate::valkyrie::config::ValkyrieConfig) -> ValidationReport {
        let mut report = ValidationReport {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            recommendations: Vec::new(),
            performance_score: 100,
        };

        // Validate performance configuration
        self.validate_performance_config(&config.performance, &mut report);
        
        // Validate routing configuration
        self.validate_routing_config(&config.routing, &mut report);
        
        // Validate server configuration
        self.validate_server_config(&config.server, &mut report);
        
        // Validate security configuration
        self.validate_security_config(&config.security, &mut report);
        
        // Validate observability configuration
        self.validate_observability_config(&config.observability, &mut report);
        
        // Validate caching configuration
        self.validate_caching_config(&config.caching, &mut report);
        
        // Check for configuration conflicts
        self.check_config_conflicts(config, &mut report);
        
        // Calculate performance score
        self.calculate_performance_score(config, &mut report);
        
        report.is_valid = report.errors.is_empty();
        report
    }

    fn validate_performance_config(
        &self,
        config: &crate::valkyrie::config::PerformanceConfig,
        report: &mut ValidationReport,
    ) {
        // Validate performance budget
        if let Some(budget_str) = &config.performance_budget {
            match self.parse_duration_microseconds(budget_str) {
                Ok(budget_us) => {
                    if budget_us > self.performance_requirements.max_latency_us {
                        report.errors.push(format!(
                            "Performance budget {}µs exceeds requirement of {}µs",
                            budget_us, self.performance_requirements.max_latency_us
                        ));
                    } else if budget_us > self.performance_requirements.max_latency_us / 2 {
                        report.warnings.push(format!(
                            "Performance budget {}µs is close to the limit of {}µs",
                            budget_us, self.performance_requirements.max_latency_us
                        ));
                    }
                }
                Err(_) => {
                    report.errors.push(format!("Invalid performance budget format: {}", budget_str));
                }
            }
        }

        // Validate throughput target
        if let Some(throughput) = config.throughput_target {
            if throughput < self.performance_requirements.min_throughput_ops {
                report.errors.push(format!(
                    "Throughput target {} is below requirement of {} ops/sec",
                    throughput, self.performance_requirements.min_throughput_ops
                ));
            }
        }

        // Validate memory configuration
        if let Some(memory_config) = &config.memory {
            if let Some(max_heap) = &memory_config.max_heap_size {
                match self.parse_memory_size(max_heap) {
                    Ok(heap_gb) => {
                        if heap_gb > self.performance_requirements.max_memory_gb {
                            report.warnings.push(format!(
                                "Max heap size {:.1}GB exceeds recommended {:.1}GB",
                                heap_gb, self.performance_requirements.max_memory_gb
                            ));
                        }
                    }
                    Err(_) => {
                        report.errors.push(format!("Invalid max heap size format: {}", max_heap));
                    }
                }
            }
        }

        // Validate CPU configuration
        if let Some(cpu_config) = &config.cpu {
            if let Some(cpu_cores) = &cpu_config.cpu_cores {
                if cpu_cores.len() > self.performance_requirements.max_cpu_cores {
                    report.warnings.push(format!(
                        "CPU core count {} exceeds recommended {}",
                        cpu_cores.len(), self.performance_requirements.max_cpu_cores
                    ));
                }
            }
        }
    }

    fn validate_routing_config(
        &self,
        config: &crate::valkyrie::config::RoutingConfig,
        report: &mut ValidationReport,
    ) {
        // Validate route cache size
        if config.route_cache_size > 1_000_000 {
            report.warnings.push(
                "Route cache size > 1M may impact memory usage".to_string()
            );
        }

        // Validate connection pool size
        if config.connection_pool_size > 10_000 {
            report.warnings.push(
                "Connection pool size > 10K may impact system resources".to_string()
            );
        }

        // Validate lock-free configuration
        if let Some(lockfree_config) = &config.lockfree {
            if lockfree_config.fit_capacity > 1_000_000 {
                report.warnings.push(
                    "FIT capacity > 1M may impact performance".to_string()
                );
            }
            
            if lockfree_config.fit_load_factor > 0.9 {
                report.warnings.push(
                    "FIT load factor > 0.9 may cause high collision rates".to_string()
                );
            }
        }

        // Validate fuzzy matching configuration
        if let Some(fuzzy_config) = &config.fuzzy_matching {
            if fuzzy_config.timeout_ms > 100 {
                report.warnings.push(
                    "Fuzzy matching timeout > 100ms may impact latency".to_string()
                );
            }
        }
    }

    fn validate_server_config(
        &self,
        config: &crate::valkyrie::config::ServerConfig,
        report: &mut ValidationReport,
    ) {
        // Validate connection limits
        if config.max_connections > 100_000 {
            report.warnings.push(
                "Max connections > 100K may require system tuning".to_string()
            );
        }

        // Validate timeouts
        if config.request_timeout_ms > 30_000 {
            report.warnings.push(
                "Request timeout > 30s may impact user experience".to_string()
            );
        }

        // Validate worker thread configuration
        if config.worker_threads > num_cpus::get() * 2 {
            report.warnings.push(
                "Worker threads exceed 2x CPU cores, may cause contention".to_string()
            );
        }
    }

    fn validate_security_config(
        &self,
        config: &crate::valkyrie::config::SecurityConfig,
        report: &mut ValidationReport,
    ) {
        // Validate TLS configuration
        if config.enable_tls {
            if config.tls_cert_path.is_none() || config.tls_key_path.is_none() {
                report.errors.push(
                    "TLS enabled but certificate or key path not specified".to_string()
                );
            }
        } else if self.strict_mode {
            report.warnings.push(
                "TLS disabled in production environment".to_string()
            );
        }

        // Validate rate limiting
        if let Some(rate_config) = &config.rate_limiting {
            if !rate_config.enable && self.strict_mode {
                report.warnings.push(
                    "Rate limiting disabled in production environment".to_string()
                );
            }
        }
    }

    fn validate_observability_config(
        &self,
        config: &crate::valkyrie::config::ObservabilityConfig,
        report: &mut ValidationReport,
    ) {
        // Validate metrics configuration
        if let Some(metrics_config) = &config.metrics {
            if !metrics_config.enable {
                report.warnings.push(
                    "Metrics collection disabled".to_string()
                );
            }
        }

        // Validate tracing configuration
        if let Some(tracing_config) = &config.tracing {
            if tracing_config.sampling_rate > 1.0 {
                report.errors.push(
                    "Tracing sampling rate cannot exceed 1.0".to_string()
                );
            }
        }
    }

    fn validate_caching_config(
        &self,
        config: &crate::valkyrie::config::CachingConfig,
        report: &mut ValidationReport,
    ) {
        // Validate Redis configuration
        if let Some(redis_config) = &config.redis {
            if redis_config.pool_size > 100 {
                report.warnings.push(
                    "Redis pool size > 100 may impact performance".to_string()
                );
            }
        }

        // Validate cache policies
        if let Some(policies) = &config.policies {
            if let Some(max_memory) = &policies.max_cache_memory {
                match self.parse_memory_size(max_memory) {
                    Ok(memory_gb) => {
                        if memory_gb > self.performance_requirements.max_memory_gb / 2.0 {
                            report.warnings.push(
                                "Cache memory > 50% of system memory".to_string()
                            );
                        }
                    }
                    Err(_) => {
                        report.errors.push(format!(
                            "Invalid cache memory format: {}", max_memory
                        ));
                    }
                }
            }
        }
    }

    fn check_config_conflicts(
        &self,
        config: &crate::valkyrie::config::ValkyrieConfig,
        report: &mut ValidationReport,
    ) {
        // Check for memory conflicts
        let mut total_memory_gb = 0.0;
        
        if let Some(heap_size) = config.performance.memory.as_ref()
            .and_then(|m| m.max_heap_size.as_ref()) {
            if let Ok(heap_gb) = self.parse_memory_size(heap_size) {
                total_memory_gb += heap_gb;
            }
        }
        
        if let Some(cache_memory) = config.caching.policies.as_ref()
            .and_then(|p| p.max_cache_memory.as_ref()) {
            if let Ok(cache_gb) = self.parse_memory_size(cache_memory) {
                total_memory_gb += cache_gb;
            }
        }
        
        if total_memory_gb > self.performance_requirements.max_memory_gb {
            report.errors.push(format!(
                "Total memory allocation {:.1}GB exceeds system limit {:.1}GB",
                total_memory_gb, self.performance_requirements.max_memory_gb
            ));
        }

        // Check for port conflicts
        let mut used_ports = std::collections::HashSet::new();
        used_ports.insert(config.server.port);
        
        if let Some(https_port) = config.server.https_port {
            if !used_ports.insert(https_port) {
                report.errors.push("Port conflict between HTTP and HTTPS".to_string());
            }
        }
        
        if let Some(metrics_port) = config.observability.metrics.as_ref()
            .and_then(|m| m.port) {
            if !used_ports.insert(metrics_port) {
                report.errors.push("Port conflict with metrics endpoint".to_string());
            }
        }
    }

    fn calculate_performance_score(
        &self,
        config: &crate::valkyrie::config::ValkyrieConfig,
        report: &mut ValidationReport,
    ) {
        let mut score = 100u8;
        
        // Deduct points for each error (20 points each)
        score = score.saturating_sub(report.errors.len() as u8 * 20);
        
        // Deduct points for each warning (5 points each)
        score = score.saturating_sub(report.warnings.len() as u8 * 5);
        
        // Performance optimizations bonus
        if config.performance.enable_simd.unwrap_or(false) {
            score = std::cmp::min(100, score + 5);
        }
        
        if config.performance.enable_zero_copy.unwrap_or(false) {
            score = std::cmp::min(100, score + 5);
        }
        
        if config.performance.enable_hot_path_optimization.unwrap_or(false) {
            score = std::cmp::min(100, score + 5);
        }
        
        report.performance_score = score;
        
        // Add recommendations based on score
        if score < 70 {
            report.recommendations.push(
                "Configuration needs significant improvements for production use".to_string()
            );
        } else if score < 85 {
            report.recommendations.push(
                "Consider addressing warnings to improve performance".to_string()
            );
        } else if score >= 95 {
            report.recommendations.push(
                "Excellent configuration for high-performance routing".to_string()
            );
        }
    }

    fn parse_duration_microseconds(&self, duration_str: &str) -> Result<u64, ()> {
        if duration_str.ends_with("us") || duration_str.ends_with("µs") {
            duration_str.trim_end_matches("us").trim_end_matches("µs")
                .parse().map_err(|_| ())
        } else if duration_str.ends_with("ms") {
            duration_str.trim_end_matches("ms")
                .parse::<u64>().map(|ms| ms * 1000).map_err(|_| ())
        } else {
            Err(())
        }
    }

    fn parse_memory_size(&self, size_str: &str) -> Result<f64, ()> {
        if size_str.ends_with("GB") {
            size_str.trim_end_matches("GB").parse().map_err(|_| ())
        } else if size_str.ends_with("MB") {
            size_str.trim_end_matches("MB")
                .parse::<f64>().map(|mb| mb / 1024.0).map_err(|_| ())
        } else {
            Err(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_budget_validation() {
        let validator = ConfigValidator::new(true);
        
        // Test valid budget
        assert_eq!(validator.parse_duration_microseconds("50µs"), Ok(50));
        assert_eq!(validator.parse_duration_microseconds("1ms"), Ok(1000));
        
        // Test invalid budget
        assert!(validator.parse_duration_microseconds("invalid").is_err());
    }

    #[test]
    fn test_memory_size_parsing() {
        let validator = ConfigValidator::new(true);
        
        assert_eq!(validator.parse_memory_size("4GB"), Ok(4.0));
        assert_eq!(validator.parse_memory_size("2048MB"), Ok(2.0));
        assert!(validator.parse_memory_size("invalid").is_err());
    }
}