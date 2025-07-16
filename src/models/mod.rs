// This file declares all model modules and re-exports their contents
// This allows other parts of the code to use `use crate::models::User`
// instead of `use crate::models::user::User`

pub mod dockerfile;
pub mod github;
pub mod user;
pub mod workspace;

// Re-export all public items from the modules
// This makes them available at the models module level
pub use dockerfile::*;
pub use github::*;
pub use user::*;
pub use workspace::*;