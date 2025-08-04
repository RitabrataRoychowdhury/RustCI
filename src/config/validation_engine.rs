use crate::config::{enhanced_validation::*, AppConfiguration};
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

/// Comprehensive configuration validation engine
pub struct ConfigValidationEngine {
    validators: HashMap<String, Box<dyn ConfigValidator>>,
    validation_cache: Arc<RwLock<ValidationCache>>,
    environment: String,
}

#[derive(Debug, Clone)]
struct ValidationCache {
    results: HashMap<String, CachedValidationResult>,
}

#[derive(Debug, Clone)]
pub struct CachedValidationResult {
    result: ValidationReport,
    timestamp: chrono::DateTime<chrono::Utc>,
    config_hash: u64,
}

impl ConfigValidationEngine {
    pub fn new(environment: &str) -> Self {
        let mut engine = Self {
            validators: HashMap::new(),
            validation_cache: Arc::new(RwLock::new(ValidationCache {
                results: HashMap::new(),
            })),
            environment: environment.to_string(),
        };

        // Register default validators
        engine.register_default_validators();
        engine
    }

    fn register_default_validators(&mut self) {
        self.register_validator(
            "security",
            Box::new(SecurityConfigValidator::new(&self.environment)),
        );
        self.register_validator("database", Box::new(DatabaseConfigValidator));
        self.register_validator("server", Box::new(ServerConfigValidator));
        self.register_validator("observability", Box::new(ObservabilityConfigValidator));
        self.register_validator("ci", Box::new(CIConfigValidator));
        self.register_validator(
            "features",
            Box::new(FeatureFlagValidator::new(&self.environment)),
        );
        self.register_validator("external_services", Box::new(ExternalServicesValidator));
    }

    pub fn register_validator(&mut self, name: &str, validator: Box<dyn ConfigValidator>) {
        self.validators.insert(name.to_string(), validator);
    }

    /// Validate configuration with caching
    pub async fn validate(
        &self,
        config: &AppConfiguration,
    ) -> Result<ComprehensiveValidationReport> {
        let config_hash = self.calculate_config_hash(config);

        // Check cache first
        if let Some(cached) = self.check_cache(config_hash).await {
            debug!("Using cached validation result");
            return Ok(ComprehensiveValidationReport::from_cached(cached));
        }

        debug!("🔍 Starting comprehensive configuration validation");
        let start_time = std::time::Instant::now();

        let mut comprehensive_report = ComprehensiveValidationReport::new();
        let mut validation_futures = Vec::new();

        // Run all validators concurrently
        for (name, validator) in &self.validators {
            let validator_name = name.clone();
            let config_clone = config.clone();
            let validator_ref = validator.as_ref();

            let future = async move {
                let result = validator_ref.validate(&config_clone).await;
                (validator_name, result)
            };

            validation_futures.push(future);
        }

        // Collect results
        let results = futures::future::join_all(validation_futures).await;

        for (validator_name, result) in results {
            match result {
                Ok(report) => {
                    comprehensive_report.add_validator_report(validator_name, report);
                }
                Err(e) => {
                    error!("Validator '{}' failed: {}", validator_name, e);
                    comprehensive_report.add_validator_error(validator_name, e.to_string());
                }
            }
        }

        // Add environment-specific validations
        self.add_environment_validations(&mut comprehensive_report, config)
            .await;

        // Add cross-validator checks
        self.add_cross_validator_checks(&mut comprehensive_report, config)
            .await;

        let duration = start_time.elapsed();
        comprehensive_report.set_validation_duration(duration);

        // Cache the result
        self.cache_result(config_hash, &comprehensive_report).await;

        info!(
            duration_ms = duration.as_millis(),
            errors = comprehensive_report.total_errors(),
            warnings = comprehensive_report.total_warnings(),
            "✅ Comprehensive configuration validation completed"
        );

        Ok(comprehensive_report)
    }

    async fn add_environment_validations(
        &self,
        report: &mut ComprehensiveValidationReport,
        config: &AppConfiguration,
    ) {
        let mut env_report = ValidationReport::new();

        match self.environment.as_str() {
            "production" => {
                // Production-specific validations
                if config.observability.logging.level == "debug"
                    || config.observability.logging.level == "trace"
                {
                    env_report.add_warning(
                        "logging_level".to_string(),
                        "Debug/trace logging in production may impact performance".to_string(),
                    );
                }

                if config.features.enable_experimental_features {
                    env_report.add_error(
                        "experimental_features".to_string(),
                        "Experimental features should not be enabled in production".to_string(),
                    );
                }

                if !config.security.session.secure_cookies {
                    env_report.add_error(
                        "secure_cookies".to_string(),
                        "Secure cookies must be enabled in production".to_string(),
                    );
                }

                if config
                    .security
                    .cors
                    .allowed_origins
                    .contains(&"*".to_string())
                {
                    env_report.add_error(
                        "cors_wildcard".to_string(),
                        "Wildcard CORS origins not allowed in production".to_string(),
                    );
                }
            }
            "staging" => {
                // Staging-specific validations
                if config.features.enable_experimental_features {
                    env_report.add_warning(
                        "experimental_features".to_string(),
                        "Experimental features enabled in staging - ensure thorough testing"
                            .to_string(),
                    );
                }
            }
            "development" => {
                // Development-specific validations
                if !config.features.enable_hot_reload {
                    env_report.add_info(
                        "hot_reload".to_string(),
                        "Hot reload disabled in development environment".to_string(),
                    );
                }

                if config.security.session.secure_cookies {
                    env_report.add_info(
                        "secure_cookies".to_string(),
                        "Secure cookies enabled in development (may cause issues with HTTP)"
                            .to_string(),
                    );
                }
            }
            _ => {
                env_report.add_warning(
                    "unknown_environment".to_string(),
                    format!("Unknown environment: {}", self.environment),
                );
            }
        }

        report.add_validator_report("environment".to_string(), env_report);
    }

    async fn add_cross_validator_checks(
        &self,
        report: &mut ComprehensiveValidationReport,
        config: &AppConfiguration,
    ) {
        let mut cross_report = ValidationReport::new();

        // Check consistency between related configurations

        // Database and CI configuration consistency
        if config.ci.max_concurrent_pipelines > config.database.connection_pool_size * 2 {
            cross_report.add_warning(
                "db_ci_consistency".to_string(),
                "High concurrent pipelines may overwhelm database connection pool".to_string(),
            );
        }

        // Security and server configuration consistency
        if config.server.port < 1024 && self.environment != "development" {
            cross_report.add_warning(
                "privileged_port".to_string(),
                "Using privileged port may require elevated permissions".to_string(),
            );
        }

        // Observability and performance consistency
        if config.observability.metrics.enabled
            && config.observability.metrics.collection_interval_seconds < 10
        {
            cross_report.add_warning(
                "metrics_frequency".to_string(),
                "Very frequent metrics collection may impact performance".to_string(),
            );
        }

        // Rate limiting and server capacity consistency
        let total_rate_limit = config.security.rate_limiting.requests_per_minute * 60; // per hour
        let server_capacity_estimate = config.server.max_connections * 100; // rough estimate

        if total_rate_limit > server_capacity_estimate {
            cross_report.add_warning(
                "rate_limit_capacity".to_string(),
                "Rate limits may exceed estimated server capacity".to_string(),
            );
        }

        report.add_validator_report("cross_validation".to_string(), cross_report);
    }

    fn calculate_config_hash(&self, config: &AppConfiguration) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        // In a real implementation, you'd serialize the config and hash it
        // For now, we'll use a simple approach
        format!("{:?}", config).hash(&mut hasher);
        hasher.finish()
    }

    async fn check_cache(&self, config_hash: u64) -> Option<CachedValidationResult> {
        let cache = self.validation_cache.read().await;
        if let Some(cached) = cache.results.get("main") {
            if cached.config_hash == config_hash {
                let age = chrono::Utc::now().signed_duration_since(cached.timestamp);
                if age.num_minutes() < 5 {
                    // Cache for 5 minutes
                    return Some(cached.clone());
                }
            }
        }
        None
    }

    async fn cache_result(&self, config_hash: u64, report: &ComprehensiveValidationReport) {
        let mut cache = self.validation_cache.write().await;
        cache.results.insert(
            "main".to_string(),
            CachedValidationResult {
                result: report.summary_report(),
                timestamp: chrono::Utc::now(),
                config_hash,
            },
        );
    }
}

/// Comprehensive validation report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComprehensiveValidationReport {
    pub validator_reports: HashMap<String, ValidationReport>,
    pub validation_duration: Option<std::time::Duration>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub environment: String,
}

impl Default for ComprehensiveValidationReport {
    fn default() -> Self {
        Self::new()
    }
}

impl ComprehensiveValidationReport {
    pub fn new() -> Self {
        Self {
            validator_reports: HashMap::new(),
            validation_duration: None,
            timestamp: chrono::Utc::now(),
            environment: std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
        }
    }

    pub fn from_cached(cached: CachedValidationResult) -> Self {
        let mut report = Self::new();
        report
            .validator_reports
            .insert("cached".to_string(), cached.result);
        report.timestamp = cached.timestamp;
        report
    }

    pub fn add_validator_report(&mut self, validator_name: String, report: ValidationReport) {
        self.validator_reports.insert(validator_name, report);
    }

    pub fn add_validator_error(&mut self, validator_name: String, error: String) {
        let mut error_report = ValidationReport::new();
        error_report.add_error("validation_error".to_string(), error);
        self.validator_reports.insert(validator_name, error_report);
    }

    pub fn set_validation_duration(&mut self, duration: std::time::Duration) {
        self.validation_duration = Some(duration);
    }

    pub fn is_valid(&self) -> bool {
        self.validator_reports
            .values()
            .all(|report| report.is_valid())
    }

    pub fn total_errors(&self) -> usize {
        self.validator_reports
            .values()
            .map(|report| report.errors.len())
            .sum()
    }

    pub fn total_warnings(&self) -> usize {
        self.validator_reports
            .values()
            .map(|report| report.warnings.len())
            .sum()
    }

    pub fn total_info(&self) -> usize {
        self.validator_reports
            .values()
            .map(|report| report.info.len())
            .sum()
    }

    pub fn summary(&self) -> String {
        format!(
            "Validation Summary: {} validators, {} errors, {} warnings, {} info messages",
            self.validator_reports.len(),
            self.total_errors(),
            self.total_warnings(),
            self.total_info()
        )
    }

    pub fn summary_report(&self) -> ValidationReport {
        let mut summary = ValidationReport::new();

        for (validator_name, report) in &self.validator_reports {
            for (error_key, error_msg) in &report.errors {
                summary.add_error(
                    format!("{}:{}", validator_name, error_key),
                    error_msg.clone(),
                );
            }
            for (warning_key, warning_msg) in &report.warnings {
                summary.add_warning(
                    format!("{}:{}", validator_name, warning_key),
                    warning_msg.clone(),
                );
            }
            for (info_key, info_msg) in &report.info {
                summary.add_info(format!("{}:{}", validator_name, info_key), info_msg.clone());
            }
            for success in &report.successes {
                summary.add_success(format!("{}:{}", validator_name, success));
            }
        }

        summary
    }
}

/// Enhanced configuration validator trait
#[async_trait::async_trait]
pub trait ConfigValidator: Send + Sync {
    async fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport>;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn priority(&self) -> u8 {
        100
    } // Lower number = higher priority
}

/// Security configuration validator
pub struct SecurityConfigValidator {
    environment: String,
}

impl SecurityConfigValidator {
    pub fn new(environment: &str) -> Self {
        Self {
            environment: environment.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl ConfigValidator for SecurityConfigValidator {
    async fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport> {
        let security_rule = SecurityValidationRule::new(&self.environment);
        security_rule.validate(config)
    }

    fn name(&self) -> &'static str {
        "security"
    }

    fn description(&self) -> &'static str {
        "Validates security configuration including JWT, encryption, OAuth, and RBAC"
    }

    fn priority(&self) -> u8 {
        10 // High priority
    }
}

/// Database configuration validator
pub struct DatabaseConfigValidator;

#[async_trait::async_trait]
impl ConfigValidator for DatabaseConfigValidator {
    async fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport> {
        let db_rule = DatabaseValidationRule;
        db_rule.validate(config)
    }

    fn name(&self) -> &'static str {
        "database"
    }

    fn description(&self) -> &'static str {
        "Validates database configuration including connection settings and timeouts"
    }

    fn priority(&self) -> u8 {
        20 // High priority
    }
}

/// Server configuration validator
pub struct ServerConfigValidator;

#[async_trait::async_trait]
impl ConfigValidator for ServerConfigValidator {
    async fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport> {
        let server_rule = ServerValidationRule;
        server_rule.validate(config)
    }

    fn name(&self) -> &'static str {
        "server"
    }

    fn description(&self) -> &'static str {
        "Validates server configuration including port, host, and connection limits"
    }
}

/// Observability configuration validator
pub struct ObservabilityConfigValidator;

#[async_trait::async_trait]
impl ConfigValidator for ObservabilityConfigValidator {
    async fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport> {
        let obs_rule = ObservabilityValidationRule;
        obs_rule.validate(config)
    }

    fn name(&self) -> &'static str {
        "observability"
    }

    fn description(&self) -> &'static str {
        "Validates observability configuration including logging, metrics, and tracing"
    }
}

/// CI configuration validator with tiered pipeline support
pub struct CIConfigValidator;

#[async_trait::async_trait]
impl ConfigValidator for CIConfigValidator {
    async fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport> {
        let ci_rule = CIValidationRule;
        ci_rule.validate(config)
    }

    fn name(&self) -> &'static str {
        "ci"
    }

    fn description(&self) -> &'static str {
        "Validates CI configuration including pipelines and executors"
    }
}

/// Tiered pipeline validator for multi-tier pipeline system
pub struct TieredPipelineValidator;

impl TieredPipelineValidator {
    pub fn new() -> Self {
        Self
    }

    /// Validate pipeline based on its detected or specified type
    pub fn validate(&self, pipeline: &crate::ci::config::CIPipeline) -> Result<ValidationReport> {
        let pipeline_type = pipeline.get_pipeline_type();
        
        match pipeline_type {
            crate::ci::config::PipelineType::Minimal => self.validate_minimal(pipeline),
            crate::ci::config::PipelineType::Simple => self.validate_simple(pipeline),
            crate::ci::config::PipelineType::Standard => self.validate_standard(pipeline),
            crate::ci::config::PipelineType::Advanced => self.validate_advanced(pipeline),
        }
    }

    /// Validate minimal pipeline - requires only repo and server fields
    fn validate_minimal(&self, pipeline: &crate::ci::config::CIPipeline) -> Result<ValidationReport> {
        let mut report = ValidationReport::new();

        // Check required fields for minimal pipeline
        if pipeline.repo.is_none() {
            report.add_error(
                "repo".to_string(),
                "Minimal pipeline requires 'repo' field".to_string(),
            );
        } else {
            report.add_success("repo".to_string());
        }

        if pipeline.server.is_none() {
            report.add_error(
                "server".to_string(),
                "Minimal pipeline requires 'server' field".to_string(),
            );
        } else {
            // Validate server configuration
            match pipeline.server.as_ref().unwrap() {
                crate::ci::config::ServerConfig::Simple(server_str) => {
                    if server_str.is_empty() {
                        report.add_error(
                            "server".to_string(),
                            "Server configuration cannot be empty".to_string(),
                        );
                    } else {
                        report.add_success("server".to_string());
                    }
                }
                crate::ci::config::ServerConfig::Detailed { host, .. } => {
                    if host.is_empty() {
                        report.add_error(
                            "server".to_string(),
                            "Server host cannot be empty".to_string(),
                        );
                    } else {
                        report.add_success("server".to_string());
                    }
                }
            }
        }

        // Optional branch validation
        if let Some(branch) = &pipeline.branch {
            if branch.is_empty() {
                report.add_warning(
                    "branch".to_string(),
                    "Branch field is empty, will default to 'main'".to_string(),
                );
            }
        }

        // Warn about unused fields in minimal pipeline
        if pipeline.steps.is_some() {
            report.add_warning(
                "steps".to_string(),
                "Steps field ignored in minimal pipeline".to_string(),
            );
        }
        if pipeline.jobs.is_some() {
            report.add_warning(
                "jobs".to_string(),
                "Jobs field ignored in minimal pipeline".to_string(),
            );
        }
        if !pipeline.stages.is_empty() {
            report.add_warning(
                "stages".to_string(),
                "Stages field ignored in minimal pipeline".to_string(),
            );
        }

        Ok(report)
    }

    /// Validate simple pipeline - requires non-empty steps array
    fn validate_simple(&self, pipeline: &crate::ci::config::CIPipeline) -> Result<ValidationReport> {
        let mut report = ValidationReport::new();

        // Check required steps field
        if let Some(steps) = &pipeline.steps {
            if steps.is_empty() {
                report.add_error(
                    "steps".to_string(),
                    "Simple pipeline must have at least one step".to_string(),
                );
            } else {
                // Validate each step
                for (i, step) in steps.iter().enumerate() {
                    match step {
                        crate::ci::config::SimpleStep::Command(cmd) => {
                            if cmd.is_empty() {
                                report.add_error(
                                    format!("step_{}", i),
                                    "Step command cannot be empty".to_string(),
                                );
                            }
                        }
                        crate::ci::config::SimpleStep::Detailed { run, .. } => {
                            if run.is_empty() {
                                report.add_error(
                                    format!("step_{}", i),
                                    "Step run command cannot be empty".to_string(),
                                );
                            }
                        }
                    }
                }
                report.add_success("steps".to_string());
            }
        } else {
            report.add_error(
                "steps".to_string(),
                "Simple pipeline requires 'steps' field".to_string(),
            );
        }

        // Warn about unused fields in simple pipeline
        if pipeline.jobs.is_some() {
            report.add_warning(
                "jobs".to_string(),
                "Jobs field ignored in simple pipeline".to_string(),
            );
        }
        if !pipeline.stages.is_empty() {
            report.add_warning(
                "stages".to_string(),
                "Stages field ignored in simple pipeline".to_string(),
            );
        }
        if pipeline.matrix.is_some() {
            report.add_warning(
                "matrix".to_string(),
                "Matrix field ignored in simple pipeline".to_string(),
            );
        }

        Ok(report)
    }

    /// Validate standard pipeline - requires stages and jobs mappings
    fn validate_standard(&self, pipeline: &crate::ci::config::CIPipeline) -> Result<ValidationReport> {
        let mut report = ValidationReport::new();

        // Check pipeline name
        if pipeline.name.is_empty() {
            report.add_error(
                "name".to_string(),
                "Pipeline name cannot be empty".to_string(),
            );
        } else {
            report.add_success("name".to_string());
        }

        // Check stages
        if pipeline.stages.is_empty() {
            report.add_error(
                "stages".to_string(),
                "Standard pipeline must have at least one stage".to_string(),
            );
        } else {
            // Validate each stage
            for (i, stage) in pipeline.stages.iter().enumerate() {
                if stage.name.is_empty() {
                    report.add_error(
                        format!("stage_{}", i),
                        "Stage name cannot be empty".to_string(),
                    );
                }

                if stage.steps.is_empty() {
                    report.add_error(
                        format!("stage_{}_steps", stage.name),
                        format!("Stage '{}' must have at least one step", stage.name),
                    );
                }

                // Validate steps within stage
                for (j, step) in stage.steps.iter().enumerate() {
                    if step.name.is_empty() {
                        report.add_error(
                            format!("stage_{}_step_{}", stage.name, j),
                            format!("Step {} in stage '{}' name cannot be empty", j, stage.name),
                        );
                    }
                }
            }
            report.add_success("stages".to_string());
        }

        // Check jobs mapping if present
        if let Some(jobs) = &pipeline.jobs {
            if jobs.is_empty() {
                report.add_warning(
                    "jobs".to_string(),
                    "Jobs mapping is empty".to_string(),
                );
            } else {
                // Validate job-stage relationships
                let stage_names: std::collections::HashSet<_> = 
                    pipeline.stages.iter().map(|s| &s.name).collect();

                for (job_name, job) in jobs {
                    match job {
                        crate::ci::config::PipelineJob::Simple(_) => {
                            // Simple jobs don't specify stages, that's ok
                        }
                        crate::ci::config::PipelineJob::Detailed { stage, script, .. } => {
                            if !stage_names.contains(stage) {
                                report.add_error(
                                    format!("job_{}_stage", job_name),
                                    format!("Job '{}' references non-existent stage '{}'", job_name, stage),
                                );
                            }
                            
                            // Validate script
                            match script {
                                crate::ci::config::JobScript::Single(cmd) => {
                                    if cmd.is_empty() {
                                        report.add_error(
                                            format!("job_{}_script", job_name),
                                            format!("Job '{}' script cannot be empty", job_name),
                                        );
                                    }
                                }
                                crate::ci::config::JobScript::Multiple(cmds) => {
                                    if cmds.is_empty() {
                                        report.add_error(
                                            format!("job_{}_script", job_name),
                                            format!("Job '{}' script array cannot be empty", job_name),
                                        );
                                    }
                                    for (i, cmd) in cmds.iter().enumerate() {
                                        if cmd.is_empty() {
                                            report.add_error(
                                                format!("job_{}_script_{}", job_name, i),
                                                format!("Job '{}' script command {} cannot be empty", job_name, i),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                report.add_success("jobs".to_string());
            }
        }

        // Warn about advanced features in standard pipeline
        if pipeline.matrix.is_some() {
            report.add_warning(
                "matrix".to_string(),
                "Matrix configuration may not be fully supported in standard pipeline".to_string(),
            );
        }
        if pipeline.variables.is_some() {
            report.add_warning(
                "variables".to_string(),
                "Variables may not be fully supported in standard pipeline".to_string(),
            );
        }

        Ok(report)
    }

    /// Validate advanced pipeline - accepts all extended keys
    fn validate_advanced(&self, pipeline: &crate::ci::config::CIPipeline) -> Result<ValidationReport> {
        let mut report = ValidationReport::new();

        // Check pipeline name
        if pipeline.name.is_empty() {
            report.add_error(
                "name".to_string(),
                "Pipeline name cannot be empty".to_string(),
            );
        } else {
            report.add_success("name".to_string());
        }

        // Validate jobs if present
        if let Some(jobs) = &pipeline.jobs {
            if jobs.is_empty() {
                report.add_error(
                    "jobs".to_string(),
                    "Advanced pipeline with jobs must have at least one job".to_string(),
                );
            } else {
                // Validate each job
                for (job_name, job) in jobs {
                    match job {
                        crate::ci::config::PipelineJob::Simple(script) => {
                            match script {
                                crate::ci::config::JobScript::Single(cmd) => {
                                    if cmd.is_empty() {
                                        report.add_error(
                                            format!("job_{}", job_name),
                                            format!("Job '{}' script cannot be empty", job_name),
                                        );
                                    }
                                }
                                crate::ci::config::JobScript::Multiple(cmds) => {
                                    if cmds.is_empty() {
                                        report.add_error(
                                            format!("job_{}", job_name),
                                            format!("Job '{}' script array cannot be empty", job_name),
                                        );
                                    }
                                }
                            }
                        }
                        crate::ci::config::PipelineJob::Detailed { script, .. } => {
                            match script {
                                crate::ci::config::JobScript::Single(cmd) => {
                                    if cmd.is_empty() {
                                        report.add_error(
                                            format!("job_{}", job_name),
                                            format!("Job '{}' script cannot be empty", job_name),
                                        );
                                    }
                                }
                                crate::ci::config::JobScript::Multiple(cmds) => {
                                    if cmds.is_empty() {
                                        report.add_error(
                                            format!("job_{}", job_name),
                                            format!("Job '{}' script array cannot be empty", job_name),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                report.add_success("jobs".to_string());
            }
        }

        // Validate matrix configuration if present
        if let Some(matrix) = &pipeline.matrix {
            if matrix.variables.is_empty() {
                report.add_warning(
                    "matrix".to_string(),
                    "Matrix configuration is empty".to_string(),
                );
            } else {
                for (var_name, values) in &matrix.variables {
                    if values.is_empty() {
                        report.add_warning(
                            format!("matrix_{}", var_name),
                            format!("Matrix variable '{}' has no values", var_name),
                        );
                    }
                }
                report.add_success("matrix".to_string());
            }
        }

        // Validate cache configuration if present
        if let Some(cache) = &pipeline.cache {
            if cache.paths.is_empty() {
                report.add_warning(
                    "cache".to_string(),
                    "Cache configuration has no paths".to_string(),
                );
            } else {
                report.add_success("cache".to_string());
            }
        }

        // Validate include configuration if present
        if let Some(includes) = &pipeline.include {
            if includes.is_empty() {
                report.add_warning(
                    "include".to_string(),
                    "Include configuration is empty".to_string(),
                );
            } else {
                for (i, include) in includes.iter().enumerate() {
                    if include.local.is_none() && include.remote.is_none() && include.template.is_none() {
                        report.add_error(
                            format!("include_{}", i),
                            "Include configuration must specify local, remote, or template".to_string(),
                        );
                    }
                }
                report.add_success("include".to_string());
            }
        }

        // Validate variables if present
        if let Some(variables) = &pipeline.variables {
            if variables.is_empty() {
                report.add_warning(
                    "variables".to_string(),
                    "Variables configuration is empty".to_string(),
                );
            } else {
                report.add_success("variables".to_string());
            }
        }

        Ok(report)
    }
}

/// Feature flag validator
pub struct FeatureFlagValidator {
    environment: String,
}

impl FeatureFlagValidator {
    pub fn new(environment: &str) -> Self {
        Self {
            environment: environment.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl ConfigValidator for FeatureFlagValidator {
    async fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport> {
        let mut report = ValidationReport::new();

        // Validate feature flag combinations
        if config.features.enable_experimental_features && self.environment == "production" {
            report.add_error(
                "experimental_in_production".to_string(),
                "Experimental features should not be enabled in production".to_string(),
            );
        }

        if config.features.enable_hot_reload && self.environment == "production" {
            report.add_warning(
                "hot_reload_in_production".to_string(),
                "Hot reload in production may impact performance".to_string(),
            );
        }

        if !config.features.enable_audit_logging && self.environment == "production" {
            report.add_error(
                "audit_logging_disabled".to_string(),
                "Audit logging should be enabled in production".to_string(),
            );
        }

        if !config.features.enable_metrics_collection {
            report.add_warning(
                "metrics_disabled".to_string(),
                "Metrics collection is disabled - monitoring will be limited".to_string(),
            );
        }

        // Check custom flags
        for (flag_name, flag_value) in &config.features.custom_flags {
            if flag_name.contains("debug") && *flag_value && self.environment == "production" {
                report.add_warning(
                    format!("debug_flag_{}", flag_name),
                    format!("Debug flag '{}' enabled in production", flag_name),
                );
            }
        }

        if report.errors.is_empty() && report.warnings.is_empty() {
            report.add_success("feature_flags".to_string());
        }

        Ok(report)
    }

    fn name(&self) -> &'static str {
        "features"
    }

    fn description(&self) -> &'static str {
        "Validates feature flag configuration and environment compatibility"
    }
}

/// External services validator
pub struct ExternalServicesValidator;

#[async_trait::async_trait]
impl ConfigValidator for ExternalServicesValidator {
    async fn validate(&self, config: &AppConfiguration) -> Result<ValidationReport> {
        let mut report = ValidationReport::new();

        // Validate GitHub service configuration
        let github_config = &config.external_services.github;
        if github_config.api_url.is_empty() {
            report.add_error(
                "github_api_url".to_string(),
                "GitHub API URL is required".to_string(),
            );
        } else if !github_config.api_url.starts_with("https://") {
            report.add_warning(
                "github_api_url_insecure".to_string(),
                "GitHub API URL should use HTTPS".to_string(),
            );
        }

        if github_config.timeout_seconds == 0 {
            report.add_warning(
                "github_timeout".to_string(),
                "GitHub API timeout is disabled".to_string(),
            );
        } else if github_config.timeout_seconds > 300 {
            report.add_warning(
                "github_timeout_long".to_string(),
                "GitHub API timeout is very long (> 5 minutes)".to_string(),
            );
        }

        // Validate Docker registry configuration
        let docker_config = &config.external_services.docker_registry;
        if docker_config.url.is_empty() {
            report.add_error(
                "docker_registry_url".to_string(),
                "Docker registry URL is required".to_string(),
            );
        }

        if docker_config.insecure
            && std::env::var("ENVIRONMENT").unwrap_or_default() == "production"
        {
            report.add_error(
                "docker_registry_insecure".to_string(),
                "Insecure Docker registry not allowed in production".to_string(),
            );
        }

        // Validate notification configuration
        let notification_config = &config.external_services.notification;
        if notification_config.enabled && notification_config.channels.is_empty() {
            report.add_warning(
                "notification_no_channels".to_string(),
                "Notifications enabled but no channels configured".to_string(),
            );
        }

        for channel in &notification_config.channels {
            if channel.name.is_empty() {
                report.add_error(
                    "notification_channel_name".to_string(),
                    "Notification channel name is required".to_string(),
                );
            }

            match channel.channel_type.as_str() {
                "email" => {
                    if !channel.config.contains_key("smtp_server") {
                        report.add_error(
                            format!("notification_channel_{}_smtp", channel.name),
                            "Email channel requires smtp_server configuration".to_string(),
                        );
                    }
                }
                "slack" => {
                    if !channel.config.contains_key("webhook_url") {
                        report.add_error(
                            format!("notification_channel_{}_webhook", channel.name),
                            "Slack channel requires webhook_url configuration".to_string(),
                        );
                    }
                }
                "webhook" => {
                    if !channel.config.contains_key("url") {
                        report.add_error(
                            format!("notification_channel_{}_url", channel.name),
                            "Webhook channel requires url configuration".to_string(),
                        );
                    }
                }
                _ => {
                    report.add_warning(
                        format!("notification_channel_{}_type", channel.name),
                        format!(
                            "Unknown notification channel type: {}",
                            channel.channel_type
                        ),
                    );
                }
            }
        }

        if report.errors.is_empty() {
            report.add_success("external_services".to_string());
        }

        Ok(report)
    }

    fn name(&self) -> &'static str {
        "external_services"
    }

    fn description(&self) -> &'static str {
        "Validates external service configurations including GitHub, Docker registry, and notifications"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_config_validation_engine() {
        let engine = ConfigValidationEngine::new("development");
        let config = AppConfiguration::default();

        let report = engine.validate(&config).await.unwrap();

        // Should have some validation results
        assert!(!report.validator_reports.is_empty());
    }

    #[tokio::test]
    async fn test_security_config_validator() {
        let validator = SecurityConfigValidator::new("production");
        let config = AppConfiguration::default();

        let report = validator.validate(&config).await.unwrap();

        // Should have errors due to default config in production
        assert!(!report.is_valid());
    }

    #[tokio::test]
    async fn test_feature_flag_validator() {
        let validator = FeatureFlagValidator::new("production");
        let mut config = AppConfiguration::default();
        config.features.enable_experimental_features = true;

        let report = validator.validate(&config).await.unwrap();

        // Should have error for experimental features in production
        assert!(!report.is_valid());
        assert!(report.errors.contains_key("experimental_in_production"));
    }

    #[test]
    fn test_tiered_pipeline_validator_minimal() {
        let validator = TieredPipelineValidator::new();
        
        // Test valid minimal pipeline
        let mut pipeline = crate::ci::config::CIPipeline::new("test".to_string());
        pipeline.pipeline_type = Some(crate::ci::config::PipelineType::Minimal);
        pipeline.repo = Some("https://github.com/user/repo.git".to_string());
        pipeline.server = Some(crate::ci::config::ServerConfig::Simple("user@host".to_string()));
        
        let report = validator.validate(&pipeline).unwrap();
        assert!(report.is_valid());
        assert!(report.successes.contains(&"repo".to_string()));
        assert!(report.successes.contains(&"server".to_string()));

        // Test minimal pipeline missing server
        pipeline.server = None;
        let report = validator.validate(&pipeline).unwrap();
        assert!(!report.is_valid());
        assert!(report.errors.contains_key("server"));
    }

    #[test]
    fn test_tiered_pipeline_validator_simple() {
        let validator = TieredPipelineValidator::new();
        
        // Test valid simple pipeline
        let mut pipeline = crate::ci::config::CIPipeline::new("test".to_string());
        pipeline.pipeline_type = Some(crate::ci::config::PipelineType::Simple);
        pipeline.steps = Some(vec![
            crate::ci::config::SimpleStep::Command("cargo build".to_string()),
            crate::ci::config::SimpleStep::Detailed {
                run: "cargo test".to_string(),
                name: Some("test".to_string()),
                working_directory: None,
            }
        ]);
        
        let report = validator.validate(&pipeline).unwrap();
        assert!(report.is_valid());
        assert!(report.successes.contains(&"steps".to_string()));

        // Test simple pipeline with empty steps
        pipeline.steps = Some(vec![]);
        let report = validator.validate(&pipeline).unwrap();
        assert!(!report.is_valid());
        assert!(report.errors.contains_key("steps"));
    }

    #[test]
    fn test_tiered_pipeline_validator_standard() {
        let validator = TieredPipelineValidator::new();
        
        // Test valid standard pipeline
        let mut pipeline = crate::ci::config::CIPipeline::new("test".to_string());
        pipeline.pipeline_type = Some(crate::ci::config::PipelineType::Standard);
        pipeline.stages = vec![
            crate::ci::config::Stage {
                name: "build".to_string(),
                condition: None,
                parallel: None,
                steps: vec![
                    crate::ci::config::Step {
                        name: "build step".to_string(),
                        step_type: crate::ci::config::StepType::Shell,
                        config: crate::ci::config::StepConfig {
                            command: Some("cargo build".to_string()),
                            ..Default::default()
                        },
                        condition: None,
                        continue_on_error: None,
                        timeout: None,
                    }
                ],
                environment: None,
                timeout: None,
                retry_count: None,
            }
        ];
        
        let report = validator.validate(&pipeline).unwrap();
        assert!(report.is_valid());
        assert!(report.successes.contains(&"name".to_string()));
        assert!(report.successes.contains(&"stages".to_string()));

        // Test standard pipeline with empty stages
        pipeline.stages = vec![];
        let report = validator.validate(&pipeline).unwrap();
        assert!(!report.is_valid());
        assert!(report.errors.contains_key("stages"));
    }

    #[test]
    fn test_tiered_pipeline_validator_advanced() {
        let validator = TieredPipelineValidator::new();
        
        // Test valid advanced pipeline
        let mut pipeline = crate::ci::config::CIPipeline::new("test".to_string());
        pipeline.pipeline_type = Some(crate::ci::config::PipelineType::Advanced);
        
        let mut jobs = HashMap::new();
        jobs.insert(
            "build".to_string(),
            crate::ci::config::PipelineJob::Simple(
                crate::ci::config::JobScript::Single("cargo build".to_string())
            )
        );
        pipeline.jobs = Some(jobs);
        
        pipeline.variables = Some({
            let mut vars = HashMap::new();
            vars.insert("RUST_VERSION".to_string(), "1.70".to_string());
            vars
        });
        
        pipeline.matrix = Some(crate::ci::config::MatrixConfig {
            variables: {
                let mut matrix_vars = HashMap::new();
                matrix_vars.insert("rust".to_string(), vec!["1.70".to_string(), "1.71".to_string()]);
                matrix_vars
            }
        });
        
        let report = validator.validate(&pipeline).unwrap();
        assert!(report.is_valid());
        assert!(report.successes.contains(&"name".to_string()));
        assert!(report.successes.contains(&"jobs".to_string()));
        assert!(report.successes.contains(&"variables".to_string()));
        assert!(report.successes.contains(&"matrix".to_string()));
    }
}
