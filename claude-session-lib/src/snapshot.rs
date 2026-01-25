//! Session snapshot types for persistence

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use crate::buffer::BufferedOutput;

/// Configuration for creating a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Unique session identifier
    pub session_id: Uuid,
    /// Working directory for the Claude session
    pub working_directory: PathBuf,
    /// Human-readable session name
    pub session_name: String,
    /// Whether to resume an existing Claude session (vs create new)
    pub resume: bool,
    /// Optional path to claude binary (defaults to "claude" in PATH)
    pub claude_path: Option<PathBuf>,
}

/// A pending permission request that hasn't been responded to
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingPermission {
    /// Unique request identifier (string format from Claude)
    pub request_id: String,
    /// Name of the tool requesting permission
    pub tool_name: String,
    /// Tool input parameters
    pub input: serde_json::Value,
    /// When the request was received
    pub requested_at: DateTime<Utc>,
}

/// Serializable session state for persistence
///
/// This captures everything needed to restore a session after
/// a service restart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    /// Session identifier
    pub id: Uuid,
    /// Session configuration
    pub config: SessionConfig,
    /// Buffered outputs not yet acknowledged by consumers
    pub pending_outputs: Vec<BufferedOutput>,
    /// Pending permission request (if any)
    pub pending_permission: Option<PendingPermission>,
    /// Timestamp of last activity
    pub last_activity: DateTime<Utc>,
    /// Whether the Claude process was running when snapshot was taken
    pub was_running: bool,
}

impl SessionSnapshot {
    /// Create a new snapshot
    pub fn new(
        id: Uuid,
        config: SessionConfig,
        pending_outputs: Vec<BufferedOutput>,
        pending_permission: Option<PendingPermission>,
        was_running: bool,
    ) -> Self {
        Self {
            id,
            config,
            pending_outputs,
            pending_permission,
            last_activity: Utc::now(),
            was_running,
        }
    }

    /// Serialize snapshot to JSON bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec_pretty(self)
    }

    /// Deserialize snapshot from JSON bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}
