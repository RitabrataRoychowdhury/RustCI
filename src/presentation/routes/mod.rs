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

// Versioned route modules
pub mod v1;
pub mod v2;
pub mod api_info;

// Re-export main router creation functions
pub use auth::auth_router;
pub use ci::ci_router;
pub use control_plane::{complete_control_plane_router, control_plane_router};
pub use plugins::create_plugin_routes;
pub use pr::pr_router;
pub use runner::runner_router;
pub use valkyrie::create_valkyrie_routes;

// Re-export versioned routers
pub use v1::{auth_router_v1}; // ci_router_v1 temporarily disabled
pub use v2::{auth_router_v2, ci_router_v2};
pub use api_info::api_info_router;
