// This file declares the user module and re-exports its contents
// This allows other parts of the code to use `use crate::models::User`
// instead of `use crate::models::user::User`

pub mod user;

// Re-export all public items from the user module
// This makes them available at the models module level
pub use user::*;