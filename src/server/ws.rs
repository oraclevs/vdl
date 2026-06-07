use std::collections::HashSet;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use serde::Deserialize;
use uuid::Uuid;

use super::state::AppState;

/// "Client → Server" WebSocket message shapes, tagged by `"type"`.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    Subscribe { session_id: Uuid },
    Unsubscribe { session_id: Uuid },
    Ping,
}

pub(crate) async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Each connection subscribes to the shared `ServerEvent` broadcast and forwards an
/// event to the client only when its `session_id` is in this connection's subscribed
/// set — exactly the per-client filtering the WebSocket Protocol section specifies.
async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let mut events = state.event_tx.subscribe();
    let mut subscribed: HashSet<Uuid> = HashSet::new();

    loop {
        tokio::select! {
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ClientMessage>(&text) {
                            Ok(ClientMessage::Subscribe { session_id }) => {
                                subscribed.insert(session_id);
                            }
                            Ok(ClientMessage::Unsubscribe { session_id }) => {
                                subscribed.remove(&session_id);
                            }
                            Ok(ClientMessage::Ping) => {
                                let pong = Message::Text(r#"{"type":"pong"}"#.into());
                                if socket.send(pong).await.is_err() {
                                    break;
                                }
                            }
                            Err(_) => continue,
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => continue,
                    Some(Err(_)) => break,
                }
            }
            event = events.recv() => {
                match event {
                    Ok(event) if subscribed.contains(&event.session_id()) => {
                        let Ok(payload) = serde_json::to_string(&event) else { continue };
                        if socket.send(Message::Text(payload.into())).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures_util::{SinkExt, StreamExt};
    use serde_json::{json, Value};
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::Message as WsMessage;
    use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

    use super::*;
    use crate::config::{Config, PlatformQuality};
    use crate::server::router;
    use crate::server::state::ServerEvent;

    type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

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

    /// Binds an ephemeral local port, serves the real router on it, and returns the
    /// shared state plus a `ws://` URL — a raw WS handshake needs a real TCP socket,
    /// which `tower::ServiceExt::oneshot` cannot provide for an Upgrade response.
    async fn spawn_test_server() -> (Arc<AppState>, String) {
        let state = Arc::new(AppState::new(sample_config()));
        let app = router(Arc::clone(&state));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind to an ephemeral port");
        let addr = listener
            .local_addr()
            .expect("listener should expose its address");

        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("test server should run without error");
        });

        (state, format!("ws://{addr}/ws"))
    }

    async fn next_json(socket: &mut WsStream) -> Value {
        loop {
            let message = socket
                .next()
                .await
                .expect("socket should yield a message before closing")
                .expect("message should not be a protocol error");

            if let WsMessage::Text(text) = message {
                return serde_json::from_str(&text).expect("message should be valid JSON");
            }
        }
    }

    #[tokio::test]
    async fn ping_receives_pong() {
        let (_state, url) = spawn_test_server().await;
        let (mut socket, _response) = tokio_tungstenite::connect_async(&url)
            .await
            .expect("websocket handshake should succeed");

        socket
            .send(WsMessage::Text(
                json!({ "type": "ping" }).to_string().into(),
            ))
            .await
            .expect("ping should send");

        let response = next_json(&mut socket).await;
        assert_eq!(response["type"], "pong");
    }

    #[tokio::test]
    async fn subscribed_clients_receive_only_their_sessions_events() {
        let (state, url) = spawn_test_server().await;
        let (mut socket, _response) = tokio_tungstenite::connect_async(&url)
            .await
            .expect("websocket handshake should succeed");

        let watched = Uuid::new_v4();
        let ignored = Uuid::new_v4();

        socket
            .send(WsMessage::Text(
                json!({ "type": "subscribe", "session_id": watched })
                    .to_string()
                    .into(),
            ))
            .await
            .expect("subscribe should send");

        // Give the connection task a moment to process the subscribe message before
        // events are broadcast — otherwise both sends could race ahead of it.
        tokio::time::sleep(Duration::from_millis(50)).await;

        let _ = state.event_tx.send(ServerEvent::Status {
            session_id: ignored,
            status: "downloading".to_string(),
            message: None,
        });
        let _ = state.event_tx.send(ServerEvent::Status {
            session_id: watched,
            status: "downloading".to_string(),
            message: None,
        });

        let response = next_json(&mut socket).await;
        assert_eq!(response["type"], "status");
        assert_eq!(response["session_id"], watched.to_string());
    }
}
