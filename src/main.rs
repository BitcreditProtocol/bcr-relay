use anyhow::Result;
use axum::{
    Router,
    http::{HeaderValue, StatusCode, Uri, header::ACCESS_CONTROL_ALLOW_ORIGIN},
    response::IntoResponse,
    serve,
};
use nostr_relay_builder::{LocalRelay, RelayBuilder};
use tower_http::set_header::SetResponseHeaderLayer;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    let listen_address = "0.0.0.0:8080";
    tracing_subscriber::fmt::init();
    info!("Starting relay...");

    let app_state = AppState::new().await?;
    let app = Router::new()
        .with_state(app_state)
        .fallback(handle_404)
        .layer(SetResponseHeaderLayer::if_not_present(
            ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("*"),
        ));

    info!("Listening on {}", listen_address);
    if let Ok(listener) = tokio::net::TcpListener::bind(listen_address).await {
        serve(listener, app).await?;
    } else {
        error!("Failed to bind to listen address {}", listen_address);
    }
    Ok(())
}

/// Handle all 404 errors as a fallback
pub async fn handle_404(uri: Uri) -> impl IntoResponse {
    info!("404 not found: {uri}");
    StatusCode::NOT_FOUND
}

#[derive(Clone)]
pub struct AppState {
    pub relay: LocalRelay,
}

impl AppState {
    pub async fn new() -> Result<Self> {
        let builder = RelayBuilder::default();
        let relay = LocalRelay::new(builder).await?;
        Ok(Self { relay })
    }
}
