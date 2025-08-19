//! Architectural patterns and design patterns
//!
//! This module contains implementations of architectural patterns like
//! CQRS, Event Sourcing, Saga pattern, and other design patterns.

pub mod commands;
pub mod correlation;
pub mod event_sourcing;
pub mod events;
pub mod projections;
pub mod queries;
pub mod sagas;

// Re-export commonly used types
pub use commands::CommandHandler;
pub use events::{EventBus, EventHandler};
pub use sagas::SagaOrchestrator;
