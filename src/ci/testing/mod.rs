//! Automated testing infrastructure for pipeline deployment validation
//! 
//! This module provides comprehensive testing capabilities for RustCI pipeline deployments
//! using different runner configurations and environment simulations.

pub mod api_client;
pub mod models;
pub mod error;

pub use api_client::RustCIApiClient;
pub use models::*;
pub use error::*;