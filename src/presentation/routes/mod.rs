// Route modules for different API endpoints
pub mod api;
pub mod auth;
pub mod ci;
pub mod control_plane;
pub mod plugins;
pub mod pr;
pub mod runner;
pub mod utils;
pub mod valkyrie;

// Re-export main router creation functions
pub use auth::auth_router;
pub use ci::ci_router;
pub use control_plane::{complete_control_plane_router, control_plane_router};
pub use plugins::create_plugin_routes;
pub use pr::pr_router;
pub use runner::runner_router;
pub use valkyrie::create_valkyrie_routes;
