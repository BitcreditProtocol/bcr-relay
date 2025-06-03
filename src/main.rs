mod blossom;
mod relay;

use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use axum::{
    extract::{ConnectInfo, State},
    http::{header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue, StatusCode, Uri},
    response::IntoResponse,
    routing::{any, delete, get, head, put},
    serve, Router,
};
use axum_raw_websocket::RawSocketUpgrade;
use blossom::file_store::FileStoreApi;
use clap::Parser;
use nostr::types::Url;
use nostr_relay_builder::LocalRelay;
use relay::RelayConfig;
use tower_http::set_header::SetResponseHeaderLayer;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    info!("Starting relay...");

    let config = RelayConfig::parse();

    let app_state = AppState::new(&config).await?;
    let app = Router::new()
        .route("/list/{pub_key}", get(blossom::handle_list))
        .route("/mirror", put(blossom::handle_mirror))
        .route("/media", any(blossom::handle_media))
        .route("/report", any(blossom::handle_report))
        .route("/upload", put(blossom::handle_upload))
        .route("/upload", head(blossom::handle_upload_head))
        .route("/{hash}", get(blossom::handle_get_file))
        .route("/{hash}", head(blossom::handle_get_file_head))
        .route("/", delete(blossom::handle_delete))
        .route("/", any(websocket_handler))
        .fallback(handle_404)
        .with_state(app_state)
        .layer(SetResponseHeaderLayer::if_not_present(
            ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("*"),
        ));

    info!("Listening on {}", &config.listen_address);
    if let Ok(listener) = tokio::net::TcpListener::bind(&config.listen_address).await {
        serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await?;
    } else {
        error!(
            "Failed to bind to listen address {}",
            &config.listen_address
        );
    }
    Ok(())
}

async fn websocket_handler(
    ws: RawSocketUpgrade,
    ConnectInfo(address): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(async move |socket| state.relay.take_connection(socket, address).await.unwrap())
}

/// Handle all 404 errors as a fallback
async fn handle_404(uri: Uri) -> impl IntoResponse {
    info!("404 not found: {uri}");
    StatusCode::NOT_FOUND
}
#[derive(Clone)]
struct AppConfig {
    pub host_url: Url,
}

#[derive(Clone)]
struct AppState {
    pub relay: LocalRelay,
    pub cfg: AppConfig,
    pub file_store: Arc<dyn FileStoreApi>,
}

impl AppState {
    pub async fn new(config: &RelayConfig) -> Result<Self> {
        let file_store =
            blossom::file_store::PostgresFileStore::new(&config.db_connection_string()).await?;
        file_store.init().await?;
        Ok(Self {
            relay: relay::init(config).await?,
            cfg: AppConfig {
                host_url: config.host_url.clone(),
            },
            file_store: Arc::new(file_store),
        })
    }
}
