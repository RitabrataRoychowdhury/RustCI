// This file declares the auth routes module and re-exports its contents
// This allows other parts of the code to use `use crate::routes::auth_router`
// instead of `use crate::routes::auth::auth_router`

pub mod auth;

// Re-export all public items from the auth module
pub use auth::*;