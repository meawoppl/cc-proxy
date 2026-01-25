# Proxy Library Extraction Plan

This document outlines the plan to extract the session wrapping logic from `claude-portal`'s proxy into a reusable library crate.

## Goal

Create a library that enables a **persistence service** to manage multiple Claude Code sessions - launching them, restarting them on failure, and maintaining their state across service restarts.

The library should provide tight encapsulation: a `Session` owns everything needed to survive restarts (buffer state, Claude process handle, etc.), while the service orchestrates multiple `Session` instances without reaching into their internals.

## Use Cases

1. **Persistence Service** - A daemon that maintains long-running Claude sessions, restarting them on failure, surviving service restarts
2. **Headless Automation** - Run Claude Code sessions without the portal UI, with programmatic permission handling
3. **Custom Backends** - Embed session management in alternative backends (not the portal)

## Current Architecture

```
proxy/
├── main.rs          (581 lines)  CLI entry point, session init
├── session.rs       (1,212 lines) Core forwarding logic
├── output_buffer.rs (358 lines)  Persistent message buffer
├── config.rs        (276 lines)  Config file management
├── auth.rs          (183 lines)  Device flow OAuth
├── ui.rs            (341 lines)  Terminal UI output
├── update.rs        (323 lines)  GitHub auto-updates
├── commands.rs      (74 lines)   --init and --logout subcommands
└── util.rs          (117 lines)  JWT parsing, init URL handling
```

**Total: ~3,500 lines**

### What Goes Into the Library

| Component | Extract? | Notes |
|-----------|----------|-------|
| `session.rs` (core loop) | **Yes** | Refactor to event-based |
| `output_buffer.rs` | **Yes** | For replay on restore, not transport ACK |
| `config.rs` | **No** | Service-specific, not library concern |
| Transport/WebSocket | **No** | Service-specific |
| `auth.rs` | **No** | Service-specific |
| `ui.rs` | **No** | CLI-specific |
| `update.rs` | **No** | Portal-specific |

## Library Design

### Core Types

```rust
// claude-session-lib/src/lib.rs

use claude_codes::AsyncClient;
use uuid::Uuid;
use std::path::PathBuf;

/// Configuration for creating a new session
pub struct SessionConfig {
    pub session_id: Uuid,
    pub working_directory: PathBuf,
    pub session_name: String,
    /// Resume an existing Claude session (vs create new)
    pub resume: bool,
    /// Optional: path to claude binary (defaults to "claude" in PATH)
    pub claude_path: Option<PathBuf>,
}

/// A managed Claude Code session
pub struct Session {
    id: Uuid,
    config: SessionConfig,
    client: AsyncClient,
    buffer: OutputBuffer,
    state: SessionState,
}

/// Internal state for session management
enum SessionState {
    Running,
    WaitingForPermission { request_id: Uuid },
    Exited { code: i32 },
}
```

### Event-Based API

The library exposes an event stream rather than a blocking `run()` method. This allows the service to:
- Multiplex multiple sessions
- Route permission requests to appropriate handlers
- Log/forward outputs as they arrive

```rust
impl Session {
    /// Create a new session (spawns Claude process)
    pub async fn new(config: SessionConfig) -> Result<Self, SessionError>;

    /// Restore a session from a snapshot (for service restart)
    pub async fn restore(snapshot: SessionSnapshot) -> Result<Self, SessionError>;

    /// Serialize current state for persistence
    pub fn snapshot(&self) -> SessionSnapshot;

    /// Get the session ID
    pub fn id(&self) -> Uuid;

    /// Poll for the next event (non-blocking with timeout)
    pub async fn next_event(&mut self) -> Option<SessionEvent>;

    /// Send user input to Claude
    pub async fn send_input(&mut self, content: serde_json::Value) -> Result<(), SessionError>;

    /// Respond to a permission request
    pub async fn respond_permission(
        &mut self,
        request_id: Uuid,
        response: PermissionResponse,
    ) -> Result<(), SessionError>;

    /// Gracefully stop the session
    pub async fn stop(&mut self) -> Result<(), SessionError>;

    /// Check if session is still running
    pub fn is_running(&self) -> bool;
}

/// Events emitted by a session
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// Claude produced output
    Output(ClaudeOutput),

    /// Claude is requesting permission for a tool
    PermissionRequest {
        request_id: Uuid,
        tool_name: String,
        input: serde_json::Value,
    },

    /// Claude process exited
    Exited { code: i32 },

    /// Session encountered an error
    Error(SessionError),

    /// Git branch changed (detected from tool usage)
    BranchChanged { branch: Option<String> },
}

/// Response to a permission request
pub struct PermissionResponse {
    pub allow: bool,
    pub input: Option<serde_json::Value>,
    pub remember: bool,
}
```

### Session Snapshots for Persistence

The service needs to persist session state to survive restarts:

```rust
/// Serializable session state for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub id: Uuid,
    pub config: SessionConfig,
    /// Buffered outputs not yet acknowledged by consumers
    pub pending_outputs: Vec<BufferedOutput>,
    /// Pending permission request (if any)
    pub pending_permission: Option<PendingPermission>,
    /// Timestamp of last activity
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferedOutput {
    pub seq: u64,
    pub content: serde_json::Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingPermission {
    pub request_id: Uuid,
    pub tool_name: String,
    pub input: serde_json::Value,
    pub requested_at: chrono::DateTime<chrono::Utc>,
}
```

### Output Buffer

The buffer stores outputs for replay on session restore, independent of any transport:

```rust
// claude-session-lib/src/buffer.rs

pub struct OutputBuffer {
    session_id: Uuid,
    outputs: VecDeque<BufferedOutput>,
    next_seq: u64,
    max_size: usize,
}

impl OutputBuffer {
    pub fn new(session_id: Uuid) -> Self;

    /// Add output to buffer, returns sequence number
    pub fn push(&mut self, content: serde_json::Value) -> u64;

    /// Mark outputs up to seq as consumed (removes from buffer)
    pub fn ack(&mut self, seq: u64);

    /// Get all pending (unacked) outputs
    pub fn pending(&self) -> impl Iterator<Item = &BufferedOutput>;

    /// Restore buffer from snapshot
    pub fn from_snapshot(outputs: Vec<BufferedOutput>) -> Self;

    /// Export for snapshot
    pub fn to_snapshot(&self) -> Vec<BufferedOutput>;
}
```

### Error Types

```rust
// claude-session-lib/src/error.rs

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("Failed to spawn Claude process: {0}")]
    SpawnFailed(#[from] std::io::Error),

    #[error("Claude process communication error: {0}")]
    CommunicationError(String),

    #[error("Session not found locally (expired)")]
    SessionNotFound,

    #[error("Invalid permission response: no pending request with id {0}")]
    InvalidPermissionResponse(Uuid),

    #[error("Session already exited with code {0}")]
    AlreadyExited(i32),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}
```

## Library Crate Structure

```
claude-session-lib/
├── Cargo.toml
└── src/
    ├── lib.rs           # Public API, re-exports
    ├── session.rs       # Session struct and event loop
    ├── buffer.rs        # OutputBuffer
    ├── snapshot.rs      # SessionSnapshot, serialization
    └── error.rs         # SessionError
```

### Cargo.toml

```toml
[package]
name = "claude-session-lib"
version = "0.1.0"
edition = "2021"

[dependencies]
claude-codes = "2.1"
tokio = { version = "1", features = ["sync", "time", "process"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1"
tracing = "0.1"
```

## Example: Persistence Service Usage

```rust
use claude_session_lib::{Session, SessionConfig, SessionEvent, PermissionResponse};
use std::collections::HashMap;
use tokio::fs;

struct PersistenceService {
    sessions: HashMap<Uuid, Session>,
    snapshot_dir: PathBuf,
}

impl PersistenceService {
    async fn run(&mut self) {
        loop {
            // Poll all sessions for events
            for (id, session) in &mut self.sessions {
                while let Some(event) = session.next_event().await {
                    match event {
                        SessionEvent::Output(output) => {
                            // Forward to connected clients, log, etc.
                            self.broadcast_output(*id, output).await;
                        }
                        SessionEvent::PermissionRequest { request_id, tool_name, input } => {
                            // Route to permission handler (could be auto-approve, UI, etc.)
                            let response = self.handle_permission(&tool_name, &input).await;
                            session.respond_permission(request_id, response).await.ok();
                        }
                        SessionEvent::Exited { code } => {
                            tracing::info!("Session {} exited with code {}", id, code);
                            // Decide: restart? remove? notify?
                            if self.should_restart(*id) {
                                self.restart_session(*id).await;
                            }
                        }
                        SessionEvent::Error(e) => {
                            tracing::error!("Session {} error: {}", id, e);
                        }
                        _ => {}
                    }
                }

                // Periodically snapshot for persistence
                self.save_snapshot(*id, session.snapshot()).await;
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn restore_sessions(&mut self) {
        // Load snapshots from disk and restore sessions
        let entries = fs::read_dir(&self.snapshot_dir).await?;
        for entry in entries {
            let snapshot: SessionSnapshot = serde_json::from_slice(
                &fs::read(entry.path()).await?
            )?;

            match Session::restore(snapshot).await {
                Ok(session) => {
                    self.sessions.insert(session.id(), session);
                }
                Err(e) => {
                    tracing::warn!("Failed to restore session: {}", e);
                }
            }
        }
    }
}
```

## Migration Plan for Existing Proxy

After extraction, the proxy becomes a thin CLI wrapper:

```rust
// proxy/src/main.rs

use claude_session_lib::{Session, SessionConfig, SessionEvent};

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let auth_token = get_or_request_auth(&args).await?;

    // Create session using library
    let config = SessionConfig {
        session_id: args.session_id.unwrap_or_else(Uuid::new_v4),
        working_directory: args.working_dir.clone(),
        session_name: generate_session_name(&args.working_dir),
        resume: args.resume,
        claude_path: args.claude_path,
    };

    let mut session = Session::new(config).await?;

    // Connect to backend (portal-specific, not in library)
    let mut backend = WebSocketBackend::connect(&args.backend_url, &auth_token).await?;

    // Event loop
    loop {
        tokio::select! {
            // Events from Claude (via library)
            Some(event) = session.next_event() => {
                match event {
                    SessionEvent::Output(output) => {
                        ui::print_output(&output);
                        backend.send_output(output).await?;
                    }
                    SessionEvent::PermissionRequest { request_id, tool_name, input } => {
                        backend.send_permission_request(request_id, &tool_name, &input).await?;
                    }
                    SessionEvent::Exited { code } => {
                        ui::print_exit(code);
                        break;
                    }
                    SessionEvent::Error(e) => {
                        ui::print_error(&e);
                        break;
                    }
                    _ => {}
                }
            }

            // Messages from backend
            Some(msg) = backend.receive() => {
                match msg {
                    BackendMessage::Input(content) => {
                        session.send_input(content).await?;
                    }
                    BackendMessage::PermissionResponse { request_id, response } => {
                        session.respond_permission(request_id, response).await?;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
```

## Implementation Phases

### Phase 1: Create Library Crate (2-3 hours)
- Set up `claude-session-lib/` crate structure
- Define public types: `Session`, `SessionConfig`, `SessionEvent`, `SessionError`
- Define `SessionSnapshot` and `OutputBuffer` types
- Stub out implementations

### Phase 2: Extract Core Session Logic (8-10 hours)
- Extract Claude process spawning from `proxy/src/session.rs`
- Implement event-based polling (convert from current channel-based approach)
- Handle permission request/response flow
- Implement `send_input()` and `respond_permission()`

### Phase 3: Implement Snapshot/Restore (4-6 hours)
- Implement `Session::snapshot()` serialization
- Implement `Session::restore()` deserialization
- Handle edge cases: pending permissions, in-flight inputs
- Test snapshot round-trip

### Phase 4: Extract Output Buffer (2-3 hours)
- Move `output_buffer.rs` logic to library
- Simplify: remove transport ACK concerns (service handles that)
- Implement `from_snapshot()` / `to_snapshot()`

### Phase 5: Update Proxy to Use Library (4-6 hours)
- Refactor `proxy/src/main.rs` to use library
- Keep WebSocket/backend logic in proxy
- Keep CLI/UI concerns in proxy
- Verify existing functionality works

### Phase 6: Tests and Documentation (4-6 hours)
- Unit tests for `Session`, `OutputBuffer`, `SessionSnapshot`
- Integration test with mock Claude process
- Document public API
- Example code for persistence service

## Estimated Effort

| Phase | Hours | Description |
|-------|-------|-------------|
| 1 | 2-3 | Create crate structure, define types |
| 2 | 8-10 | Extract session logic, event-based API |
| 3 | 4-6 | Snapshot/restore implementation |
| 4 | 2-3 | Output buffer extraction |
| 5 | 4-6 | Update proxy to use library |
| 6 | 4-6 | Tests and documentation |
| **Total** | **24-34** | **~4-5 days of focused work** |

## Open Questions

1. **Claude process lifecycle on restore** - When restoring a snapshot, should we spawn a new Claude process with `--resume`, or is snapshot only for the service's bookkeeping?

2. **Permission timeout** - If a permission request goes unanswered, should the library timeout and auto-deny? Or leave that to the service?

3. **Buffer size limits** - What's the max buffer size before we start dropping old outputs? Configurable?

4. **Git branch detection** - Keep in library (subprocess calls `git`) or move to service?

## References

- [claude-codes crate](https://github.com/meawoppl/rust-claude-codes) - AsyncClient implementation
- [proxy/session.rs](../proxy/src/session.rs) - Current implementation
- [proxy/output_buffer.rs](../proxy/src/output_buffer.rs) - Current buffer implementation
