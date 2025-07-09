// This file declares the auth middleware module and re-exports its contents
// This allows other parts of the code to use `use crate::middleware::auth`
// instead of `use crate::middleware::auth::auth`

pub mod auth;

// Re-export all public items from the auth module
pub use auth::*;