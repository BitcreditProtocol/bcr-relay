mod relay;

use std::net::SocketAddr;

use anyhow::Result;
use axum::{
    Router,
    extract::{ConnectInfo, State},
    http::{HeaderValue, StatusCode, Uri, header::ACCESS_CONTROL_ALLOW_ORIGIN},
    response::IntoResponse,
    routing::any,
    serve,
};
use axum_raw_websocket::RawSocketUpgrade;
use clap::Parser;
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
struct AppState {
    pub relay: LocalRelay,
}

impl AppState {
    pub async fn new(config: &RelayConfig) -> Result<Self> {
        Ok(Self {
            relay: relay::init(config).await?,
        })
    }
}
