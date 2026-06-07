mod download;
mod routes;
mod state;
mod ws;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::response::Html;
use axum::routing::{delete, get, post};
use axum::Router;
use colored::Colorize;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

use crate::cli::ServeArgs;
use crate::config::Config;
use crate::sandbox;
use crate::tui;

use state::AppState;

const PLACEHOLDER_HTML: &str = include_str!("placeholder.html");
const SHUTDOWN_GRACE: Duration = Duration::from_secs(10);
const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Assembles the Axum router shared by `start` and the integration tests in
/// `routes`/`ws` — a permissive CORS layer is fine for same-origin use now and will
/// be tightened once the real frontend is embedded same-origin in Phase 2.
pub(crate) fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(placeholder))
        .route(
            "/api/config",
            get(routes::get_config).put(routes::put_config),
        )
        .route("/api/fetch-metadata", post(routes::fetch_metadata))
        .route("/api/download", post(routes::start_download))
        .route(
            "/api/download/{session_id}",
            delete(routes::cancel_download),
        )
        .route("/api/downloads", get(routes::list_downloads))
        .route("/ws", get(ws::ws_handler))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn placeholder() -> Html<&'static str> {
    Html(PLACEHOLDER_HTML)
}

/// Starts the server: ensures sandboxed binaries exist (same check the CLI path
/// performs), builds shared state, binds, prints the URL, optionally opens a
/// browser, and serves until Ctrl-C triggers a graceful shutdown.
pub(crate) async fn start(args: ServeArgs, config: Config) -> Result<()> {
    sandbox::ensure_installed(&config)
        .await
        .context("Failed to prepare sandboxed binaries for the server")?;

    let state = Arc::new(AppState::new(config));
    let app = router(Arc::clone(&state));

    let addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .with_context(|| format!("Invalid host/port combination: {}:{}", args.host, args.port))?;

    if !is_loopback(&args.host) {
        println!(
            "{} binding to a non-loopback address with no authentication — \
             anyone on your network will be able to reach this server.",
            "Warning:".yellow().bold()
        );
    }

    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("Failed to bind to {addr}"))?;

    let url = format!("http://{addr}");
    println!("{} {}", "vdl server running →".green().bold(), url);

    if args.open {
        open_in_browser(&url);
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(Arc::clone(&state)))
        .await
        .context("Server exited with an error")
}

fn is_loopback(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

fn open_in_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("cmd")
        .args(["/C", "start", url])
        .spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();

    if let Err(err) = result {
        tui::print_warning(&format!("Failed to open browser at {url}: {err}"));
    }
}

/// Resolves once Ctrl-C is pressed. Cancels every still-active session (the same
/// `abort.cancel()` path `DELETE /api/download/:id` uses) and then waits — polling
/// up to [`SHUTDOWN_GRACE`] — for `cleanup_download_artifacts` calls in flight to
/// finish before `axum::serve` tears the listener down.
async fn shutdown_signal(state: Arc<AppState>) {
    let _ = tokio::signal::ctrl_c().await;
    println!(
        "\n{} cancelling active downloads...",
        "Shutting down —".yellow().bold()
    );

    for entry in state.sessions.iter() {
        if !entry.value().status.is_finished() {
            entry.value().abort.cancel();
        }
    }

    let deadline = tokio::time::Instant::now() + SHUTDOWN_GRACE;
    while tokio::time::Instant::now() < deadline {
        let still_active = state
            .sessions
            .iter()
            .any(|entry| !entry.value().status.is_finished());
        if !still_active {
            break;
        }
        tokio::time::sleep(SHUTDOWN_POLL_INTERVAL).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_hosts_are_recognised() {
        assert!(is_loopback("127.0.0.1"));
        assert!(is_loopback("localhost"));
        assert!(is_loopback("::1"));
        assert!(!is_loopback("0.0.0.0"));
        assert!(!is_loopback("192.168.1.10"));
    }
}
