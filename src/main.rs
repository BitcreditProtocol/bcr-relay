mod blossom;
mod db;
mod notification;
mod relay;
mod util;

use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use axum::{
    extract::{ConnectInfo, State},
    http::{StatusCode, Uri},
    response::IntoResponse,
    routing::{any, delete, get, head, post, put},
    serve, Router,
};
use axum_raw_websocket::RawSocketUpgrade;
use blossom::file_store::FileStoreApi;
use clap::Parser;
use nostr::types::Url;
use nostr_relay_builder::LocalRelay;
use relay::RelayConfig;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info};

use crate::notification::{
    email::{
        mailjet::{MailjetConfig, MailjetService},
        EmailService,
    },
    notification_store::NotificationStoreApi,
};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    info!("Starting relay...");

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

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
        .route("/notifications/v1/start", post(notification::start))
        .route("/notifications/v1/register", post(notification::register))
        .route(
            "/notifications/confirm_email",
            get(notification::confirm_email),
        )
        .route("/", any(websocket_handler))
        .fallback(handle_404)
        .with_state(app_state)
        .layer(cors);

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
    pub email_from_address: String,
}

#[derive(Clone)]
struct AppState {
    pub relay: LocalRelay,
    pub cfg: AppConfig,
    pub file_store: Arc<dyn FileStoreApi>,
    pub notification_store: Arc<dyn NotificationStoreApi>,
    pub email_service: Arc<dyn EmailService>,
}

impl AppState {
    pub async fn new(config: &RelayConfig) -> Result<Self> {
        let db = db::PostgresStore::new(&config.db_connection_string()).await?;
        db.init().await?;
        let store = Arc::new(db);

        let email_service = MailjetService::new(&MailjetConfig {
            api_key: config.email_api_key.clone(),
            api_secret_key: config.email_api_secret_key.clone(),
            url: config.email_url.clone(),
        });
        Ok(Self {
            relay: relay::init(config).await?,
            cfg: AppConfig {
                host_url: config.host_url.clone(),
                email_from_address: config.email_from_address.clone(),
            },
            file_store: store.clone(),
            notification_store: store,
            email_service: Arc::new(email_service),
        })
    }
}
