// This file declares handler modules and re-exports their contents
// This allows other parts of the code to use `use crate::handlers::function_name`
// instead of `use crate::handlers::auth::function_name`

pub mod auth;
pub mod ci;
pub mod dockerfile;
pub mod pr;
pub mod repository;
pub mod workspace;

// Re-export all public items from the modules
pub use auth::*;