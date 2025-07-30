// This file declares the auth routes module and re-exports its contents
// This allows other parts of the code to use `use crate::routes::auth_router`
// instead of `use crate::routes::auth::auth_router`

pub mod auth;
pub mod ci;
pub mod pr;
pub mod api;

// Re-export all public items from the modules
pub use auth::*;
pub use ci::*;
pub use pr::*;