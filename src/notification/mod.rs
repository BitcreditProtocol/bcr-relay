use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use base64::{engine::general_purpose::URL_SAFE, Engine as _};
use bitflags::bitflags;
use chrono::{DateTime, Duration, Utc};
use nostr::nips::nip19::FromBech32;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tracing::{error, warn};

use crate::{notification::email::build_email_confirmation_message, util, AppState};

pub mod email;
pub mod notification_store;

/// Maximum age of a challenge - we expect requests to be made immediately after each other
const CHALLENGE_EXPIRY_SECONDS: i64 = 120; // 2 minutes

/// Maximum age of an email confirmation
const EMAIL_CONFIRMATION_EXPIRY_SECONDS: i64 = 60 * 60 * 24; // 1 day

/// A challenge to validate the request comes from a given npub
#[derive(Debug)]
pub struct Challenge {
    pub npub: String,
    pub challenge: String,
    pub created_at: DateTime<Utc>,
}

/// Email confirmation state
#[derive(Debug)]
pub struct EmailConfirmation {
    pub npub: String,
    pub email: String,
    pub confirmed: bool,
    pub sent_at: DateTime<Utc>,
}

/// Email preferences state
#[derive(Debug)]
#[allow(unused)]
pub struct EmailPreferences {
    pub npub: String,
    pub enabled: bool,
    pub email: String,
    pub email_confirmed: bool,
    pub ebill_url: url::Url,
    pub flags: PreferencesFlags,
}

bitflags! {
/// A set of preference flags packed in an efficient way
#[derive(Debug, Clone, PartialEq, Eq)]
    pub struct PreferencesFlags: i64 {
        const IssueBill = 0b0001;
    }
}

impl Default for PreferencesFlags {
    fn default() -> Self {
        Self::IssueBill
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationStartReq {
    pub npub: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotificationStartResp {
    pub challenge: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorResp {
    pub msg: String,
}

#[derive(Deserialize)]
pub struct EmailConfirmationToken {
    pub token: String,
}

impl ErrorResp {
    pub fn new(msg: &str) -> Self {
        Self {
            msg: msg.to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SuccessResp {
    pub msg: String,
}

impl SuccessResp {
    pub fn new(msg: &str) -> Self {
        Self {
            msg: msg.to_owned(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationRegisterReq {
    pub npub: String,
    pub signed_challenge: String,
    pub ebill_url: url::Url,
    pub email: String,
}

/// Send back a random challenge to the caller, which we expect to be signed with their npub to validate
/// the request actually comes from the given npub
pub async fn start(
    State(state): State<AppState>,
    Json(payload): Json<NotificationStartReq>,
) -> impl IntoResponse {
    let parsed_npub = match nostr::PublicKey::from_bech32(&payload.npub) {
        Ok(npub) => npub,
        Err(e) => {
            error!("notification start with invalid npub: {e}");
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResp::new("Invalid npub")),
            )
                .into_response();
        }
    };
    if let Err(e) = parsed_npub.xonly() {
        error!("notification start with invalid npub: {e}");
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResp::new("Invalid npub")),
        )
            .into_response();
    }

    let mut random_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut random_bytes);
    let challenge = hex::encode(random_bytes);

    if let Err(e) = state
        .notification_store
        .insert_challenge_for_npub(payload.npub, challenge.clone())
        .await
    {
        error!("Could not persist challenge for npub: {e}");
    }

    (StatusCode::OK, Json(NotificationStartResp { challenge })).into_response()
}

/// We validate npub, email and signed challenge. If everything is OK, we send a confirmation email
/// and we create a stub for email preferences with a token to change them later
pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<NotificationRegisterReq>,
) -> impl IntoResponse {
    let parsed_npub = match nostr::PublicKey::from_bech32(&payload.npub) {
        Ok(npub) => npub,
        Err(e) => {
            error!("notification start with invalid npub: {e}");
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResp::new("Invalid npub")),
            )
                .into_response();
        }
    };
    let x_only = match parsed_npub.xonly() {
        Ok(x_only) => x_only,
        Err(e) => {
            error!("notification start with invalid npub: {e}");
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResp::new("Invalid npub")),
            )
                .into_response();
        }
    };

    if !email_address::EmailAddress::is_valid(&payload.email) {
        error!(
            "notification register with invalid email: {}",
            &payload.email
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResp::new("Invalid email")),
        )
            .into_response();
    }

    let challenge = match state
        .notification_store
        .get_challenge_for_npub(&payload.npub)
        .await
    {
        Ok(Some(c)) => c,
        Ok(None) => {
            error!(
                "notification register for npub {}, but no challenge",
                &payload.npub
            );
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResp::new("No challenge existing")),
            )
                .into_response();
        }
        Err(e) => {
            error!(
                "notification register for npub {}, fetching challenge failed: {e}",
                &payload.npub
            );
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResp::new("challenge error")),
            )
                .into_response();
        }
    };

    let now = Utc::now();
    // challenge expired
    if now > (challenge.created_at + Duration::seconds(CHALLENGE_EXPIRY_SECONDS)) {
        error!("notification register challenge expired");
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResp::new("challenge expired")),
        )
            .into_response();
    }

    match util::verify_signature(&challenge.challenge, &payload.signed_challenge, &x_only) {
        Ok(true) => {
            // remove consumed challenge from DB
            if let Err(e) = state
                .notification_store
                .remove_challenge_for_npub(&challenge.npub)
                .await
            {
                warn!("Failed to delete consumed challenge: {e}");
            }

            // send email confirmation mail
            let mut random_bytes = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut random_bytes);
            let email_confirmation_token = URL_SAFE.encode(random_bytes);
            let email_msg = build_email_confirmation_message(
                &state.cfg.host_url,
                &state.cfg.email_from_address,
                &payload.email,
                &email_confirmation_token,
            );

            if let Err(e) = state.email_service.send(email_msg).await {
                error!("notification register send confirmation mail error: {e}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResp::new("send mail confirmation error")),
                )
                    .into_response();
            }

            // persist email confirmation state
            if let Err(e) = state
                .notification_store
                .insert_confirmation_email_sent_for_npub(
                    &challenge.npub,
                    &payload.email,
                    &email_confirmation_token,
                )
                .await
            {
                error!("notification register persist email confirmation state: {e}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResp::new("mail confirmation error")),
                )
                    .into_response();
            }

            // persist email notification preferences with token to change them
            let mut random_bytes_pref_token = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut random_bytes_pref_token);
            let preferences_token = URL_SAFE.encode(random_bytes_pref_token);
            if let Err(e) = state
                .notification_store
                .insert_email_preferences_for_npub(
                    &challenge.npub,
                    &payload.email,
                    &preferences_token,
                    payload.ebill_url.as_str(),
                    PreferencesFlags::default(),
                )
                .await
            {
                error!("notification register persist email preferences state: {e}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResp::new("mail confirmation error")),
                )
                    .into_response();
            }
            (StatusCode::OK, Json(SuccessResp::new("OK"))).into_response()
        }
        Ok(false) => {
            error!("notification register check invalid challenge error");
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResp::new("invalid challenge")),
            )
                .into_response()
        }
        Err(e) => {
            error!("notification register check challenge error: {e}");
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResp::new("error checking challenge")),
            )
                .into_response()
        }
    }
}

/// We validate the email confirmation token and enable the email preferences, if everything is valid
pub async fn confirm_email(
    State(state): State<AppState>,
    qry: Query<EmailConfirmationToken>,
) -> impl IntoResponse {
    let token = qry.token.clone();
    if let Err(e) = URL_SAFE.decode(&token) {
        error!("notification email confirmation base64 error: {e}");
        return (StatusCode::BAD_REQUEST, "invalid token").into_response();
    }

    let email_confirmation = match state
        .notification_store
        .get_confirmation_email_state_for_token(&token)
        .await
    {
        Ok(Some(conf)) => conf,
        _ => {
            error!("notification email confirmation not found by token");
            return (StatusCode::BAD_REQUEST, "invalid token").into_response();
        }
    };

    let now = Utc::now();
    // token expired
    if now > (email_confirmation.sent_at + Duration::seconds(EMAIL_CONFIRMATION_EXPIRY_SECONDS)) {
        error!("notification confirm email token expired");
        return (StatusCode::BAD_REQUEST, "token expired").into_response();
    }

    // already confirmed
    if email_confirmation.confirmed {
        error!("notification confirm email already confirmed");
        return (StatusCode::BAD_REQUEST, "email already confirmed").into_response();
    }

    // preferences exist for npub
    let email_preferences = match state
        .notification_store
        .get_email_preferences_for_npub(&email_confirmation.npub)
        .await
    {
        Ok(Some(pref)) => pref,
        _ => {
            error!("notification email confirmation no preferences found for npub");
            return (StatusCode::BAD_REQUEST, "invalid token").into_response();
        }
    };

    // email doesn't match created preferences
    if email_confirmation.email != email_preferences.email {
        error!("notification email confirmation prefs don't match confirmation");
        return (StatusCode::BAD_REQUEST, "invalid email").into_response();
    }

    // set to confirmed
    if let Err(e) = state
        .notification_store
        .set_confirmation_email_confirmed_for_npub(&email_confirmation.npub)
        .await
    {
        error!("notification email confirmation, setting to confirmed failed: {e} ");
        return (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response();
    }

    (StatusCode::OK, "Success! Email Confirmed").into_response()
}
