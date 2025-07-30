//! Docker connector module
//!
//! This module provides Docker-based execution capabilities for CI/CD steps.
//! It handles Docker image building, container execution, and cleanup.

pub mod connector;

pub use connector::DockerConnector;
