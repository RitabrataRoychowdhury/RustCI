//! Security module for enterprise-grade security controls
//!
//! This module provides comprehensive security features including:
//! - Multi-factor authentication (MFA)
//! - Input validation and sanitization
//! - Enterprise-grade encryption
//! - Audit logging
//! - Rate limiting
//! - Security context management

pub mod mfa;
pub mod input_sanitizer;
pub mod input_validation_middleware;
pub mod encryption;
pub mod audit_logger;
pub mod rate_limiter;
pub mod security_context;
pub mod production_security_manager;

pub use mfa::*;
pub use input_sanitizer::*;
pub use input_validation_middleware::*;
pub use encryption::*;
pub use audit_logger::*;
pub use rate_limiter::*;
pub use security_context::*;
pub use production_security_manager::*;

#[cfg(test)]
mod mfa_test;