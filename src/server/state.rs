use std::sync::Arc;
use std::time::Instant;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::commands::Platform;
use crate::config::Config;

/// Caps the number of *finished* sessions retained in memory; active sessions are
/// never evicted. Bounds memory on long-running servers without adding persistence.
const MAX_FINISHED_SESSIONS: usize = 50;

/// Shared state handed to every Axum route and WebSocket connection.
pub(crate) struct AppState {
    pub(crate) config: Arc<RwLock<Config>>,
    pub(crate) sessions: Arc<DashMap<Uuid, DownloadSession>>,
    pub(crate) event_tx: broadcast::Sender<ServerEvent>,
}

impl AppState {
    pub(crate) fn new(config: Config) -> Self {
        let (event_tx, _rx) = broadcast::channel(256);

        Self {
            config: Arc::new(RwLock::new(config)),
            sessions: Arc::new(DashMap::new()),
            event_tx,
        }
    }
}

/// Tracks one in-flight or completed server-driven download.
pub(crate) struct DownloadSession {
    pub(crate) id: Uuid,
    pub(crate) platform: Platform,
    pub(crate) url: String,
    pub(crate) status: SessionStatus,
    pub(crate) progress: f32,
    /// Set when `status` lands on `Error`/`Cancelled`, so `GET /api/downloads`
    /// can explain *why* a session failed even to clients that missed the
    /// transient WS `ServerEvent::Error` broadcast.
    pub(crate) error_message: Option<String>,
    pub(crate) abort: CancellationToken,
    pub(crate) created_at: Instant,
}

/// Mirrors the lifecycle of a server-driven download.
#[derive(Debug, Clone)]
pub(crate) enum SessionStatus {
    Pending,
    FetchingMetadata,
    Downloading,
    Merging,
    Complete,
    Error,
    Cancelled,
}

impl SessionStatus {
    /// Renders the lowercase string form used in `GET /api/downloads` and WS `status` events.
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            SessionStatus::Pending => "pending",
            SessionStatus::FetchingMetadata => "fetching_metadata",
            SessionStatus::Downloading => "downloading",
            SessionStatus::Merging => "merging",
            SessionStatus::Complete => "complete",
            SessionStatus::Error => "error",
            SessionStatus::Cancelled => "cancelled",
        }
    }

    pub(crate) fn is_finished(&self) -> bool {
        matches!(
            self,
            SessionStatus::Complete | SessionStatus::Error | SessionStatus::Cancelled
        )
    }
}

/// Internal broadcast payload mirroring the WebSocket "Server → Client" message shapes.
/// `Pong` is intentionally absent — `ws.rs` answers `ping` directly without using this channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ServerEvent {
    Progress {
        session_id: Uuid,
        percent: f32,
        speed: String,
        eta: String,
        size: String,
    },
    Status {
        session_id: Uuid,
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    Metadata {
        session_id: Uuid,
        title: String,
        uploader: Option<String>,
        thumbnail: Option<String>,
        duration: Option<i64>,
    },
    Complete {
        session_id: Uuid,
        output_path: String,
    },
    Error {
        session_id: Uuid,
        message: String,
    },
}

impl ServerEvent {
    pub(crate) fn session_id(&self) -> Uuid {
        match self {
            ServerEvent::Progress { session_id, .. }
            | ServerEvent::Status { session_id, .. }
            | ServerEvent::Metadata { session_id, .. }
            | ServerEvent::Complete { session_id, .. }
            | ServerEvent::Error { session_id, .. } => *session_id,
        }
    }
}

/// Evicts the oldest finished sessions (by `created_at`) once their count exceeds
/// [`MAX_FINISHED_SESSIONS`]. Active sessions are never touched.
pub(crate) fn evict_finished_sessions(sessions: &DashMap<Uuid, DownloadSession>) {
    let mut finished: Vec<(Uuid, Instant)> = sessions
        .iter()
        .filter(|entry| entry.value().status.is_finished())
        .map(|entry| (*entry.key(), entry.value().created_at))
        .collect();

    if finished.len() <= MAX_FINISHED_SESSIONS {
        return;
    }

    finished.sort_by_key(|(_, created_at)| *created_at);
    let overflow = finished.len() - MAX_FINISHED_SESSIONS;

    for (id, _) in finished.into_iter().take(overflow) {
        sessions.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn session_at(status: SessionStatus, offset_secs: u64, base: Instant) -> DownloadSession {
        DownloadSession {
            id: Uuid::new_v4(),
            platform: Platform::YouTube,
            url: "https://example.com/watch?v=test".to_string(),
            status,
            progress: 0.0,
            error_message: None,
            abort: CancellationToken::new(),
            created_at: base + Duration::from_secs(offset_secs),
        }
    }

    #[test]
    fn eviction_keeps_only_the_newest_finished_sessions() {
        let sessions: DashMap<Uuid, DownloadSession> = DashMap::new();
        let base = Instant::now();

        for offset in 0..60u64 {
            let session = session_at(SessionStatus::Complete, offset, base);
            sessions.insert(session.id, session);
        }

        evict_finished_sessions(&sessions);

        assert_eq!(sessions.len(), MAX_FINISHED_SESSIONS);
        let oldest_remaining = sessions
            .iter()
            .map(|entry| entry.value().created_at)
            .min()
            .expect("sessions should not be empty");
        assert_eq!(oldest_remaining, base + Duration::from_secs(10));
    }

    #[test]
    fn eviction_never_removes_active_sessions() {
        let sessions: DashMap<Uuid, DownloadSession> = DashMap::new();
        let base = Instant::now();

        for offset in 0..60u64 {
            sessions.insert(
                Uuid::new_v4(),
                session_at(SessionStatus::Downloading, offset, base),
            );
        }

        evict_finished_sessions(&sessions);

        assert_eq!(sessions.len(), 60);
    }

    #[test]
    fn server_event_serializes_with_tagged_shape() {
        let session_id = Uuid::new_v4();
        let event = ServerEvent::Status {
            session_id,
            status: "downloading".to_string(),
            message: None,
        };

        let json = serde_json::to_value(&event).expect("event should serialize");
        assert_eq!(json["type"], "status");
        assert_eq!(json["session_id"], session_id.to_string());
        assert_eq!(json["status"], "downloading");
        assert!(json.get("message").is_none());
    }
}
