// This file declares the auth module and re-exports its contents
// This allows other parts of the code to use `use crate::handlers::function_name`
// instead of `use crate::handlers::auth::function_name`

pub mod auth;
pub mod ci;

// Re-export all public items from the auth module
pub use auth::*;