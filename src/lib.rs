//! RustCI - A high-performance CI/CD platform built in Rust
//!
//! This library provides the core functionality for the RustCI platform,
//! including job execution, event handling, and runner management.

#![allow(non_snake_case)]

use std::sync::Arc;

pub mod application;
pub mod ci;
pub mod config;
pub mod core;
pub mod domain;
pub mod error;
pub mod infrastructure;
pub mod presentation;
pub mod service_registry;
pub mod token;
pub mod upload;

// Re-export commonly used types
pub use error::{AppError, Result};

// Re-export health check handler
pub use crate::core::monitoring::health_check_handler;

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub env: Arc<config::AppConfiguration>,
    pub db: Arc<infrastructure::database::DatabaseManager>,
    pub audit_logger: Option<Arc<dyn core::security::AuditLogger>>,
    pub config_manager: Arc<tokio::sync::RwLock<config::HotReloadConfigManager>>,
    pub observability: Arc<core::observability::ObservabilityService>,
    pub ci_engine: Arc<ci::engine::CIEngineOrchestrator>,
}
