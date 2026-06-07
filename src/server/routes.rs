use std::sync::Arc;
use std::time::Instant;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::commands::{fetch_video, prepare_download_environment, Platform};
use crate::config::Config;

use super::download::{self, DownloadRequest};
use super::state::{AppState, DownloadSession, SessionStatus};

/// Maps every handler failure to `{ "error": "<message>" }` with an explicit status —
/// no `unwrap()` anywhere in server code; every failure path returns a real response.
pub(crate) struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn unprocessable(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, message)
    }

    fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

pub(crate) async fn get_config(State(state): State<Arc<AppState>>) -> Json<Config> {
    Json(state.config.read().await.clone())
}

/// Full PUT-replace semantics — see the spec's "PUT /api/config" section for why partial
/// merge was rejected (`Config` derives `deny_unknown_fields` with mostly-required fields).
/// Parsed manually (not via the `Json<Config>` extractor) so deserialization failures map
/// to `422` rather than axum's default `400`.
pub(crate) async fn put_config(
    State(state): State<Arc<AppState>>,
    body: Bytes,
) -> Result<Json<Config>, ApiError> {
    let new_config: Config = serde_json::from_slice(&body)
        .map_err(|err| ApiError::unprocessable(format!("Invalid config: {err}")))?;

    if new_config.cookies_file.is_some() && new_config.cookies_from_browser.is_some() {
        return Err(ApiError::unprocessable(
            "cookies_file and cookies_from_browser are mutually exclusive.",
        ));
    }

    new_config
        .save()
        .map_err(|err| ApiError::internal(err.to_string()))?;

    *state.config.write().await = new_config.clone();

    Ok(Json(new_config))
}

#[derive(Debug, Deserialize)]
pub(crate) struct FetchMetadataRequest {
    url: String,
    platform: Platform,
}

#[derive(Debug, Serialize)]
pub(crate) struct MetadataResponse {
    title: String,
    uploader: Option<String>,
    duration: Option<i64>,
    thumbnail: Option<String>,
    /// Format identifiers (`Format::format_id`) — the simplest representation that lets
    /// a future UI list available formats without re-encoding the full `yt_dlp::Format`.
    formats: Vec<String>,
}

pub(crate) async fn fetch_metadata(
    State(state): State<Arc<AppState>>,
    Json(request): Json<FetchMetadataRequest>,
) -> Result<Json<MetadataResponse>, ApiError> {
    if request.url.trim().is_empty() {
        return Err(ApiError::unprocessable("Field \"url\" is required."));
    }

    let cfg = state.config.read().await.clone();
    let output_dir = cfg.download_path_expanded();
    let downloader = prepare_download_environment(&cfg, &output_dir)
        .await
        .map_err(|err| ApiError::unprocessable(err.to_string()))?;

    let video = fetch_video(&downloader, &cfg, &request.url)
        .await
        .map_err(|err| {
            let message = match request.platform.auth_hint() {
                Some(hint) => format!("{err}\n{hint}"),
                None => err.to_string(),
            };
            ApiError::unprocessable(message)
        })?;

    Ok(Json(MetadataResponse {
        title: video.title,
        uploader: video.uploader,
        duration: video.duration,
        thumbnail: video.thumbnail,
        formats: video
            .formats
            .iter()
            .map(|format| format.format_id.clone())
            .collect(),
    }))
}

/// Validates `url` is present *before* anything that could reach `normalize_common_args`'s
/// blocking `tui::prompt_input` fallback — see the spec's "Hazard guarded against" note.
/// Also rejects clip fields (no engine support) and Spotify (its request shape doesn't
/// match `CommonArgs`/this JSON body — out of scope for this phase's unified endpoint).
pub(crate) async fn start_download(
    State(state): State<Arc<AppState>>,
    Json(request): Json<DownloadRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let url = match &request.url {
        Some(url) if !url.trim().is_empty() => url.clone(),
        _ => return Err(ApiError::unprocessable("Field \"url\" is required.")),
    };

    if request.start.is_some() || request.end.is_some() {
        return Err(ApiError::unprocessable("Clipping is not yet supported."));
    }

    if request.platform == Platform::Spotify {
        return Err(ApiError::unprocessable(
            "Spotify downloads are not yet supported via the web API.",
        ));
    }

    let session_id = Uuid::new_v4();
    state.sessions.insert(
        session_id,
        DownloadSession {
            id: session_id,
            platform: request.platform,
            url,
            status: SessionStatus::Pending,
            progress: 0.0,
            abort: CancellationToken::new(),
            created_at: Instant::now(),
        },
    );

    let state_for_task = Arc::clone(&state);
    tokio::spawn(download::run(session_id, request, state_for_task));

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({ "session_id": session_id.to_string() })),
    ))
}

pub(crate) async fn cancel_download(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    match state.sessions.get(&session_id) {
        Some(session) => {
            session.abort.cancel();
            Ok(Json(json!({ "cancelled": true })))
        }
        None => Err(ApiError::not_found("Session not found.")),
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct SessionSummary {
    session_id: String,
    url: String,
    platform: Platform,
    status: String,
    progress: f32,
}

pub(crate) async fn list_downloads(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<SessionSummary>> {
    let mut entries: Vec<(Instant, SessionSummary)> = state
        .sessions
        .iter()
        .map(|entry| {
            let session = entry.value();
            (
                session.created_at,
                SessionSummary {
                    session_id: session.id.to_string(),
                    url: session.url.clone(),
                    platform: session.platform,
                    status: session.status.as_str().to_string(),
                    progress: session.progress,
                },
            )
        })
        .collect();

    entries.sort_by(|a, b| b.0.cmp(&a.0));
    Json(entries.into_iter().map(|(_, summary)| summary).collect())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use serde_json::{json, Value};
    use tower::ServiceExt;

    use super::*;
    use crate::config::PlatformQuality;
    use crate::server::router;

    /// A config that never resolves to a real filesystem path the test suite shouldn't
    /// touch — `no_progress: true` keeps any incidental engine calls quiet.
    fn sample_config() -> Config {
        Config {
            download_path: "~/Downloads/vdl".to_string(),
            default_format: "mp4".to_string(),
            default_video_quality: "1080".to_string(),
            platform_quality: PlatformQuality {
                youtube: "1080".to_string(),
                tiktok: "best".to_string(),
                instagram: "best".to_string(),
                twitter: "best".to_string(),
                spotify: "best".to_string(),
            },
            bins_dir: "~/.local/share/vdl/bins".to_string(),
            cookies_file: None,
            cookies_from_browser: None,
            confirm_before_download: true,
            search_results_count: 8,
            termux_mode: false,
            no_progress: true,
        }
    }

    fn test_state() -> Arc<AppState> {
        Arc::new(AppState::new(sample_config()))
    }

    async fn body_json(response: Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should be readable");
        serde_json::from_slice(&bytes).expect("body should be valid JSON")
    }

    fn json_request(method: &str, uri: &str, body: Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .expect("request should build")
    }

    fn empty_request(method: &str, uri: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .expect("request should build")
    }

    fn download_body(overrides: Value) -> Value {
        let mut base = json!({
            "platform": "yt",
            "url": "https://example.com/watch?v=test",
            "quality": null,
            "format": null,
            "audio_only": false,
            "video_only": false,
            "start": null,
            "end": null,
            "output_dir": null
        });
        let merged = base.as_object_mut().expect("base should be an object");
        for (key, value) in overrides
            .as_object()
            .expect("overrides should be an object")
        {
            merged.insert(key.clone(), value.clone());
        }
        base
    }

    #[tokio::test]
    async fn get_config_returns_current_config() {
        let app = router(test_state());

        let response = app
            .oneshot(empty_request("GET", "/api/config"))
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::OK);
        let body = body_json(response).await;
        assert_eq!(body["download_path"], "~/Downloads/vdl");
        assert_eq!(body["search_results_count"], 8);
    }

    #[tokio::test]
    async fn put_config_rejects_invalid_json() {
        let app = router(test_state());

        let request = Request::builder()
            .method("PUT")
            .uri("/api/config")
            .header("content-type", "application/json")
            .body(Body::from("{ not json"))
            .expect("request should build");

        let response = app.oneshot(request).await.expect("request should succeed");

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn put_config_rejects_conflicting_cookie_settings() {
        let app = router(test_state());

        let mut config = sample_config();
        config.cookies_file = Some("~/cookies.txt".to_string());
        config.cookies_from_browser = Some("firefox".to_string());
        let body = serde_json::to_value(&config).expect("config should serialize");

        let response = app
            .oneshot(json_request("PUT", "/api/config", body))
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = body_json(response).await;
        assert!(body["error"]
            .as_str()
            .expect("error should be a string")
            .contains("mutually exclusive"));
    }

    #[tokio::test]
    async fn fetch_metadata_rejects_empty_url_without_blocking() {
        let app = router(test_state());

        let response = tokio::time::timeout(
            Duration::from_secs(5),
            app.oneshot(json_request(
                "POST",
                "/api/fetch-metadata",
                json!({ "url": "", "platform": "yt" }),
            )),
        )
        .await
        .expect("request should not hang on stdin")
        .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn start_download_rejects_missing_url_without_blocking() {
        let app = router(test_state());
        let body = download_body(json!({ "url": null }));

        let response = tokio::time::timeout(
            Duration::from_secs(5),
            app.oneshot(json_request("POST", "/api/download", body)),
        )
        .await
        .expect("request should not hang on stdin")
        .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn start_download_rejects_clip_fields() {
        let app = router(test_state());
        let body = download_body(json!({ "start": 5.0 }));

        let response = app
            .oneshot(json_request("POST", "/api/download", body))
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = body_json(response).await;
        assert_eq!(body["error"], "Clipping is not yet supported.");
    }

    #[tokio::test]
    async fn start_download_rejects_spotify() {
        let app = router(test_state());
        let body = download_body(json!({
            "platform": "sp",
            "url": "https://open.spotify.com/track/abc"
        }));

        let response = app
            .oneshot(json_request("POST", "/api/download", body))
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn start_download_accepts_valid_request_and_lists_session() {
        let app = router(test_state());
        let body = download_body(json!({}));

        let response = app
            .clone()
            .oneshot(json_request("POST", "/api/download", body))
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let body = body_json(response).await;
        let session_id = body["session_id"]
            .as_str()
            .expect("session_id should be a string")
            .to_string();

        let list_response = app
            .oneshot(empty_request("GET", "/api/downloads"))
            .await
            .expect("request should succeed");

        assert_eq!(list_response.status(), StatusCode::OK);
        let sessions = body_json(list_response).await;
        let sessions = sessions.as_array().expect("response should be an array");
        assert!(sessions
            .iter()
            .any(|entry| entry["session_id"] == session_id));
    }

    #[tokio::test]
    async fn cancel_download_returns_404_for_unknown_session() {
        let app = router(test_state());

        let response = app
            .oneshot(empty_request(
                "DELETE",
                &format!("/api/download/{}", Uuid::new_v4()),
            ))
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn cancel_download_cancels_known_session() {
        let state = test_state();
        let session_id = Uuid::new_v4();
        let abort = CancellationToken::new();
        state.sessions.insert(
            session_id,
            DownloadSession {
                id: session_id,
                platform: Platform::YouTube,
                url: "https://example.com/watch?v=test".to_string(),
                status: SessionStatus::Downloading,
                progress: 12.0,
                abort: abort.clone(),
                created_at: Instant::now(),
            },
        );
        let app = router(Arc::clone(&state));

        let response = app
            .oneshot(empty_request(
                "DELETE",
                &format!("/api/download/{session_id}"),
            ))
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::OK);
        assert!(abort.is_cancelled());
    }
}
