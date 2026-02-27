//! Cybulous Core Orchestration Engine
//!
//! Provides the foundational orchestration layer for cross-functional
//! AI-augmented platform instances with biophysical autonomy.

#![forbid(unsafe_code)]
#![warn(
    missing_docs,
    rust_2018_idioms,
    unreachable_pub,
    missing_debug_implementations
)]

pub mod agent;
pub mod artifact;
pub mod orchestration;
pub mod platform;
pub mod state;
pub mod types;

pub use agent::{Agent, AgentCapability, AgentPool};
pub use artifact::{Artifact, ArtifactRegistry};
pub use orchestration::{Orchestrator, ToolCall, ToolResponse};
pub use platform::{PlatformInstance, PlatformType};
pub use state::{StateManager, UserSession};

use thiserror::Error;

/// Core error types for Cybulous platform
#[derive(Error, Debug)]
pub enum CybulousError {
    /// Orchestration-related errors
    #[error("orchestration failed: {0}")]
    OrchestrationFailed(String),

    /// Agent pool errors
    #[error("agent pool error: {0}")]
    AgentPoolError(String),

    /// Platform instance errors
    #[error("platform instance error: {0}")]
    PlatformError(String),

    /// State management errors
    #[error("state management error: {0}")]
    StateError(String),

    /// Consent verification errors
    #[error("consent verification failed: {0}")]
    ConsentError(String),

    /// Artifact storage errors
    #[error("artifact storage error: {0}")]
    ArtifactError(String),

    /// Network errors
    #[error("network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    /// Serialization errors
    #[error("serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

/// Result type alias for Cybulous operations
pub type Result<T> = std::result::Result<T, CybulousError>;

/// Platform version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Minimum supported protocol version
pub const MIN_PROTOCOL_VERSION: u32 = 1;

/// Current protocol version
pub const PROTOCOL_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_constants() {
        assert!(!VERSION.is_empty());
        assert!(PROTOCOL_VERSION >= MIN_PROTOCOL_VERSION);
    }
}
