use anyhow::anyhow;
use axum::{Json, extract::State, response::IntoResponse};
use borsh_derive::BorshSerialize;
use hickory_resolver::{
    Resolver, name_server::GenericConnector, proto::runtime::TokioRuntimeProvider,
};
use reqwest::StatusCode;
use serde::Deserialize;
use tracing::{error, warn};
use url::Url;

use crate::{
    AppState,
    rate_limit::RealIp,
    util::{self, is_blocked_proxy_host_ip, is_valid_proxy_url},
};

pub const PROXY_REQ_TIMEOUT_SEC: u64 = 5;
pub const PROXY_REQ_MAX_REDIRECTS: usize = 2;
pub const PROXY_MAX_BODY_SIZE: usize = 2 * 1024 * 1024; // 2 MB

#[derive(Debug, Clone)]
pub struct ProxyClient {
    pub dns_resolver: Resolver<GenericConnector<TokioRuntimeProvider>>,
    pub cl: reqwest::Client,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProxyReq {
    pub payload: ProxyReqPayload,
    pub signature: String,
}

#[derive(Debug, Clone, Deserialize, BorshSerialize)]
pub struct ProxyReqPayload {
    pub npub: String,
    pub url: String,
}

pub async fn req(
    RealIp(ip): RealIp,
    State(state): State<AppState>,
    Json(payload): Json<ProxyReq>,
) -> impl IntoResponse {
    let signature = payload.signature;
    let payload = payload.payload;

    let x_only_npub = match util::validate_npub(&payload.npub) {
        Ok(n) => n,
        Err(e) => {
            error!("Proxy req with invalid npub: {e}");
            return (StatusCode::BAD_REQUEST, "proxy_invalid_npub").into_response();
        }
    };

    // check that it's a valid URL
    let url = match Url::parse(&payload.url) {
        Ok(u) => u,
        Err(_) => {
            error!("Proxy req with invalid url: {}", &payload.url);
            return (StatusCode::BAD_REQUEST, "proxy_invalid_url").into_response();
        }
    };

    let mut rate_limiter = state.rate_limiter.lock().await;
    let allowed = rate_limiter.check(&ip.to_string(), None, Some(&payload.npub), None);
    drop(rate_limiter);

    if !allowed {
        warn!(
            "Rate limited req from {} with npub {}",
            &ip.to_string(),
            &payload.npub
        );
        return (StatusCode::TOO_MANY_REQUESTS, "proxy_rate_limit").into_response();
    }

    if let Err(e) = check_url(&url, &state.proxy_client).await {
        error!("Proxy req with invalid url: {e}");
        return (StatusCode::BAD_REQUEST, "proxy_invalid_url").into_response();
    }

    // make sure sender signed the request
    match util::verify_request(&payload, &signature, &x_only_npub) {
        Ok(true) => {
            match do_capped_req_with_validated_redirects(url.clone(), state.proxy_client).await {
                Ok((status, body_bytes)) => (status, body_bytes).into_response(),
                Err(e) => {
                    error!("Error during proxy request to {url}: {e}");
                    (StatusCode::INTERNAL_SERVER_ERROR, "proxy_invalid_request").into_response()
                }
            }
        }
        Ok(false) => {
            error!("proxy req check invalid signature error");
            (StatusCode::BAD_REQUEST, "proxy_invalid_signature").into_response()
        }
        Err(e) => {
            error!("proxy req check signature error: {e}");
            (StatusCode::BAD_REQUEST, "proxy_invalid_signature").into_response()
        }
    }
}

async fn do_capped_req_with_validated_redirects(
    url: Url,
    proxy_client: ProxyClient,
) -> Result<(reqwest::StatusCode, Vec<u8>), anyhow::Error> {
    let mut redirects = 0;
    let mut url = url;
    loop {
        let mut resp = proxy_client.cl.get(url.clone()).send().await?;

        if resp.status().is_redirection() {
            if redirects >= PROXY_REQ_MAX_REDIRECTS {
                return Err(anyhow!("too many redirects"));
            }
            let loc = match resp.headers().get(reqwest::header::LOCATION) {
                Some(l) => l.to_str()?,
                None => {
                    return Err(anyhow!("redirect without location"));
                }
            };

            // use new location as URL - safe for relative and absolute urls
            url = url.join(loc)?;
            check_url(&url, &proxy_client).await?;

            redirects += 1;
        } else {
            let status = resp.status();

            // Stream body to avoid too large payloads
            let mut body = Vec::new();
            while let Some(chunk) = resp.chunk().await? {
                if body.len() + chunk.len() > PROXY_MAX_BODY_SIZE {
                    return Err(anyhow::anyhow!("response too big"));
                }
                body.extend_from_slice(&chunk);
            }

            return Ok((status, body));
        }
    }
}

async fn check_url(url: &Url, proxy_client: &ProxyClient) -> Result<(), anyhow::Error> {
    if !is_valid_proxy_url(url) {
        return Err(anyhow!("invalid URL"));
    }
    let host = match url.host() {
        Some(h) => h,
        None => {
            return Err(anyhow!("invalid host"));
        }
    };
    match proxy_client.dns_resolver.lookup_ip(&host.to_string()).await {
        Ok(lookup) => {
            for ip in lookup.iter() {
                if is_blocked_proxy_host_ip(ip) {
                    return Err(anyhow!("invalid IP"));
                }
            }
        }
        Err(e) => {
            warn!("Error during DNS lookup: {e}");
            return Err(anyhow!("redirect with invalid URL DNS"));
        }
    };
    Ok(())
}
