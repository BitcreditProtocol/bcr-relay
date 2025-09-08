pub mod file_store;

use std::io::Write;

use axum::{
    Json,
    body::{Body, Bytes},
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use nostr::{
    hashes::{
        Hash,
        sha256::{self, Hash as Sha256Hash},
    },
    types::Url,
};
use serde::Serialize;
use tracing::{error, info};

use crate::AppState;

const MAX_FILE_SIZE_BYTES: usize = 1_000_000; // ~1 MB
const ENCRYPTION_PUB_KEY_BYTE_LEN: usize = 65; // we use uncompressed keys

/// For now, the only parts of the API we implement are
/// GET /<sha256> - get a file
/// PUT /upload - upload a file
///
/// Both endpoints work without Authorization, since all uploaded content is supposed to be encrypted
/// by the uploader (but potentially for someone else to decrypt).

#[derive(Debug, Clone, Serialize)]
pub struct BlobDescriptor {
    sha256: Sha256Hash,
    url: Url,
    size: usize,
    uploaded: i64,
}

#[derive(Debug)]
pub struct File {
    pub hash: Sha256Hash,
    pub bytes: Vec<u8>,
    pub size: i32,
}

impl BlobDescriptor {
    pub fn new(base_url: Url, hash: Sha256Hash, size: usize) -> Result<Self, anyhow::Error> {
        Ok(Self {
            sha256: hash,
            size,
            url: base_url.join(&hash.to_string())?,
            uploaded: chrono::Utc::now().timestamp(),
        })
    }
}

/// Checks the file size, hashes the file and stores it in the database, returning a
/// blob descriptor.
/// If the file already exists - simply returns the descriptor
pub async fn handle_upload(State(state): State<AppState>, body: Bytes) -> impl IntoResponse {
    let size = body.len();

    info!("Upload File called for {} bytes", size);
    // check size
    if size > MAX_FILE_SIZE_BYTES {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("File too big - max {MAX_FILE_SIZE_BYTES} bytes"),
        )
            .into_response();
    }

    if size == 0 {
        return (StatusCode::BAD_REQUEST, "Empty body").into_response();
    }
    // validate it's an ECIES/secp256k1 encrypted blob by checking if it starts with an ephemeral secp256k1 pub key
    // this is not a 100% guarantee (which is impossible), but rather a pretty reliable heuristic
    if size < ENCRYPTION_PUB_KEY_BYTE_LEN {
        error!("Non-encrypted Upload rejected - not big enough");
        return (StatusCode::BAD_REQUEST, "Invalid body").into_response();
    }
    let pubkey_bytes = &body[0..ENCRYPTION_PUB_KEY_BYTE_LEN];
    if let Err(e) = nostr::secp256k1::PublicKey::from_slice(pubkey_bytes) {
        error!("Non-encrypted Upload rejected: {e}");
        return (StatusCode::BAD_REQUEST, "Invalid body").into_response();
    }

    // create hash
    let mut hash_engine = sha256::HashEngine::default();
    if let Err(e) = hash_engine.write_all(&body) {
        error!("Error while hashing {size} bytes: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_SERVER_ERROR").into_response();
    }
    let hash = sha256::Hash::from_engine(hash_engine);

    let file = File {
        hash,
        bytes: body.into(),
        size: size as i32,
    };

    // store
    if let Err(e) = state.file_store.insert(file).await {
        error!("Error while storing {size} bytes with hash {hash}: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_SERVER_ERROR").into_response();
    }

    // return blob descriptor
    let blob_desc = BlobDescriptor::new(state.cfg.host_url, hash, size).unwrap();
    (StatusCode::OK, Json(blob_desc)).into_response()
}

/// Checks if there is a file with the given hash and returns it as application/octet-stream
/// since all our files are encrypted
pub async fn handle_get_file(
    State(state): State<AppState>,
    Path(hash): Path<Sha256Hash>,
) -> impl IntoResponse {
    info!("Get File called with hash {hash}");

    let file = match state.file_store.get(&hash).await {
        Ok(Some(file)) => file,
        Ok(None) => {
            error!("No file found with hash {hash}");
            return (StatusCode::NOT_FOUND, "NOT_FOUND").into_response();
        }
        Err(e) => {
            error!("Error while fetching file with hash {hash}: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_SERVER_ERROR").into_response();
        }
    };

    match Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/octet-stream")
        .body(Body::from(file.bytes))
    {
        Ok(resp) => resp,
        Err(e) => {
            error!("Error while creating response for {hash}: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_SERVER_ERROR").into_response()
        }
    }
}

pub async fn handle_list(Path(_pub_key): Path<String>) -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, "NOT_IMPLEMENTED")
}

pub async fn handle_mirror() -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, "NOT_IMPLEMENTED")
}

pub async fn handle_media() -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, "NOT_IMPLEMENTED")
}

pub async fn handle_report() -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, "NOT_IMPLEMENTED")
}

pub async fn handle_delete(Path(_hash): Path<String>) -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, "NOT_IMPLEMENTED")
}

pub async fn handle_upload_head() -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, "NOT_IMPLEMENTED")
}

pub async fn handle_get_file_head(Path(_hash): Path<String>) -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, "NOT_IMPLEMENTED")
}
