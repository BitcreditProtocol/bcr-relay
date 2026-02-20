mod blossom;
mod db;
mod rate_limit;
mod relay;

use anyhow::Result;
use axum::extract::DefaultBodyLimit;
use axum::{
    Json, Router,
    extract::{ConnectInfo, State},
    http::{StatusCode, Uri},
    response::IntoResponse,
    routing::{any, delete, get, head, put},
    serve,
};
use axum_raw_websocket::RawSocketUpgrade;
use blossom::file_store::FileStoreApi;
use clap::Parser;
use deadpool_postgres::Manager;
use deadpool_postgres::ManagerConfig;
use deadpool_postgres::Pool;
use deadpool_postgres::RecyclingMethod;
use nostr::types::Url;
use nostr_relay_builder::LocalRelay;
use relay::RelayConfig;
use serde::Serialize;
use std::{net::SocketAddr, sync::Arc};
use tokio_postgres::NoTls;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info};

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
        .route(
            "/upload",
            put(blossom::handle_upload).layer(DefaultBodyLimit::max(config.max_file_size_bytes)),
        )
        .route("/upload", head(blossom::handle_upload_head))
        .route("/{hash}", get(blossom::handle_get_file))
        .route("/{hash}", head(blossom::handle_get_file_head))
        .route("/", delete(blossom::handle_delete))
        .route("/relay_features", get(features_handler))
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

#[derive(Debug, Clone, Serialize)]
struct RelayFeatures {
    pub relay_version: String,
    pub features: Vec<RelayFeature>,
}

#[derive(Debug, Clone, Serialize)]
struct RelayFeature {
    pub name: String,
    pub version: String,
}

/// An endpoint to check the capabilities of this relay in the context of our custom relay implementations
async fn features_handler() -> impl IntoResponse {
    let features = RelayFeatures {
        relay_version: "0.1.0".to_string(),
        features: vec![RelayFeature {
            name: "file_upload".to_string(),
            version: "1".to_string(),
        }],
    };
    (StatusCode::OK, Json(features)).into_response()
}

/// Handle all 404 errors as a fallback
async fn handle_404(uri: Uri) -> impl IntoResponse {
    info!("404 not found: {uri}");
    StatusCode::NOT_FOUND
}
#[derive(Clone)]
struct AppConfig {
    pub host_url: Url,
    pub max_file_size_bytes: usize,
}

#[derive(Clone)]
struct AppState {
    pub relay: LocalRelay,
    pub cfg: AppConfig,
    pub file_store: Arc<dyn FileStoreApi>,
}

impl AppState {
    pub async fn new(config: &RelayConfig) -> Result<Self> {
        let pool = postgres_connection_pool(&config.db_connection_string()).await?;
        let db = db::PostgresStore::new(pool.clone());
        db.init().await?;
        let store = Arc::new(db);

        Ok(Self {
            relay: relay::init(config, pool).await?,
            cfg: AppConfig {
                host_url: config.host_url.clone(),
                max_file_size_bytes: config.max_file_size_bytes,
            },
            file_store: store.clone(),
        })
    }
}

async fn postgres_connection_pool(db_url: &str) -> Result<Pool> {
    let cfg: tokio_postgres::Config = db_url.parse()?;
    let mgr_config = ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    };
    Ok(Pool::builder(Manager::from_config(cfg, NoTls, mgr_config))
        .max_size(16)
        .build()?)
}
