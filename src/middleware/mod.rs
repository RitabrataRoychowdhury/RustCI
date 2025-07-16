// This file declares middleware modules and re-exports their contents
// This allows other parts of the code to use `use crate::middleware::function_name`

pub mod auth;
pub mod validation;

// Re-export all public items from the modules
pub use auth::*;