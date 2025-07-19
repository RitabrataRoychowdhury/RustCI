pub mod command;
pub mod dockerfile_generation;
pub mod dockerfile_validation;
pub mod encryption;
pub mod github;
pub mod notification;
pub mod pr_builder;
pub mod project_detection;
pub mod workspace;

// Re-export commonly used services
pub use encryption::EncryptionService;
pub use github::GitHubService;
// pub use workspace::WorkspaceService; // Unused import