//! API Version 2 routes
//!
//! This module contains all V2 API route implementations (current version).

pub mod ci;
pub mod auth;

pub use ci::ci_router_v2;
pub use auth::auth_router_v2;