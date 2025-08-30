//! API Version 1 routes
//!
//! This module contains all V1 API route implementations for backward compatibility.

pub mod ci;
pub mod auth;

pub use ci::ci_router_v1;
pub use auth::auth_router_v1;