//! Kubernetes connector module
//!
//! This module provides Kubernetes-based execution capabilities for CI/CD steps.
//! It handles job creation, lifecycle management, log collection, and cleanup.
//! Enhanced with PVC support, lifecycle hooks, and improved resource management.

pub mod connector;
pub mod job_manager;
pub mod lifecycle_hooks;
pub mod validation;
pub mod yaml_generator;

pub use connector::KubernetesConnector;
