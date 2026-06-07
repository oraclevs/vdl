use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use uuid::Uuid;
use yt_dlp::events::DownloadEvent;
use yt_dlp::model::Video;
use yt_dlp::Downloader;

use crate::cli::CommonArgs;
use crate::commands::{
    cleanup_download_artifacts, execute_audio_download, execute_full_download,
    execute_video_only_download, fetch_video, format_size, normalize_common_args,
    prepare_download_environment, sanitise_filename, CommonRequest, Platform,
};
use crate::config::Config;

use super::state::{evict_finished_sessions, AppState, ServerEvent, SessionStatus};

/// Body of `POST /api/download`. `start`/`end` are accepted for forward JSON-shape
/// compatibility with the eventual UI but rejected — see the spec's "Note on start/end".
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DownloadRequest {
    pub(crate) platform: Platform,
    pub(crate) url: Option<String>,
    pub(crate) quality: Option<String>,
    pub(crate) format: Option<String>,
    #[serde(default)]
    pub(crate) audio_only: bool,
    #[serde(default)]
    pub(crate) video_only: bool,
    #[serde(default)]
    pub(crate) start: Option<f64>,
    #[serde(default)]
    pub(crate) end: Option<f64>,
    #[serde(default)]
    pub(crate) output_dir: Option<PathBuf>,
}

/// Distinguishes "the engine reported an error" from "the user cancelled" so `run`
/// can broadcast the right terminal `ServerEvent`/`SessionStatus` pair.
enum DriveError {
    Cancelled,
    Failed(String),
}

/// Drives one server-side download end-to-end and always leaves the session in a
/// terminal state with a corresponding broadcast event — the single entry point
/// spawned by `routes::start_download`.
pub(crate) async fn run(session_id: Uuid, request: DownloadRequest, state: Arc<AppState>) {
    match drive(session_id, &request, &state).await {
        Ok(output_path) => finish(
            &state,
            session_id,
            SessionStatus::Complete,
            ServerEvent::Complete {
                session_id,
                output_path: output_path.display().to_string(),
            },
        ),
        Err(DriveError::Cancelled) => finish(
            &state,
            session_id,
            SessionStatus::Cancelled,
            ServerEvent::Error {
                session_id,
                message: "cancelled by user".to_string(),
            },
        ),
        Err(DriveError::Failed(message)) => finish(
            &state,
            session_id,
            SessionStatus::Error,
            ServerEvent::Error {
                session_id,
                message,
            },
        ),
    }
}

async fn drive(
    session_id: Uuid,
    request: &DownloadRequest,
    state: &Arc<AppState>,
) -> Result<PathBuf, DriveError> {
    let cancel = state
        .sessions
        .get(&session_id)
        .map(|session| session.abort.clone())
        .ok_or_else(|| {
            DriveError::Failed("Session disappeared before the download started.".to_string())
        })?;

    if request.start.is_some() || request.end.is_some() {
        return Err(DriveError::Failed(
            "Clipping is not yet supported.".to_string(),
        ));
    }

    let url = match &request.url {
        Some(url) if !url.trim().is_empty() => url.clone(),
        _ => return Err(DriveError::Failed("Field \"url\" is required.".to_string())),
    };

    let cfg = state.config.read().await.clone();
    let platform = request.platform;

    let args = CommonArgs {
        url: Some(url.clone()),
        quality: request.quality.clone(),
        audio_only: request.audio_only,
        video_only: request.video_only,
        format: request.format.clone(),
        output: request.output_dir.clone(),
        search: None,
        yes: true,
    };

    let common = normalize_common_args(platform, args, &cfg)
        .map_err(|err| DriveError::Failed(err.to_string()))?;

    let downloader = prepare_download_environment(&cfg, &common.output_dir)
        .await
        .map_err(|err| DriveError::Failed(err.to_string()))?;

    set_status(
        state,
        session_id,
        SessionStatus::FetchingMetadata,
        Some(status_event(
            session_id,
            &SessionStatus::FetchingMetadata,
            None,
        )),
    );

    let video = fetch_video(&downloader, &cfg, &url).await.map_err(|err| {
        let message = match platform.auth_hint() {
            Some(hint) => format!("{err}\n{hint}"),
            None => err.to_string(),
        };
        DriveError::Failed(message)
    })?;

    broadcast(
        state,
        ServerEvent::Metadata {
            session_id,
            title: video.title.clone(),
            uploader: video.uploader.clone(),
            thumbnail: video.thumbnail.clone(),
            duration: video.duration,
        },
    );

    let filename = format!("{}.{}", sanitise_filename(&video.title), common.format);
    let output_path = common.output_dir.join(&filename);

    set_status(
        state,
        session_id,
        SessionStatus::Downloading,
        Some(status_event(session_id, &SessionStatus::Downloading, None)),
    );

    let progress = indicatif::ProgressBar::hidden();
    let mut events = downloader.subscribe_events();
    let bridge_state = Arc::clone(state);
    let bridge = tokio::spawn(async move {
        loop {
            match events.recv().await {
                Ok(event) => translate_event(&bridge_state, session_id, &event),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    let download_future =
        execute_download(&cfg, &downloader, &video, &output_path, &common, &progress);
    tokio::pin!(download_future);

    let outcome: Result<PathBuf, DriveError> = tokio::select! {
        result = &mut download_future => result.map_err(|err| DriveError::Failed(err.to_string())),
        _ = cancel.cancelled() => Err(DriveError::Cancelled),
    };

    bridge.abort();

    let temp_dir = cfg.download_path_expanded();
    let _ = cleanup_download_artifacts(&temp_dir, cfg.no_progress).await;

    outcome
}

/// Dispatches to the same lower-level download functions the CLI path uses, picking
/// the mode from the normalized request exactly like `execute_common_download` does.
async fn execute_download(
    cfg: &Config,
    downloader: &Downloader,
    video: &Video,
    output_path: &Path,
    request: &CommonRequest,
    progress: &indicatif::ProgressBar,
) -> anyhow::Result<PathBuf> {
    if request.audio_only {
        execute_audio_download(downloader, video, output_path, request, progress).await
    } else if request.video_only {
        execute_video_only_download(downloader, video, output_path, request, progress).await
    } else {
        let filename = output_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "download".to_string());
        execute_full_download(
            cfg,
            downloader,
            video,
            output_path,
            &filename,
            request,
            progress,
        )
        .await
    }
}

/// Translates the engine's own `DownloadEvent` stream into `ServerEvent` broadcasts —
/// the "event bridge". Each session gets a freshly built `Downloader`, so every event
/// observed here belongs to this session; no `download_id` matching is required.
fn translate_event(state: &AppState, session_id: Uuid, event: &DownloadEvent) {
    match event {
        DownloadEvent::DownloadProgress {
            downloaded_bytes,
            total_bytes,
            speed_bytes_per_sec,
            eta_seconds,
            ..
        } => {
            let percent = if *total_bytes > 0 {
                (*downloaded_bytes as f32 / *total_bytes as f32) * 100.0
            } else {
                0.0
            };
            set_progress(state, session_id, percent);
            broadcast(
                state,
                ServerEvent::Progress {
                    session_id,
                    percent,
                    speed: format!("{}/s", format_size(speed_bytes_per_sec.round() as u64)),
                    eta: format_eta(*eta_seconds),
                    size: format_size(*total_bytes),
                },
            );
        }
        DownloadEvent::PostProcessStarted { .. } => {
            set_status(
                state,
                session_id,
                SessionStatus::Merging,
                Some(status_event(session_id, &SessionStatus::Merging, None)),
            );
        }
        _ => {}
    }
}

fn format_eta(seconds: Option<u64>) -> String {
    match seconds {
        Some(total) => format!("{:02}:{:02}", total / 60, total % 60),
        None => "--:--".to_string(),
    }
}

fn status_event(session_id: Uuid, status: &SessionStatus, message: Option<String>) -> ServerEvent {
    ServerEvent::Status {
        session_id,
        status: status.as_str().to_string(),
        message,
    }
}

fn broadcast(state: &AppState, event: ServerEvent) {
    let _ = state.event_tx.send(event);
}

fn set_status(
    state: &AppState,
    session_id: Uuid,
    status: SessionStatus,
    event: Option<ServerEvent>,
) {
    if let Some(mut session) = state.sessions.get_mut(&session_id) {
        session.status = status;
    }
    if let Some(event) = event {
        broadcast(state, event);
    }
}

fn set_progress(state: &AppState, session_id: Uuid, percent: f32) {
    if let Some(mut session) = state.sessions.get_mut(&session_id) {
        session.progress = percent;
    }
}

fn finish(state: &AppState, session_id: Uuid, status: SessionStatus, event: ServerEvent) {
    if let Some(mut session) = state.sessions.get_mut(&session_id) {
        session.status = status;
        if let ServerEvent::Error { message, .. } = &event {
            session.error_message = Some(message.clone());
        }
    }
    broadcast(state, event);
    evict_finished_sessions(&state.sessions);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_eta_renders_minutes_and_seconds() {
        assert_eq!(format_eta(Some(75)), "01:15");
        assert_eq!(format_eta(Some(5)), "00:05");
        assert_eq!(format_eta(None), "--:--");
    }

    #[test]
    fn status_event_carries_lowercase_status_string() {
        let session_id = Uuid::new_v4();
        let event = status_event(session_id, &SessionStatus::Merging, None);

        match event {
            ServerEvent::Status {
                status, message, ..
            } => {
                assert_eq!(status, "merging");
                assert_eq!(message, None);
            }
            other => panic!("expected a Status event, got {other:?}"),
        }
    }
}
