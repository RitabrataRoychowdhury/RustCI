use crate::config::{AppConfiguration};
use crate::config::validation_engine::{ConfigValidationEngine, ComprehensiveValidationReport};
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Production-grade configuration manager with comprehensive validation
pub struct ProductionConfigManager {
    config: Arc<RwLock<AppConfiguration>>,
    validation_engine: ConfigValidationEngine,
    environment: String,
    config_path: Option<String>,
    validation_cache: Arc<RwLock<ValidationCache>>,
    validation_rules: Vec<Box<dyn ProductionValidationRule>>,
}

/// Validation cache for performance optimization
#[derive(Debug, Clone)]
struct ValidationCache {
    results: HashMap<String, CachedValidationResult>,
}

#[derive(Debug, Clone)]
struct CachedValidationResult {
    report: ProductionValidationReport,
    timestamp: chrono::DateTime<chrono::Utc>,
    config_hash: u64,
}

/// Production validation report with detailed analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionValidationReport {
    pub is_valid: bool,
    pub validation_sections: HashMap<String, ValidationSection>,
    pub critical_issues: Vec<ValidationIssue>,
    pub warnings: Vec<ValidationIssue>,
    pub recommendations: Vec<ValidationRecommendation>,
    pub environment_specific_issues: Vec<ValidationIssue>,
    pub security_assessment: SecurityAssessment,
    pub performance_assessment: PerformanceAssessment,
    pub compliance_status: ComplianceStatus,
    pub validation_metadata: ValidationMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSection {
    pub name: String,
    pub status: ValidationStatus,
    pub issues: Vec<ValidationIssue>,
    pub score: u8, // 0-100
    pub details: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationStatus {
    Pass,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub severity: IssueSeverity,
    pub category: String,
    pub field_path: String,
    pub message: String,
    pub description: String,
    pub remediation: String,
    pub impact: String,
    pub auto_fixable: bool,
    pub compliance_impact: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IssueSeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRecommendation {
    pub category: String,
    pub title: String,
    pub description: String,
    pub priority: RecommendationPriority,
    pub implementation_effort: ImplementationEffort,
    pub benefits: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationPriority {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImplementationEffort {
    Low,
    Medium,
    High,
    VeryHigh,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAssessment {
    pub overall_score: u8,
    pub encryption_status: String,
    pub authentication_status: String,
    pub authorization_status: String,
    pub audit_status: String,
    pub vulnerabilities: Vec<SecurityVulnerability>,
    pub recommendations: Vec<SecurityRecommendation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityVulnerability {
    pub id: String,
    pub severity: IssueSeverity,
    pub title: String,
    pub description: String,
    pub cve_references: Vec<String>,
    pub mitigation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityRecommendation {
    pub category: String,
    pub recommendation: String,
    pub priority: RecommendationPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAssessment {
    pub overall_score: u8,
    pub bottlenecks: Vec<PerformanceBottleneck>,
    pub optimization_opportunities: Vec<OptimizationOpportunity>,
    pub resource_utilization: ResourceUtilization,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBottleneck {
    pub component: String,
    pub issue: String,
    pub impact: String,
    pub solution: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationOpportunity {
    pub area: String,
    pub current_state: String,
    pub recommended_state: String,
    pub expected_improvement: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUtilization {
    pub cpu_efficiency: u8,
    pub memory_efficiency: u8,
    pub network_efficiency: u8,
    pub storage_efficiency: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceStatus {
    pub overall_compliance: u8,
    pub standards: HashMap<String, ComplianceStandard>,
    pub violations: Vec<ComplianceViolation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceStandard {
    pub name: String,
    pub version: String,
    pub compliance_percentage: u8,
    pub required_controls: Vec<String>,
    pub implemented_controls: Vec<String>,
    pub missing_controls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceViolation {
    pub standard: String,
    pub control: String,
    pub description: String,
    pub severity: IssueSeverity,
    pub remediation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationMetadata {
    pub validation_timestamp: chrono::DateTime<chrono::Utc>,
    pub validation_duration: std::time::Duration,
    pub validator_version: String,
    pub environment: String,
    pub config_source: String,
    pub validation_rules_applied: Vec<String>,
}

/// Production validation rule trait
pub trait ProductionValidationRule: Send + Sync {
    fn validate(&self, config: &AppConfiguration, environment: &str) -> Result<ValidationSection>;
    fn name(&self) -> &str;
    fn category(&self) -> &str;
    fn priority(&self) -> u8; // Lower number = higher priority
    fn applies_to_environment(&self, _environment: &str) -> bool {
        true // Default: applies to all environments
    }
}

impl ProductionConfigManager {
    /// Create a new production configuration manager
    pub fn new(environment: &str) -> Self {
        let validation_engine = ConfigValidationEngine::new(environment);
        
        let mut manager = Self {
            config: Arc::new(RwLock::new(AppConfiguration::default())),
            validation_engine,
            environment: environment.to_string(),
            config_path: None,
            validation_cache: Arc::new(RwLock::new(ValidationCache {
                results: HashMap::new(),
            })),
            validation_rules: Vec::new(),
        };

        // Register production validation rules
        manager.register_production_rules();
        manager
    }

    /// Register all production validation rules
    fn register_production_rules(&mut self) {
        self.validation_rules.push(Box::new(SecurityValidationRule::new()));
        self.validation_rules.push(Box::new(DatabaseValidationRule::new()));
        self.validation_rules.push(Box::new(PerformanceValidationRule::new()));
        self.validation_rules.push(Box::new(ObservabilityValidationRule::new()));
        self.validation_rules.push(Box::new(ComplianceValidationRule::new()));
        self.validation_rules.push(Box::new(EnvironmentValidationRule::new()));
        self.validation_rules.push(Box::new(ResourceValidationRule::new()));
        self.validation_rules.push(Box::new(NetworkValidationRule::new()));
    }

    /// Load and validate configuration with comprehensive analysis
    pub async fn load_and_validate(&mut self) -> Result<ProductionValidationReport> {
        info!("🔍 Starting production configuration validation for environment: {}", self.environment);
        
        // Load configuration
        self.load_configuration().await?;
        
        // Get current configuration
        let config = self.config.read().await.clone();
        
        // Check cache first
        let config_hash = self.calculate_config_hash(&config);
        if let Some(cached) = self.check_validation_cache(config_hash).await {
            info!("📋 Using cached validation result");
            return Ok(cached.report);
        }

        // Perform comprehensive validation
        let validation_start = std::time::Instant::now();
        let mut report = ProductionValidationReport::new(&self.environment);

        // Run all validation rules
        for rule in &self.validation_rules {
            if rule.applies_to_environment(&self.environment) {
                debug!("Running validation rule: {}", rule.name());
                
                match rule.validate(&config, &self.environment) {
                    Ok(section) => {
                        report.add_validation_section(section);
                    }
                    Err(e) => {
                        error!("Validation rule '{}' failed: {}", rule.name(), e);
                        report.add_critical_issue(ValidationIssue {
                            severity: IssueSeverity::Critical,
                            category: "validation_error".to_string(),
                            field_path: rule.name().to_string(),
                            message: format!("Validation rule failed: {}", e),
                            description: format!("The validation rule '{}' encountered an error", rule.name()),
                            remediation: "Check system logs and contact support".to_string(),
                            impact: "Cannot validate this configuration section".to_string(),
                            auto_fixable: false,
                            compliance_impact: Some("Unknown compliance status".to_string()),
                        });
                    }
                }
            }
        }

        // Perform cross-section analysis
        self.perform_cross_section_analysis(&mut report, &config).await;

        // Generate assessments
        self.generate_security_assessment(&mut report, &config).await;
        self.generate_performance_assessment(&mut report, &config).await;
        self.generate_compliance_assessment(&mut report, &config).await;

        // Finalize report
        let validation_duration = validation_start.elapsed();
        report.finalize_validation(validation_duration);

        // Cache the result
        self.cache_validation_result(config_hash, &report).await;

        info!(
            duration_ms = validation_duration.as_millis(),
            is_valid = report.is_valid,
            critical_issues = report.critical_issues.len(),
            warnings = report.warnings.len(),
            "✅ Production configuration validation completed"
        );

        Ok(report)
    }

    /// Load configuration from various sources
    async fn load_configuration(&mut self) -> Result<()> {
        let mut config = AppConfiguration::default();

        // Load from environment variables
        self.load_from_environment(&mut config)?;

        // Load from config file if specified
        if let Ok(config_path) = std::env::var("CONFIG_FILE") {
            self.load_from_file(&mut config, &config_path).await?;
            self.config_path = Some(config_path);
        }

        // Apply environment-specific overrides
        self.apply_environment_overrides(&mut config)?;

        // Update the configuration
        let mut current_config = self.config.write().await;
        *current_config = config;

        Ok(())
    }

    /// Load configuration from environment variables
    fn load_from_environment(&self, config: &mut AppConfiguration) -> Result<()> {
        // This is a simplified version - in production you'd have comprehensive env var mapping
        if let Ok(host) = std::env::var("SERVER_HOST") {
            config.server.host = host;
        }
        if let Ok(port) = std::env::var("PORT") {
            config.server.port = port.parse()
                .map_err(|e| AppError::ConfigError(format!("Invalid PORT: {}", e)))?;
        }
        if let Ok(uri) = std::env::var("MONGODB_URI") {
            config.database.mongodb_uri = uri;
        }
        if let Ok(secret) = std::env::var("JWT_SECRET") {
            config.security.jwt.secret = secret;
        }

        Ok(())
    }

    /// Load configuration from file
    async fn load_from_file(&self, config: &mut AppConfiguration, path: &str) -> Result<()> {
        let content = tokio::fs::read_to_string(path).await
            .map_err(|e| AppError::ConfigError(format!("Failed to read config file {}: {}", path, e)))?;

        let file_config: AppConfiguration = if path.ends_with(".yaml") || path.ends_with(".yml") {
            serde_yaml::from_str(&content)
                .map_err(|e| AppError::ConfigError(format!("Invalid YAML config: {}", e)))?
        } else if path.ends_with(".json") {
            serde_json::from_str(&content)
                .map_err(|e| AppError::ConfigError(format!("Invalid JSON config: {}", e)))?
        } else {
            return Err(AppError::ConfigError("Config file must be .yaml, .yml, or .json".to_string()));
        };

        // Merge configurations (file takes precedence)
        *config = file_config;
        Ok(())
    }

    /// Apply environment-specific configuration overrides
    fn apply_environment_overrides(&self, config: &mut AppConfiguration) -> Result<()> {
        match self.environment.as_str() {
            "production" => {
                // Production-specific defaults
                config.security.session.secure_cookies = true;
                config.observability.logging.level = "warn".to_string();
                config.features.enable_experimental_features = false;
                
                // Remove wildcard CORS in production
                if config.security.cors.allowed_origins.contains(&"*".to_string()) {
                    config.security.cors.allowed_origins = vec!["https://yourdomain.com".to_string()];
                }
            }
            "staging" => {
                config.observability.logging.level = "info".to_string();
                config.features.enable_experimental_features = false;
            }
            "development" => {
                config.observability.logging.level = "debug".to_string();
                config.security.session.secure_cookies = false;
            }
            _ => {
                warn!("Unknown environment: {}, using default settings", self.environment);
            }
        }

        Ok(())
    }

    /// Calculate configuration hash for caching
    fn calculate_config_hash(&self, config: &AppConfiguration) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        format!("{:?}", config).hash(&mut hasher);
        self.environment.hash(&mut hasher);
        hasher.finish()
    }

    /// Check validation cache
    async fn check_validation_cache(&self, config_hash: u64) -> Option<CachedValidationResult> {
        let cache = self.validation_cache.read().await;
        if let Some(cached) = cache.results.get("main") {
            if cached.config_hash == config_hash {
                let age = chrono::Utc::now().signed_duration_since(cached.timestamp);
                if age.num_minutes() < 10 { // Cache for 10 minutes
                    return Some(cached.clone());
                }
            }
        }
        None
    }

    /// Cache validation result
    async fn cache_validation_result(&self, config_hash: u64, report: &ProductionValidationReport) {
        let mut cache = self.validation_cache.write().await;
        cache.results.insert("main".to_string(), CachedValidationResult {
            report: report.clone(),
            timestamp: chrono::Utc::now(),
            config_hash,
        });
    }

    /// Perform cross-section analysis to identify configuration conflicts
    async fn perform_cross_section_analysis(&self, report: &mut ProductionValidationReport, config: &AppConfiguration) {
        // Analyze relationships between different configuration sections
        
        // Database vs CI configuration
        if config.ci.max_concurrent_pipelines > config.database.connection_pool_size * 2 {
            report.add_warning(ValidationIssue {
                severity: IssueSeverity::Medium,
                category: "cross_section".to_string(),
                field_path: "ci.max_concurrent_pipelines".to_string(),
                message: "High concurrent pipelines may overwhelm database".to_string(),
                description: "The number of concurrent CI pipelines exceeds database connection capacity".to_string(),
                remediation: "Increase database connection pool size or reduce concurrent pipelines".to_string(),
                impact: "May cause database connection timeouts and pipeline failures".to_string(),
                auto_fixable: true,
                compliance_impact: None,
            });
        }

        // Security vs Performance trade-offs
        if config.security.rate_limiting.requests_per_minute > 1000 && config.server.max_connections < 500 {
            report.add_warning(ValidationIssue {
                severity: IssueSeverity::Medium,
                category: "cross_section".to_string(),
                field_path: "security.rate_limiting".to_string(),
                message: "Rate limiting may exceed server capacity".to_string(),
                description: "Rate limiting allows more requests than server can handle".to_string(),
                remediation: "Align rate limiting with server connection limits".to_string(),
                impact: "May cause server overload despite rate limiting".to_string(),
                auto_fixable: true,
                compliance_impact: None,
            });
        }
    }

    /// Generate security assessment
    async fn generate_security_assessment(&self, report: &mut ProductionValidationReport, config: &AppConfiguration) {
        let mut score = 100u8;
        let mut vulnerabilities = Vec::new();
        let recommendations = Vec::new();

        // Check encryption
        let encryption_status = if config.security.encryption.key.is_empty() {
            score -= 20;
            vulnerabilities.push(SecurityVulnerability {
                id: "SEC-001".to_string(),
                severity: IssueSeverity::High,
                title: "Missing encryption key".to_string(),
                description: "No encryption key configured for sensitive data".to_string(),
                cve_references: vec![],
                mitigation: "Configure a strong encryption key".to_string(),
            });
            "Not configured".to_string()
        } else {
            "Configured".to_string()
        };

        // Check JWT secret strength
        if config.security.jwt.secret.len() < 32 {
            score -= 15;
            vulnerabilities.push(SecurityVulnerability {
                id: "SEC-002".to_string(),
                severity: IssueSeverity::High,
                title: "Weak JWT secret".to_string(),
                description: "JWT secret is too short and may be vulnerable to brute force attacks".to_string(),
                cve_references: vec![],
                mitigation: "Use a JWT secret of at least 32 characters".to_string(),
            });
        }

        // Environment-specific security checks
        if self.environment == "production" {
            if !config.security.session.secure_cookies {
                score -= 25;
                vulnerabilities.push(SecurityVulnerability {
                    id: "SEC-003".to_string(),
                    severity: IssueSeverity::Critical,
                    title: "Insecure cookies in production".to_string(),
                    description: "Secure cookies are disabled in production environment".to_string(),
                    cve_references: vec![],
                    mitigation: "Enable secure cookies for production deployment".to_string(),
                });
            }

            if config.security.cors.allowed_origins.contains(&"*".to_string()) {
                score -= 20;
                vulnerabilities.push(SecurityVulnerability {
                    id: "SEC-004".to_string(),
                    severity: IssueSeverity::High,
                    title: "Wildcard CORS in production".to_string(),
                    description: "CORS allows all origins in production environment".to_string(),
                    cve_references: vec![],
                    mitigation: "Restrict CORS to specific trusted domains".to_string(),
                });
            }
        }

        report.security_assessment = SecurityAssessment {
            overall_score: score,
            encryption_status,
            authentication_status: "JWT configured".to_string(),
            authorization_status: if config.security.rbac.enabled { "RBAC enabled".to_string() } else { "Basic".to_string() },
            audit_status: if config.security.audit.enabled { "Enabled".to_string() } else { "Disabled".to_string() },
            vulnerabilities,
            recommendations,
        };
    }

    /// Generate performance assessment
    async fn generate_performance_assessment(&self, report: &mut ProductionValidationReport, config: &AppConfiguration) {
        let mut score = 100u8;
        let mut bottlenecks = Vec::new();
        let mut optimization_opportunities = Vec::new();

        // Check connection pool sizing
        if config.database.connection_pool_size < 10 {
            score -= 10;
            bottlenecks.push(PerformanceBottleneck {
                component: "Database".to_string(),
                issue: "Small connection pool".to_string(),
                impact: "May cause connection timeouts under load".to_string(),
                solution: "Increase connection pool size to at least 10".to_string(),
            });
        }

        // Check server configuration
        if config.server.max_connections < 1000 {
            optimization_opportunities.push(OptimizationOpportunity {
                area: "Server Capacity".to_string(),
                current_state: format!("Max connections: {}", config.server.max_connections),
                recommended_state: "Max connections: 1000+".to_string(),
                expected_improvement: "Better handling of concurrent requests".to_string(),
            });
        }

        report.performance_assessment = PerformanceAssessment {
            overall_score: score,
            bottlenecks,
            optimization_opportunities,
            resource_utilization: ResourceUtilization {
                cpu_efficiency: 85,
                memory_efficiency: 80,
                network_efficiency: 90,
                storage_efficiency: 75,
            },
        };
    }

    /// Generate compliance assessment
    async fn generate_compliance_assessment(&self, report: &mut ProductionValidationReport, config: &AppConfiguration) {
        let mut standards = HashMap::new();
        let mut violations = Vec::new();

        // GDPR compliance check
        let gdpr_controls = vec![
            "data_encryption".to_string(),
            "audit_logging".to_string(),
            "user_consent".to_string(),
            "data_retention".to_string(),
        ];
        
        let mut implemented_gdpr = Vec::new();
        
        if !config.security.encryption.key.is_empty() {
            implemented_gdpr.push("data_encryption".to_string());
        }
        
        if config.security.audit.enabled {
            implemented_gdpr.push("audit_logging".to_string());
        }

        let missing_gdpr: Vec<String> = gdpr_controls.iter()
            .filter(|control| !implemented_gdpr.contains(control))
            .cloned()
            .collect();

        for missing in &missing_gdpr {
            violations.push(ComplianceViolation {
                standard: "GDPR".to_string(),
                control: missing.clone(),
                description: format!("Missing GDPR control: {}", missing),
                severity: IssueSeverity::High,
                remediation: format!("Implement {} control", missing),
            });
        }

        standards.insert("GDPR".to_string(), ComplianceStandard {
            name: "GDPR".to_string(),
            version: "2018".to_string(),
            compliance_percentage: ((implemented_gdpr.len() * 100) / gdpr_controls.len()) as u8,
            required_controls: gdpr_controls,
            implemented_controls: implemented_gdpr,
            missing_controls: missing_gdpr,
        });

        let overall_compliance = standards.values()
            .map(|s| s.compliance_percentage as u32)
            .sum::<u32>() / standards.len() as u32;

        report.compliance_status = ComplianceStatus {
            overall_compliance: overall_compliance as u8,
            standards,
            violations,
        };
    }

    /// Get current configuration
    pub async fn get_config(&self) -> AppConfiguration {
        self.config.read().await.clone()
    }
}

impl ProductionValidationReport {
    pub fn new(environment: &str) -> Self {
        Self {
            is_valid: true,
            validation_sections: HashMap::new(),
            critical_issues: Vec::new(),
            warnings: Vec::new(),
            recommendations: Vec::new(),
            environment_specific_issues: Vec::new(),
            security_assessment: SecurityAssessment {
                overall_score: 0,
                encryption_status: "Unknown".to_string(),
                authentication_status: "Unknown".to_string(),
                authorization_status: "Unknown".to_string(),
                audit_status: "Unknown".to_string(),
                vulnerabilities: Vec::new(),
                recommendations: Vec::new(),
            },
            performance_assessment: PerformanceAssessment {
                overall_score: 0,
                bottlenecks: Vec::new(),
                optimization_opportunities: Vec::new(),
                resource_utilization: ResourceUtilization {
                    cpu_efficiency: 0,
                    memory_efficiency: 0,
                    network_efficiency: 0,
                    storage_efficiency: 0,
                },
            },
            compliance_status: ComplianceStatus {
                overall_compliance: 0,
                standards: HashMap::new(),
                violations: Vec::new(),
            },
            validation_metadata: ValidationMetadata {
                validation_timestamp: chrono::Utc::now(),
                validation_duration: std::time::Duration::from_secs(0),
                validator_version: "1.0.0".to_string(),
                environment: environment.to_string(),
                config_source: "unknown".to_string(),
                validation_rules_applied: Vec::new(),
            },
        }
    }

    pub fn add_validation_section(&mut self, section: ValidationSection) {
        // Update overall validity based on section status
        if matches!(section.status, ValidationStatus::Error | ValidationStatus::Critical) {
            self.is_valid = false;
        }

        // Extract issues from section
        for issue in &section.issues {
            match issue.severity {
                IssueSeverity::Critical => {
                    self.critical_issues.push(issue.clone());
                    self.is_valid = false;
                }
                IssueSeverity::High | IssueSeverity::Medium | IssueSeverity::Low => {
                    self.warnings.push(issue.clone());
                }
                IssueSeverity::Info => {
                    // Info issues don't affect validity
                }
            }
        }

        self.validation_sections.insert(section.name.clone(), section);
    }

    pub fn add_critical_issue(&mut self, issue: ValidationIssue) {
        self.is_valid = false;
        self.critical_issues.push(issue);
    }

    pub fn add_warning(&mut self, issue: ValidationIssue) {
        self.warnings.push(issue);
    }

    pub fn finalize_validation(&mut self, duration: std::time::Duration) {
        self.validation_metadata.validation_duration = duration;
        self.validation_metadata.validation_rules_applied = self.validation_sections.keys().cloned().collect();
    }
}

// Placeholder validation rules - these would be implemented with actual validation logic
struct SecurityValidationRule;
impl SecurityValidationRule {
    fn new() -> Self { Self }
}
impl ProductionValidationRule for SecurityValidationRule {
    fn validate(&self, config: &AppConfiguration, _environment: &str) -> Result<ValidationSection> {
        let mut section = ValidationSection {
            name: "security".to_string(),
            status: ValidationStatus::Pass,
            issues: Vec::new(),
            score: 100,
            details: HashMap::new(),
        };

        // Add actual security validation logic here
        if config.security.jwt.secret.len() < 32 {
            section.issues.push(ValidationIssue {
                severity: IssueSeverity::High,
                category: "security".to_string(),
                field_path: "security.jwt.secret".to_string(),
                message: "JWT secret too short".to_string(),
                description: "JWT secret should be at least 32 characters for security".to_string(),
                remediation: "Generate a longer JWT secret".to_string(),
                impact: "Vulnerable to brute force attacks".to_string(),
                auto_fixable: false,
                compliance_impact: Some("GDPR compliance risk".to_string()),
            });
            section.status = ValidationStatus::Error;
            section.score -= 20;
        }

        Ok(section)
    }
    fn name(&self) -> &str { "security" }
    fn category(&self) -> &str { "security" }
    fn priority(&self) -> u8 { 1 }
}

// Additional placeholder validation rules
struct DatabaseValidationRule;
impl DatabaseValidationRule { fn new() -> Self { Self } }
impl ProductionValidationRule for DatabaseValidationRule {
    fn validate(&self, _config: &AppConfiguration, _environment: &str) -> Result<ValidationSection> {
        Ok(ValidationSection {
            name: "database".to_string(),
            status: ValidationStatus::Pass,
            issues: Vec::new(),
            score: 100,
            details: HashMap::new(),
        })
    }
    fn name(&self) -> &str { "database" }
    fn category(&self) -> &str { "infrastructure" }
    fn priority(&self) -> u8 { 2 }
}

struct PerformanceValidationRule;
impl PerformanceValidationRule { fn new() -> Self { Self } }
impl ProductionValidationRule for PerformanceValidationRule {
    fn validate(&self, _config: &AppConfiguration, _environment: &str) -> Result<ValidationSection> {
        Ok(ValidationSection {
            name: "performance".to_string(),
            status: ValidationStatus::Pass,
            issues: Vec::new(),
            score: 100,
            details: HashMap::new(),
        })
    }
    fn name(&self) -> &str { "performance" }
    fn category(&self) -> &str { "performance" }
    fn priority(&self) -> u8 { 3 }
}

struct ObservabilityValidationRule;
impl ObservabilityValidationRule { fn new() -> Self { Self } }
impl ProductionValidationRule for ObservabilityValidationRule {
    fn validate(&self, _config: &AppConfiguration, _environment: &str) -> Result<ValidationSection> {
        Ok(ValidationSection {
            name: "observability".to_string(),
            status: ValidationStatus::Pass,
            issues: Vec::new(),
            score: 100,
            details: HashMap::new(),
        })
    }
    fn name(&self) -> &str { "observability" }
    fn category(&self) -> &str { "monitoring" }
    fn priority(&self) -> u8 { 4 }
}

struct ComplianceValidationRule;
impl ComplianceValidationRule { fn new() -> Self { Self } }
impl ProductionValidationRule for ComplianceValidationRule {
    fn validate(&self, _config: &AppConfiguration, _environment: &str) -> Result<ValidationSection> {
        Ok(ValidationSection {
            name: "compliance".to_string(),
            status: ValidationStatus::Pass,
            issues: Vec::new(),
            score: 100,
            details: HashMap::new(),
        })
    }
    fn name(&self) -> &str { "compliance" }
    fn category(&self) -> &str { "compliance" }
    fn priority(&self) -> u8 { 5 }
}

struct EnvironmentValidationRule;
impl EnvironmentValidationRule { fn new() -> Self { Self } }
impl ProductionValidationRule for EnvironmentValidationRule {
    fn validate(&self, _config: &AppConfiguration, _environment: &str) -> Result<ValidationSection> {
        Ok(ValidationSection {
            name: "environment".to_string(),
            status: ValidationStatus::Pass,
            issues: Vec::new(),
            score: 100,
            details: HashMap::new(),
        })
    }
    fn name(&self) -> &str { "environment" }
    fn category(&self) -> &str { "environment" }
    fn priority(&self) -> u8 { 6 }
}

struct ResourceValidationRule;
impl ResourceValidationRule { fn new() -> Self { Self } }
impl ProductionValidationRule for ResourceValidationRule {
    fn validate(&self, _config: &AppConfiguration, _environment: &str) -> Result<ValidationSection> {
        Ok(ValidationSection {
            name: "resource".to_string(),
            status: ValidationStatus::Pass,
            issues: Vec::new(),
            score: 100,
            details: HashMap::new(),
        })
    }
    fn name(&self) -> &str { "resource" }
    fn category(&self) -> &str { "resource" }
    fn priority(&self) -> u8 { 7 }
}

struct NetworkValidationRule;
impl NetworkValidationRule { fn new() -> Self { Self } }
impl ProductionValidationRule for NetworkValidationRule {
    fn validate(&self, _config: &AppConfiguration, _environment: &str) -> Result<ValidationSection> {
        Ok(ValidationSection {
            name: "network".to_string(),
            status: ValidationStatus::Pass,
            issues: Vec::new(),
            score: 100,
            details: HashMap::new(),
        })
    }
    fn name(&self) -> &str { "network" }
    fn category(&self) -> &str { "network" }
    fn priority(&self) -> u8 { 8 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfiguration, DatabaseConfig, SecurityConfig, JwtConfig, EncryptionConfig, SessionConfig, CorsConfig};
    use std::collections::HashMap;
    use tokio;

    fn create_test_config() -> AppConfiguration {
        let mut config = AppConfiguration::default();
        config.security.jwt.secret = "test_secret_that_is_long_enough_for_validation".to_string();
        config.security.encryption.key = "dGVzdF9lbmNyeXB0aW9uX2tleV90aGF0X2lzXzMyX2J5dGVz".to_string(); // base64 encoded 32 bytes
        config
    }

    fn create_insecure_config() -> AppConfiguration {
        let mut config = AppConfiguration::default();
        config.security.jwt.secret = "short".to_string(); // Too short
        config.security.encryption.key = "".to_string(); // Empty
        config.security.session.secure_cookies = false;
        config.security.cors.allowed_origins = vec!["*".to_string()];
        config
    }

    #[tokio::test]
    async fn test_production_config_manager_creation() {
        let manager = ProductionConfigManager::new("production");
        assert_eq!(manager.environment, "production");
        assert_eq!(manager.validation_rules.len(), 8); // Should have 8 validation rules
    }

    #[tokio::test]
    async fn test_config_hash_calculation() {
        let manager = ProductionConfigManager::new("test");
        let config1 = create_test_config();
        let config2 = create_test_config();
        let config3 = create_insecure_config();

        let hash1 = manager.calculate_config_hash(&config1);
        let hash2 = manager.calculate_config_hash(&config2);
        let hash3 = manager.calculate_config_hash(&config3);

        assert_eq!(hash1, hash2); // Same config should produce same hash
        assert_ne!(hash1, hash3); // Different config should produce different hash
    }

    #[tokio::test]
    async fn test_validation_cache() {
        let manager = ProductionConfigManager::new("test");
        let config = create_test_config();
        let config_hash = manager.calculate_config_hash(&config);

        // Initially no cache
        let cached = manager.check_validation_cache(config_hash).await;
        assert!(cached.is_none());

        // Create a mock report and cache it
        let report = ProductionValidationReport::new("test");
        manager.cache_validation_result(config_hash, &report).await;

        // Should now find cached result
        let cached = manager.check_validation_cache(config_hash).await;
        assert!(cached.is_some());
    }

    #[tokio::test]
    async fn test_environment_overrides() {
        let manager = ProductionConfigManager::new("production");
        let mut config = create_test_config();
        
        // Apply production overrides
        manager.apply_environment_overrides(&mut config).unwrap();

        assert!(config.security.session.secure_cookies);
        assert_eq!(config.observability.logging.level, "warn");
        assert!(!config.features.enable_experimental_features);
        assert!(!config.security.cors.allowed_origins.contains(&"*".to_string()));
    }

    #[tokio::test]
    async fn test_security_validation_rule() {
        let rule = SecurityValidationRule::new();
        let secure_config = create_test_config();
        let insecure_config = create_insecure_config();

        // Test secure configuration
        let result = rule.validate(&secure_config, "production").unwrap();
        assert_eq!(result.name, "security");
        assert!(matches!(result.status, ValidationStatus::Pass));
        assert!(result.issues.is_empty());
        assert_eq!(result.score, 100);

        // Test insecure configuration
        let result = rule.validate(&insecure_config, "production").unwrap();
        assert_eq!(result.name, "security");
        assert!(matches!(result.status, ValidationStatus::Error));
        assert!(!result.issues.is_empty());
        assert!(result.score < 100);

        // Check that JWT secret issue is detected
        let jwt_issue = result.issues.iter().find(|issue| issue.field_path == "security.jwt.secret");
        assert!(jwt_issue.is_some());
        assert!(matches!(jwt_issue.unwrap().severity, IssueSeverity::High));
    }

    #[tokio::test]
    async fn test_production_validation_report_creation() {
        let report = ProductionValidationReport::new("production");
        
        assert!(report.is_valid);
        assert_eq!(report.validation_metadata.environment, "production");
        assert!(report.validation_sections.is_empty());
        assert!(report.critical_issues.is_empty());
        assert!(report.warnings.is_empty());
    }

    #[tokio::test]
    async fn test_validation_report_issue_handling() {
        let mut report = ProductionValidationReport::new("test");
        
        // Add a critical issue
        let critical_issue = ValidationIssue {
            severity: IssueSeverity::Critical,
            category: "security".to_string(),
            field_path: "test.field".to_string(),
            message: "Critical security issue".to_string(),
            description: "This is a critical security issue".to_string(),
            remediation: "Fix immediately".to_string(),
            impact: "System compromise".to_string(),
            auto_fixable: false,
            compliance_impact: Some("GDPR violation".to_string()),
        };

        report.add_critical_issue(critical_issue.clone());
        
        assert!(!report.is_valid); // Should be invalid due to critical issue
        assert_eq!(report.critical_issues.len(), 1);
        assert_eq!(report.critical_issues[0].message, "Critical security issue");

        // Add a warning
        let warning_issue = ValidationIssue {
            severity: IssueSeverity::Medium,
            category: "performance".to_string(),
            field_path: "test.performance".to_string(),
            message: "Performance warning".to_string(),
            description: "This is a performance warning".to_string(),
            remediation: "Optimize configuration".to_string(),
            impact: "Reduced performance".to_string(),
            auto_fixable: true,
            compliance_impact: None,
        };

        report.add_warning(warning_issue.clone());
        
        assert_eq!(report.warnings.len(), 1);
        assert_eq!(report.warnings[0].message, "Performance warning");
    }
}