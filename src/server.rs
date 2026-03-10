use std::{net::SocketAddr, sync::Arc};

#[cfg(debug_assertions)]
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use axum::{
    Json, Router,
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{StatusCode, Uri, header},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::{
    net::TcpListener,
    sync::{RwLock, mpsc, oneshot},
};

#[cfg(debug_assertions)]
use tokio::fs;
use tracing::{info, warn};

#[cfg(not(debug_assertions))]
use rust_embed::Embed;

use crate::{
    cli::ServerArgs,
    protocol::WireMessage,
    registry::{PeerTx, Registry},
    server_config,
    util::mask_secret,
};

#[cfg(not(debug_assertions))]
#[derive(Embed)]
#[folder = "hurryvc-ui/dist/"]
struct EmbeddedAssets;

#[derive(Clone)]
pub struct AppState {
    pub master_key: Arc<String>,
    pub registry: Arc<RwLock<Registry>>,
}

#[derive(Debug, Deserialize)]
struct SessionsQuery {
    master_key: String,
    group_key: String,
}

pub async fn run(args: ServerArgs) -> Result<()> {
    let listen: SocketAddr = args
        .listen
        .parse()
        .with_context(|| format!("invalid listen address: {}", args.listen))?;
    let master_key = server_config::load_or_create()?.master_key;
    info!("server master key {}", mask_secret(&master_key));

    let state = AppState {
        master_key: Arc::new(master_key),
        registry: Arc::new(RwLock::new(Registry::default())),
    };

    let app = build_router(state);
    let listener = TcpListener::bind(listen).await?;
    info!("server listening on http://{}", listen);
    axum::serve(listener, app).await?;
    Ok(())
}

fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index_handler))
        .route("/index.html", get(index_handler))
        .route("/favicon.ico", get(asset_handler))
        .route("/assets/{*path}", get(asset_handler))
        .route("/api/health", get(|| async { Json(serde_json::json!({"ok": true})) }))
        .route("/api/sessions", get(api_sessions))
        .route("/ws/producer", get(ws_producer))
        .route("/ws/consumer", get(ws_consumer))
        .fallback(get(index_handler))
        .with_state(state)
}

async fn index_handler(State(_state): State<AppState>) -> Response {
    if let Some(response) = preferred_asset_response("index.html").await {
        return response;
    }
    missing_ui_response()
}

async fn asset_handler(State(_state): State<AppState>, uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/').to_string();
    if let Some(response) = preferred_asset_response(&path).await {
        return response;
    }
    (StatusCode::NOT_FOUND, "404 Not Found").into_response()
}

async fn preferred_asset_response(path: &str) -> Option<Response> {
    #[cfg(debug_assertions)]
    {
        if let Some(response) = debug_disk_asset_response(path).await {
            return Some(response);
        }
        None
    }
    #[cfg(not(debug_assertions))]
    {
        embedded_asset_response(path)
    }
}

#[cfg(debug_assertions)]
async fn debug_disk_asset_response(path: &str) -> Option<Response> {
    let web_dir = crate::util::debug_web_dir();
    disk_asset_response(&web_dir, path).await
}

#[cfg(debug_assertions)]
async fn disk_asset_response(web_dir: &Path, path: &str) -> Option<Response> {
    let relative = sanitize_relative_path(path)?;
    let full_path = web_dir.join(relative);
    let metadata = fs::metadata(&full_path).await.ok()?;
    if !metadata.is_file() {
        return None;
    }
    let content = fs::read(&full_path).await.ok()?;
    let mime = mime_guess::from_path(&full_path).first_or_octet_stream();
    Some(([(header::CONTENT_TYPE, mime.as_ref())], content).into_response())
}

#[cfg(not(debug_assertions))]
fn embedded_asset_response(path: &str) -> Option<Response> {
    let content = EmbeddedAssets::get(path)?;
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    Some(([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response())
}

#[cfg(debug_assertions)]
fn sanitize_relative_path(path: &str) -> Option<PathBuf> {
    let mut sanitized = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            Component::Normal(part) => sanitized.push(part),
            Component::CurDir => {}
            Component::RootDir => {}
            Component::Prefix(_) | Component::ParentDir => return None,
        }
    }
    (!sanitized.as_os_str().is_empty()).then_some(sanitized)
}

fn missing_ui_response() -> Response {
    Html(
        "<html><body><h1>hurryvc</h1><p>Frontend UI is unavailable. Release builds should serve embedded assets; debug builds should read hurryvc-ui/dist from the project directory. Run npm --prefix hurryvc-ui run build.</p></body></html>",
    )
    .into_response()
}

async fn api_sessions(
    State(state): State<AppState>,
    Query(query): Query<SessionsQuery>,
) -> Response {
    if query.master_key != *state.master_key {
        return (StatusCode::UNAUTHORIZED, "invalid master key").into_response();
    }
    let sessions = state
        .registry
        .read()
        .await
        .sessions_for_group(&query.group_key);
    Json(serde_json::json!({ "sessions": sessions })).into_response()
}

async fn ws_producer(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_producer_socket(state, socket))
}

async fn ws_consumer(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_consumer_socket(state, socket))
}

async fn handle_producer_socket(state: AppState, socket: WebSocket) {
    let (tx, mut rx) = mpsc::unbounded_channel::<WireMessage>();
    let (close_tx, mut close_rx) = oneshot::channel();
    let (mut sender, mut receiver) = socket.split();
    let writer = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if send_axum_message(&mut sender, &message).await.is_err() {
                break;
            }
        }
    });

    let first_message = tokio::select! {
        _ = &mut close_rx => None,
        message = receiver.next() => message,
    };
    let Some(Ok(message)) = first_message else {
        let _ = writer.await;
        return;
    };
    let Ok(WireMessage::ProducerHello { payload, .. }) = parse_axum_message(message) else {
        let _ = tx.send(WireMessage::server_kick("expected producer_hello as first message"));
        let _ = writer.await;
        return;
    };
    if payload.master_key != *state.master_key {
        let _ = tx.send(WireMessage::server_kick("invalid master key"));
        let _ = writer.await;
        return;
    }

    let register = {
        let mut registry = state.registry.write().await;
        registry.register_producer(payload.clone(), tx.clone(), close_tx)
    };
    let producer_id = register.producer_id.clone();
    dispatch_messages(register.messages);
    let _ = tx.send(WireMessage::producer_welcome(producer_id.clone()));
    info!(
        "producer {} registered group={} name={} pid={}",
        producer_id, payload.production_group_key, payload.producer_name, payload.pid
    );

    loop {
        let next_message = tokio::select! {
            _ = &mut close_rx => {
                warn!("producer {} closed by replacement", producer_id);
                break;
            }
            message = receiver.next() => message,
        };
        let Some(Ok(message)) = next_message else {
            let messages = state
                .registry
                .write()
                .await
                .remove_producer(&producer_id, None, None, "producer disconnected");
            dispatch_messages(messages);
            break;
        };
        match parse_axum_message(message) {
            Ok(WireMessage::ProducerPing { .. }) => {}
            Ok(WireMessage::TermSnapshot { payload, .. }) => {
                let messages = state
                    .registry
                    .write()
                    .await
                    .update_snapshot(&payload.producer_id, payload.snapshot);
                dispatch_messages(messages);
            }
            Ok(WireMessage::TermDelta { payload, .. }) => {
                let messages = state
                    .registry
                    .write()
                    .await
                    .update_delta(&payload.producer_id, payload.delta);
                dispatch_messages(messages);
            }
            Ok(WireMessage::ProducerExit { payload, .. }) => {
                let messages = state.registry.write().await.remove_producer(
                    &payload.producer_id,
                    payload.snapshot,
                    payload.exit_status,
                    payload.reason,
                );
                dispatch_messages(messages);
                break;
            }
            Ok(other) => warn!("unexpected producer message: {:?}", other),
            Err(error) => warn!("invalid producer message: {error:#}"),
        }
    }
    drop(tx);
    let _ = writer.await;
}

async fn handle_consumer_socket(state: AppState, socket: WebSocket) {
    let (tx, mut rx) = mpsc::unbounded_channel::<WireMessage>();
    let (close_tx, mut close_rx) = oneshot::channel();
    let (mut sender, mut receiver) = socket.split();
    let writer = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if send_axum_message(&mut sender, &message).await.is_err() {
                break;
            }
        }
    });

    let first_message = tokio::select! {
        _ = &mut close_rx => None,
        message = receiver.next() => message,
    };
    let Some(Ok(message)) = first_message else {
        let _ = writer.await;
        return;
    };
    let Ok(WireMessage::ConsumerHello { payload, .. }) = parse_axum_message(message) else {
        let _ = tx.send(WireMessage::consumer_error("expected consumer_hello as first message"));
        let _ = writer.await;
        return;
    };
    if payload.master_key != *state.master_key {
        let _ = tx.send(WireMessage::server_kick("invalid master key"));
        let _ = writer.await;
        return;
    }

    let register = {
        let mut registry = state.registry.write().await;
        registry.register_consumer(payload.clone(), tx.clone(), close_tx)
    };
    let consumer_id = register.consumer_id.clone();
    dispatch_messages(register.messages);
    info!(
        "consumer {} registered group={} client={:?}",
        consumer_id, payload.production_group_key, payload.client_info
    );

    loop {
        let next_message = tokio::select! {
            _ = &mut close_rx => {
                warn!("consumer {} closed by replacement", consumer_id);
                break;
            }
            message = receiver.next() => message,
        };
        let Some(Ok(message)) = next_message else {
            let messages = state.registry.write().await.remove_consumer(&consumer_id);
            dispatch_messages(messages);
            break;
        };
        match parse_axum_message(message) {
            Ok(WireMessage::ConsumerPing { .. }) => {}
            Ok(WireMessage::SubscribeSession { payload, .. }) => {
                let messages = state
                    .registry
                    .write()
                    .await
                    .subscribe_consumer(&consumer_id, &payload.producer_id);
                dispatch_messages(messages);
            }
            Ok(WireMessage::UnsubscribeSession { .. }) => {
                let messages = state.registry.write().await.unsubscribe_consumer(&consumer_id);
                dispatch_messages(messages);
            }
            Ok(WireMessage::ConsumerInput { payload, .. }) => {
                let messages = state.registry.read().await.producer_input(
                    &consumer_id,
                    &payload.producer_id,
                    payload.input,
                );
                dispatch_messages(messages);
            }
            Ok(other) => warn!("unexpected consumer message: {:?}", other),
            Err(error) => warn!("invalid consumer message: {error:#}"),
        }
    }
    drop(tx);
    let _ = writer.await;
}

fn dispatch_messages(messages: Vec<(PeerTx, WireMessage)>) {
    for (tx, message) in messages {
        let _ = tx.send(message);
    }
}

async fn send_axum_message(
    sender: &mut futures::stream::SplitSink<WebSocket, Message>,
    message: &WireMessage,
) -> Result<()> {
    sender
        .send(Message::Text(serde_json::to_string(message)?.into()))
        .await?;
    Ok(())
}

fn parse_axum_message(message: Message) -> Result<WireMessage> {
    match message {
        Message::Text(text) => Ok(serde_json::from_str(text.as_ref())?),
        Message::Binary(data) => Ok(serde_json::from_slice(&data)?),
        Message::Close(_) => Err(anyhow!("socket closed")),
        Message::Ping(_) | Message::Pong(_) => Err(anyhow!("control frame")),
    }
}
