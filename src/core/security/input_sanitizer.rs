//! Input Validation and Sanitization System
//!
//! This module provides comprehensive input validation and sanitization
//! to prevent injection attacks and ensure data integrity.

use crate::error::AppError;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, warn};
use uuid::Uuid;

/// Validation rule types for different input contexts
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValidationRule {
    /// Prevent SQL injection attacks
    SqlInjection,
    /// Prevent XSS attacks
    XssProtection,
    /// Prevent command injection
    CommandInjection,
    /// Validate email format
    Email,
    /// Validate URL format
    Url,
    /// Validate alphanumeric input
    Alphanumeric,
    /// Validate numeric input
    Numeric,
    /// Custom regex validation
    CustomRegex { pattern: String, description: String },
    /// Length validation
    Length { min: usize, max: usize },
    /// Required field validation
    Required,
    /// Whitelist validation
    Whitelist { allowed_values: Vec<String> },
}

/// Sanitization action to take on input
#[derive(Debug, Clone, PartialEq)]
pub enum SanitizationAction {
    /// Remove dangerous characters
    Remove,
    /// Escape dangerous characters
    Escape,
    /// Encode dangerous characters
    Encode,
    /// Reject the input entirely
    Reject,
}

/// Validation result with detailed information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub sanitized_input: Option<String>,
    pub violations: Vec<ValidationViolation>,
    pub risk_level: RiskLevel,
}

/// Individual validation violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationViolation {
    pub rule: String,
    pub description: String,
    pub severity: ViolationSeverity,
    pub position: Option<usize>,
    pub suggested_fix: Option<String>,
}

/// Risk level assessment
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Violation severity
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ViolationSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Configuration for input sanitizer
#[derive(Debug, Clone)]
pub struct InputSanitizerConfig {
    pub strict_mode: bool,
    pub auto_sanitize: bool,
    pub log_violations: bool,
    pub max_input_length: usize,
    pub custom_patterns: HashMap<String, String>,
}

impl Default for InputSanitizerConfig {
    fn default() -> Self {
        Self {
            strict_mode: true,
            auto_sanitize: true,
            log_violations: true,
            max_input_length: 10_000,
            custom_patterns: HashMap::new(),
        }
    }
}

/// Production-ready input sanitizer with comprehensive validation
pub struct InputSanitizer {
    config: InputSanitizerConfig,
    sql_injection_patterns: Vec<Regex>,
    xss_patterns: Vec<Regex>,
    command_injection_patterns: Vec<Regex>,
    email_regex: Regex,
    url_regex: Regex,
    alphanumeric_regex: Regex,
    numeric_regex: Regex,
}

impl InputSanitizer {
    /// Create a new input sanitizer with default configuration
    pub fn new() -> Result<Self, AppError> {
        Self::with_config(InputSanitizerConfig::default())
    }

    /// Create a new input sanitizer with custom configuration
    pub fn with_config(config: InputSanitizerConfig) -> Result<Self, AppError> {
        let sql_injection_patterns = Self::compile_sql_patterns()?;
        let xss_patterns = Self::compile_xss_patterns()?;
        let command_injection_patterns = Self::compile_command_patterns()?;
        
        let email_regex = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$")
            .map_err(|e| AppError::InternalServerError(format!("Failed to compile email regex: {}", e)))?;
        
        let url_regex = Regex::new(r"^https?://[^\s/$.?#].[^\s]*$")
            .map_err(|e| AppError::InternalServerError(format!("Failed to compile URL regex: {}", e)))?;
        
        let alphanumeric_regex = Regex::new(r"^[a-zA-Z0-9]+$")
            .map_err(|e| AppError::InternalServerError(format!("Failed to compile alphanumeric regex: {}", e)))?;
        
        let numeric_regex = Regex::new(r"^[0-9]+$")
            .map_err(|e| AppError::InternalServerError(format!("Failed to compile numeric regex: {}", e)))?;

        Ok(Self {
            config,
            sql_injection_patterns,
            xss_patterns,
            command_injection_patterns,
            email_regex,
            url_regex,
            alphanumeric_regex,
            numeric_regex,
        })
    }

    /// Validate and sanitize input according to the provided rules
    pub fn validate_and_sanitize(
        &self,
        input: &str,
        rules: &[ValidationRule],
    ) -> Result<ValidationResult, AppError> {
        let correlation_id = Uuid::new_v4();
        debug!("Starting input validation for correlation_id: {}", correlation_id);

        // Check input length first
        if input.len() > self.config.max_input_length {
            return Ok(ValidationResult {
                is_valid: false,
                sanitized_input: None,
                violations: vec![ValidationViolation {
                    rule: "MaxLength".to_string(),
                    description: format!("Input exceeds maximum length of {} characters", self.config.max_input_length),
                    severity: ViolationSeverity::Error,
                    position: Some(self.config.max_input_length),
                    suggested_fix: Some("Reduce input length".to_string()),
                }],
                risk_level: RiskLevel::High,
            });
        }

        let mut violations = Vec::new();
        let mut sanitized_input = input.to_string();
        let mut max_risk_level = RiskLevel::Low;

        // Apply each validation rule
        for rule in rules {
            match self.apply_validation_rule(&sanitized_input, rule) {
                Ok(rule_result) => {
                    if !rule_result.violations.is_empty() {
                        violations.extend(rule_result.violations);
                        
                        // Update risk level
                        if rule_result.risk_level as u8 > max_risk_level as u8 {
                            max_risk_level = rule_result.risk_level;
                        }

                        // Apply sanitization if configured
                        if self.config.auto_sanitize {
                            if let Some(sanitized) = rule_result.sanitized_input {
                                sanitized_input = sanitized;
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Validation rule failed: {:?}", e);
                    violations.push(ValidationViolation {
                        rule: format!("{:?}", rule),
                        description: format!("Validation rule execution failed: {}", e),
                        severity: ViolationSeverity::Error,
                        position: None,
                        suggested_fix: None,
                    });
                    max_risk_level = RiskLevel::High;
                }
            }
        }

        let is_valid = violations.is_empty() || 
            (!self.config.strict_mode && violations.iter().all(|v| v.severity != ViolationSeverity::Critical));

        // Log violations if configured
        if self.config.log_violations && !violations.is_empty() {
            warn!(
                "Input validation violations detected (correlation_id: {}): {} violations, risk_level: {:?}",
                correlation_id,
                violations.len(),
                max_risk_level
            );
        }

        Ok(ValidationResult {
            is_valid,
            sanitized_input: if is_valid && self.config.auto_sanitize {
                Some(sanitized_input)
            } else {
                None
            },
            violations,
            risk_level: max_risk_level,
        })
    }

    /// Apply a single validation rule
    fn apply_validation_rule(
        &self,
        input: &str,
        rule: &ValidationRule,
    ) -> Result<ValidationResult, AppError> {
        match rule {
            ValidationRule::SqlInjection => self.check_sql_injection(input),
            ValidationRule::XssProtection => self.check_xss_protection(input),
            ValidationRule::CommandInjection => self.check_command_injection(input),
            ValidationRule::Email => self.validate_email(input),
            ValidationRule::Url => self.validate_url(input),
            ValidationRule::Alphanumeric => self.validate_alphanumeric(input),
            ValidationRule::Numeric => self.validate_numeric(input),
            ValidationRule::CustomRegex { pattern, description } => {
                self.validate_custom_regex(input, pattern, description)
            }
            ValidationRule::Length { min, max } => self.validate_length(input, *min, *max),
            ValidationRule::Required => self.validate_required(input),
            ValidationRule::Whitelist { allowed_values } => {
                self.validate_whitelist(input, allowed_values)
            }
        }
    }

    /// Check for SQL injection patterns
    fn check_sql_injection(&self, input: &str) -> Result<ValidationResult, AppError> {
        let mut violations = Vec::new();
        let mut sanitized = input.to_string();

        for pattern in &self.sql_injection_patterns {
            if let Some(mat) = pattern.find(input) {
                violations.push(ValidationViolation {
                    rule: "SqlInjection".to_string(),
                    description: format!("Potential SQL injection detected at position {}", mat.start()),
                    severity: ViolationSeverity::Critical,
                    position: Some(mat.start()),
                    suggested_fix: Some("Remove or escape SQL keywords and special characters".to_string()),
                });

                // Sanitize by removing dangerous patterns
                sanitized = pattern.replace_all(&sanitized, "").to_string();
            }
        }

        let is_valid = violations.is_empty();
        let risk_level = if is_valid { RiskLevel::Low } else { RiskLevel::Critical };
        
        Ok(ValidationResult {
            is_valid,
            sanitized_input: Some(sanitized),
            violations,
            risk_level,
        })
    }

    /// Check for XSS patterns
    fn check_xss_protection(&self, input: &str) -> Result<ValidationResult, AppError> {
        let mut violations = Vec::new();
        let mut sanitized = input.to_string();

        for pattern in &self.xss_patterns {
            if let Some(mat) = pattern.find(input) {
                violations.push(ValidationViolation {
                    rule: "XssProtection".to_string(),
                    description: format!("Potential XSS attack detected at position {}", mat.start()),
                    severity: ViolationSeverity::Critical,
                    position: Some(mat.start()),
                    suggested_fix: Some("HTML encode or remove script tags and event handlers".to_string()),
                });

                // Sanitize by HTML encoding
                sanitized = self.html_encode(&sanitized);
            }
        }

        let is_valid = violations.is_empty();
        let risk_level = if is_valid { RiskLevel::Low } else { RiskLevel::Critical };
        
        Ok(ValidationResult {
            is_valid,
            sanitized_input: Some(sanitized),
            violations,
            risk_level,
        })
    }

    /// Check for command injection patterns
    fn check_command_injection(&self, input: &str) -> Result<ValidationResult, AppError> {
        let mut violations = Vec::new();
        let mut sanitized = input.to_string();

        for pattern in &self.command_injection_patterns {
            if let Some(mat) = pattern.find(input) {
                violations.push(ValidationViolation {
                    rule: "CommandInjection".to_string(),
                    description: format!("Potential command injection detected at position {}", mat.start()),
                    severity: ViolationSeverity::Critical,
                    position: Some(mat.start()),
                    suggested_fix: Some("Remove or escape shell metacharacters".to_string()),
                });

                // Sanitize by removing dangerous characters
                sanitized = pattern.replace_all(&sanitized, "").to_string();
            }
        }

        let is_valid = violations.is_empty();
        let risk_level = if is_valid { RiskLevel::Low } else { RiskLevel::Critical };
        
        Ok(ValidationResult {
            is_valid,
            sanitized_input: Some(sanitized),
            violations,
            risk_level,
        })
    }

    /// Validate email format
    fn validate_email(&self, input: &str) -> Result<ValidationResult, AppError> {
        let is_valid = self.email_regex.is_match(input);
        
        Ok(ValidationResult {
            is_valid,
            sanitized_input: Some(input.to_string()),
            violations: if is_valid {
                Vec::new()
            } else {
                vec![ValidationViolation {
                    rule: "Email".to_string(),
                    description: "Invalid email format".to_string(),
                    severity: ViolationSeverity::Error,
                    position: None,
                    suggested_fix: Some("Use format: user@domain.com".to_string()),
                }]
            },
            risk_level: if is_valid { RiskLevel::Low } else { RiskLevel::Medium },
        })
    }

    /// Validate URL format
    fn validate_url(&self, input: &str) -> Result<ValidationResult, AppError> {
        let is_valid = self.url_regex.is_match(input);
        
        Ok(ValidationResult {
            is_valid,
            sanitized_input: Some(input.to_string()),
            violations: if is_valid {
                Vec::new()
            } else {
                vec![ValidationViolation {
                    rule: "Url".to_string(),
                    description: "Invalid URL format".to_string(),
                    severity: ViolationSeverity::Error,
                    position: None,
                    suggested_fix: Some("Use format: https://domain.com/path".to_string()),
                }]
            },
            risk_level: if is_valid { RiskLevel::Low } else { RiskLevel::Medium },
        })
    }

    /// Validate alphanumeric input
    fn validate_alphanumeric(&self, input: &str) -> Result<ValidationResult, AppError> {
        let is_valid = self.alphanumeric_regex.is_match(input);
        
        Ok(ValidationResult {
            is_valid,
            sanitized_input: Some(input.chars().filter(|c| c.is_alphanumeric()).collect()),
            violations: if is_valid {
                Vec::new()
            } else {
                vec![ValidationViolation {
                    rule: "Alphanumeric".to_string(),
                    description: "Input contains non-alphanumeric characters".to_string(),
                    severity: ViolationSeverity::Warning,
                    position: None,
                    suggested_fix: Some("Use only letters and numbers".to_string()),
                }]
            },
            risk_level: if is_valid { RiskLevel::Low } else { RiskLevel::Low },
        })
    }

    /// Validate numeric input
    fn validate_numeric(&self, input: &str) -> Result<ValidationResult, AppError> {
        let is_valid = self.numeric_regex.is_match(input);
        
        Ok(ValidationResult {
            is_valid,
            sanitized_input: Some(input.chars().filter(|c| c.is_numeric()).collect()),
            violations: if is_valid {
                Vec::new()
            } else {
                vec![ValidationViolation {
                    rule: "Numeric".to_string(),
                    description: "Input contains non-numeric characters".to_string(),
                    severity: ViolationSeverity::Warning,
                    position: None,
                    suggested_fix: Some("Use only numbers".to_string()),
                }]
            },
            risk_level: if is_valid { RiskLevel::Low } else { RiskLevel::Low },
        })
    }

    /// Validate custom regex pattern
    fn validate_custom_regex(
        &self,
        input: &str,
        pattern: &str,
        description: &str,
    ) -> Result<ValidationResult, AppError> {
        let regex = Regex::new(pattern)
            .map_err(|e| AppError::ValidationError(format!("Invalid regex pattern: {}", e)))?;
        
        let is_valid = regex.is_match(input);
        
        Ok(ValidationResult {
            is_valid,
            sanitized_input: Some(input.to_string()),
            violations: if is_valid {
                Vec::new()
            } else {
                vec![ValidationViolation {
                    rule: "CustomRegex".to_string(),
                    description: format!("Input does not match pattern: {}", description),
                    severity: ViolationSeverity::Error,
                    position: None,
                    suggested_fix: Some(format!("Input must match pattern: {}", pattern)),
                }]
            },
            risk_level: if is_valid { RiskLevel::Low } else { RiskLevel::Medium },
        })
    }

    /// Validate input length
    fn validate_length(&self, input: &str, min: usize, max: usize) -> Result<ValidationResult, AppError> {
        let len = input.len();
        let is_valid = len >= min && len <= max;
        
        Ok(ValidationResult {
            is_valid,
            sanitized_input: Some(input.to_string()),
            violations: if is_valid {
                Vec::new()
            } else {
                vec![ValidationViolation {
                    rule: "Length".to_string(),
                    description: format!("Input length {} is outside allowed range {}-{}", len, min, max),
                    severity: ViolationSeverity::Error,
                    position: None,
                    suggested_fix: Some(format!("Input must be between {} and {} characters", min, max)),
                }]
            },
            risk_level: if is_valid { RiskLevel::Low } else { RiskLevel::Medium },
        })
    }

    /// Validate required field
    fn validate_required(&self, input: &str) -> Result<ValidationResult, AppError> {
        let is_valid = !input.trim().is_empty();
        
        Ok(ValidationResult {
            is_valid,
            sanitized_input: Some(input.trim().to_string()),
            violations: if is_valid {
                Vec::new()
            } else {
                vec![ValidationViolation {
                    rule: "Required".to_string(),
                    description: "Field is required and cannot be empty".to_string(),
                    severity: ViolationSeverity::Error,
                    position: None,
                    suggested_fix: Some("Provide a non-empty value".to_string()),
                }]
            },
            risk_level: if is_valid { RiskLevel::Low } else { RiskLevel::Medium },
        })
    }

    /// Validate whitelist
    fn validate_whitelist(&self, input: &str, allowed_values: &[String]) -> Result<ValidationResult, AppError> {
        let is_valid = allowed_values.contains(&input.to_string());
        
        Ok(ValidationResult {
            is_valid,
            sanitized_input: Some(input.to_string()),
            violations: if is_valid {
                Vec::new()
            } else {
                vec![ValidationViolation {
                    rule: "Whitelist".to_string(),
                    description: format!("Value '{}' is not in the allowed list", input),
                    severity: ViolationSeverity::Error,
                    position: None,
                    suggested_fix: Some(format!("Use one of: {}", allowed_values.join(", "))),
                }]
            },
            risk_level: if is_valid { RiskLevel::Low } else { RiskLevel::Medium },
        })
    }

    /// HTML encode dangerous characters
    fn html_encode(&self, input: &str) -> String {
        input
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#x27;")
            .replace('/', "&#x2F;")
    }

    /// Compile SQL injection detection patterns
    fn compile_sql_patterns() -> Result<Vec<Regex>, AppError> {
        let patterns = vec![
            r"(?i)\b(union|select|insert|update|delete|drop|create|alter|exec|execute)\b",
            r"(?i)\b(or|and)\s+\d+\s*=\s*\d+",
            r"(?i)\b(or|and)\s+['\x22][^'\x22]*['\x22]",
            r"(?i)--|\#|\/\*|\*\/",
            r"(?i)\bxp_cmdshell\b",
            r"(?i)\bsp_executesql\b",
            r"[';](\s*)(union|select|insert|update|delete)",
        ];

        patterns
            .into_iter()
            .map(|pattern| {
                Regex::new(pattern).map_err(|e| {
                    AppError::InternalServerError(format!("Failed to compile SQL pattern: {}", e))
                })
            })
            .collect()
    }

    /// Compile XSS detection patterns
    fn compile_xss_patterns() -> Result<Vec<Regex>, AppError> {
        let patterns = vec![
            r"(?i)<script[^>]*>.*?</script>",
            r"(?i)<iframe[^>]*>.*?</iframe>",
            r"(?i)<object[^>]*>.*?</object>",
            r"(?i)<embed[^>]*>",
            r"(?i)<link[^>]*>",
            r"(?i)on\w+\s*=",
            r"(?i)javascript:",
            r"(?i)vbscript:",
            r"(?i)data:text/html",
            r"(?i)<img[^>]*src\s*=\s*[\x22\x27]javascript:[^\x22\x27]*[\x22\x27]",
        ];

        patterns
            .into_iter()
            .map(|pattern| {
                Regex::new(pattern).map_err(|e| {
                    AppError::InternalServerError(format!("Failed to compile XSS pattern: {}", e))
                })
            })
            .collect()
    }

    /// Compile command injection detection patterns
    fn compile_command_patterns() -> Result<Vec<Regex>, AppError> {
        let patterns = vec![
            r"[;&|`$(){}[\]\\]",
            r"(?i)\b(rm|del|format|fdisk|mkfs)\b",
            r"(?i)\b(cat|type|more|less)\b\s+/",
            r"(?i)\b(wget|curl|nc|netcat)\b",
            r"(?i)\b(chmod|chown|sudo)\b",
            r"(?i)>\s*/dev/",
            r"(?i)/bin/|/usr/bin/|cmd\.exe|powershell",
        ];

        patterns
            .into_iter()
            .map(|pattern| {
                Regex::new(pattern).map_err(|e| {
                    AppError::InternalServerError(format!("Failed to compile command pattern: {}", e))
                })
            })
            .collect()
    }
}

impl Default for InputSanitizer {
    fn default() -> Self {
        Self::new().expect("Failed to create default InputSanitizer")
    }
}