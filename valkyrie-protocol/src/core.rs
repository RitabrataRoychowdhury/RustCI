//! Core protocol engine and types

pub mod engine;
pub mod connection;
pub mod registry;

pub use engine::ValkyrieEngine;
pub use connection::{Connection, ConnectionManager};
pub use registry::{ServiceRegistry, ServiceInfo};