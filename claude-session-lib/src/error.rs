//! Error types for claude-session-lib

/// Errors that can occur during session management
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("Failed to spawn Claude process: {0}")]
    SpawnFailed(#[source] std::io::Error),

    #[error("Claude process communication error: {0}")]
    CommunicationError(String),

    #[error("Session not found locally (expired)")]
    SessionNotFound,

    #[error("Invalid permission response: no pending request with id {0}")]
    InvalidPermissionResponse(String),

    #[error("Session already exited with code {0}")]
    AlreadyExited(i32),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Claude client error: {0}")]
    ClaudeError(#[from] claude_codes::Error),
}
