// Route modules for different API endpoints
pub mod api;
pub mod auth;
pub mod ci;
pub mod pr;
pub mod runner;
pub mod utils;

// Re-export main router creation functions
pub use auth::auth_router;
pub use ci::ci_router;
pub use pr::pr_router;
pub use runner::runner_router;
