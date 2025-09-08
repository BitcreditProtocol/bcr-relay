use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use axum_extra::extract::Form;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use borsh_derive::BorshSerialize;
use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tinytemplate::TinyTemplate;
use tracing::{error, warn};

use crate::{
    AppState,
    notification::{
        email::{build_email_confirmation_message, build_email_notification_message},
        preferences::{PreferencesContextContentFlag, PreferencesFlags},
    },
    rate_limit::RealIp,
    util::{self, get_logo_link},
};

pub mod email;
pub mod notification_store;
mod preferences;
mod template;

/// Maximum age of a challenge - we expect requests to be made immediately after each other
const CHALLENGE_EXPIRY_SECONDS: i64 = 120; // 2 minutes

/// Maximum age of an email confirmation
const EMAIL_CONFIRMATION_EXPIRY_SECONDS: i64 = 60 * 60 * 24; // 1 day

const BITCR_PREFIX: &str = "bitcr";

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
    pub token: String,
    pub email: String,
    pub email_confirmed: bool,
    pub ebill_url: url::Url,
    pub flags: PreferencesFlags,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationStartReq {
    pub npub: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotificationStartResp {
    pub challenge: String,
    pub ttl_seconds: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorResp {
    pub msg: String,
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

#[derive(Deserialize)]
pub struct EmailConfirmationToken {
    pub token: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmailRegisterResp {
    pub preferences_token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationRegisterReq {
    pub npub: String,
    pub signed_challenge: String,
    pub ebill_url: url::Url,
    pub email: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationSendReq {
    /// The payload for the notification
    pub payload: NotificationSendPayload,
    /// The payload signed by the sender
    pub signature: String,
}

#[derive(Debug, Clone, Deserialize, BorshSerialize)]
pub struct NotificationSendPayload {
    /// The type of event, e.g. BillSigned
    pub kind: String,
    /// The domain ID, e.g. a bill id
    pub id: String,
    /// The receiver npub
    pub receiver: String,
    /// The sender npub
    pub sender: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChangePreferencesReq {
    pub preferences_token: String,
    pub enabled: Option<String>,
    pub flags: Option<Vec<i64>>,
}

/// Send back a random challenge to the caller, which we expect to be signed with their npub to validate
/// the request actually comes from the given npub
pub async fn start(
    RealIp(ip): RealIp,
    State(state): State<AppState>,
    Json(payload): Json<NotificationStartReq>,
) -> impl IntoResponse {
    if let Err(e) = util::validate_npub(&payload.npub) {
        error!("notification start with invalid npub: {e}");
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResp::new("Invalid npub")),
        )
            .into_response();
    }

    let mut rate_limiter = state.rate_limiter.lock().await;
    let allowed = rate_limiter.check(&ip.to_string(), None, None, Some(&payload.npub));
    drop(rate_limiter);
    if !allowed {
        warn!(
            "Rate limited req from {} with npub {}",
            &ip.to_string(),
            &payload.npub
        );
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ErrorResp::new("Please try again later")),
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

    (
        StatusCode::OK,
        Json(NotificationStartResp {
            challenge,
            ttl_seconds: CHALLENGE_EXPIRY_SECONDS,
        }),
    )
        .into_response()
}

/// We validate npub, email and signed challenge. If everything is OK, we send a confirmation email
/// and we create a stub for email preferences with a token to change them later
pub async fn register(
    RealIp(ip): RealIp,
    State(state): State<AppState>,
    Json(payload): Json<NotificationRegisterReq>,
) -> impl IntoResponse {
    let x_only = match util::validate_npub(&payload.npub) {
        Ok(n) => n,
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

    let mut rate_limiter = state.rate_limiter.lock().await;
    let allowed = rate_limiter.check(
        &ip.to_string(),
        Some(&payload.email),
        None,
        Some(&payload.npub),
    );
    drop(rate_limiter);
    if !allowed {
        warn!(
            "Rate limited req from {} with npub {} and email {}",
            &ip.to_string(),
            &payload.npub,
            &payload.email,
        );
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ErrorResp::new("Please try again later")),
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
            let email_confirmation_token = URL_SAFE_NO_PAD.encode(random_bytes);
            let email_msg = match build_email_confirmation_message(
                &state.cfg.host_url,
                &state.cfg.email_from_address,
                &payload.email,
                &email_confirmation_token,
            ) {
                Ok(msg) => msg,
                Err(e) => {
                    error!("notification register create confirmation mail error: {e}");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResp::new("send mail confirmation error")),
                    )
                        .into_response();
                }
            };

            if let Err(e) = state.email_service.send(email_msg).await {
                error!("notification register send confirmation mail error: {e}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResp::new("send mail confirmation error")),
                )
                    .into_response();
            }

            let mut random_bytes_pref_token = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut random_bytes_pref_token);
            let preferences_token = URL_SAFE_NO_PAD.encode(random_bytes_pref_token);

            // persist email confirmation state email notification preferences with token to change them
            if let Err(e) = state
                .notification_store
                .insert_confirmation_email_sent_and_preferences_for_npub(
                    &challenge.npub,
                    &payload.email,
                    &email_confirmation_token,
                    &preferences_token,
                    payload.ebill_url.as_str(),
                    PreferencesFlags::default(),
                )
                .await
            {
                error!(
                    "notification register persist email confirmation and preferences state: {e}"
                );
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResp::new("mail confirmation error")),
                )
                    .into_response();
            }

            // return preferences token, so we can open email preferences from the app
            (
                StatusCode::OK,
                Json(EmailRegisterResp { preferences_token }),
            )
                .into_response()
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

pub async fn send(
    RealIp(ip): RealIp,
    State(state): State<AppState>,
    Json(req): Json<NotificationSendReq>,
) -> impl IntoResponse {
    let payload = req.payload;
    let signature = req.signature;
    if let Err(e) = util::validate_npub(&payload.receiver) {
        error!("notification send with invalid receiver npub: {e}");
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResp::new("Invalid receiver npub")),
        )
            .into_response();
    }

    let x_only_sender = match util::validate_npub(&payload.sender) {
        Ok(n) => n,
        Err(e) => {
            error!("notification send with invalid sender npub: {e}");
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResp::new("Invalid sender npub")),
            )
                .into_response();
        }
    };

    let mut rate_limiter = state.rate_limiter.lock().await;
    let allowed = rate_limiter.check(
        &ip.to_string(),
        None,
        Some(&payload.sender),
        Some(&payload.receiver),
    );
    drop(rate_limiter);
    if !allowed {
        warn!(
            "Rate limited req from {} with npub {}",
            &ip.to_string(),
            &payload.receiver
        );
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ErrorResp::new("Please try again later")),
        )
            .into_response();
    }

    let notification_type = match PreferencesFlags::from_name(&payload.kind) {
        Some(nt) => nt,
        None => {
            error!(
                "notification send with invalid event type: {}",
                &payload.kind
            );
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResp::new("Invalid kind")),
            )
                .into_response();
        }
    };

    if payload.id.is_empty() || !payload.id.starts_with(BITCR_PREFIX) {
        error!("notification send with empty, or invalid id",);
        return (StatusCode::BAD_REQUEST, Json(ErrorResp::new("Invalid ID"))).into_response();
    }

    // make sure sender signed the request
    match util::verify_request(&payload, &signature, &x_only_sender) {
        Ok(true) => {
            let email_preferences = match state
                .notification_store
                .get_email_preferences_for_npub(&payload.receiver)
                .await
            {
                Ok(Some(pref)) => pref,
                Ok(None) => {
                    // no mapping - ignore message
                    return (StatusCode::OK, Json(SuccessResp::new("OK"))).into_response();
                }
                Err(e) => {
                    error!("notification send error fetching email preferences: {e}");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResp::new("Error sending email")),
                    )
                        .into_response();
                }
            };

            if !email_preferences.enabled {
                // receiver does not want notifications - ignore message
                return (StatusCode::OK, Json(SuccessResp::new("OK"))).into_response();
            }

            if !email_preferences.flags.contains(notification_type) {
                // receiver does not want this notification type - ignore message
                return (StatusCode::OK, Json(SuccessResp::new("OK"))).into_response();
            }

            let email_msg = match build_email_notification_message(
                &state.cfg.host_url,
                &email_preferences.token,
                &state.cfg.email_from_address,
                &email_preferences.email,
                &notification_type.to_title(),
                &notification_type.to_link(&email_preferences.ebill_url, &payload.id),
            ) {
                Ok(msg) => msg,
                Err(e) => {
                    error!("notification register create confirmation mail error: {e}");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResp::new("send mail confirmation error")),
                    )
                        .into_response();
                }
            };

            if let Err(e) = state.email_service.send(email_msg).await {
                error!("notification send mail error: {e}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResp::new("Error sending mail")),
                )
                    .into_response();
            }

            (StatusCode::OK, Json(SuccessResp::new("OK"))).into_response()
        }
        Ok(false) => {
            error!("notification send check invalid signature error");
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResp::new("invalid signature")),
            )
                .into_response()
        }
        Err(e) => {
            error!("notification send check signature error: {e}");
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResp::new("error checking signature")),
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
    if let Err(e) = URL_SAFE_NO_PAD.decode(&token) {
        error!("notification email confirmation base64 error: {e}");
        return build_html_error(
            StatusCode::BAD_REQUEST,
            "invalid token",
            &state.cfg.host_url,
        )
        .into_response();
    }

    let email_confirmation = match state
        .notification_store
        .get_confirmation_email_state_for_token(&token)
        .await
    {
        Ok(Some(conf)) => conf,
        _ => {
            error!("notification email confirmation not found by token");
            return build_html_error(
                StatusCode::BAD_REQUEST,
                "invalid token",
                &state.cfg.host_url,
            )
            .into_response();
        }
    };

    let now = Utc::now();
    // token expired
    if now > (email_confirmation.sent_at + Duration::seconds(EMAIL_CONFIRMATION_EXPIRY_SECONDS)) {
        error!("notification confirm email token expired");
        return build_html_error(
            StatusCode::BAD_REQUEST,
            "token expired",
            &state.cfg.host_url,
        )
        .into_response();
    }

    // already confirmed
    if email_confirmation.confirmed {
        error!("notification confirm email already confirmed");
        return build_html_error(
            StatusCode::BAD_REQUEST,
            "email already confirmed",
            &state.cfg.host_url,
        )
        .into_response();
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
            return build_html_error(
                StatusCode::BAD_REQUEST,
                "invalid token",
                &state.cfg.host_url,
            )
            .into_response();
        }
    };

    // email doesn't match created preferences
    if email_confirmation.email != email_preferences.email {
        error!("notification email confirmation prefs don't match confirmation");
        return build_html_error(
            StatusCode::BAD_REQUEST,
            "invalid email",
            &state.cfg.host_url,
        )
        .into_response();
    }

    // set to confirmed
    if let Err(e) = state
        .notification_store
        .set_confirmation_email_confirmed_for_npub(&email_confirmation.npub)
        .await
    {
        error!("notification email confirmation, setting to confirmed failed: {e} ");
        return build_html_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal server error",
            &state.cfg.host_url,
        )
        .into_response();
    }

    build_html_success("Success! Email Confirmed", &state.cfg.host_url).into_response()
}

pub async fn preferences(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> impl IntoResponse {
    if let Err(e) = URL_SAFE_NO_PAD.decode(&token) {
        error!("notification preferences called with invalid token: {e}");
        return build_html_error(
            StatusCode::BAD_REQUEST,
            "invalid token",
            &state.cfg.host_url,
        )
        .into_response();
    }

    // check email preferences exist
    let email_preferences = match state
        .notification_store
        .get_email_preferences_for_token(&token)
        .await
    {
        Ok(Some(p)) => p,
        _ => {
            error!("notification update preferences invalid token");
            return build_html_error(
                StatusCode::BAD_REQUEST,
                "invalid token",
                &state.cfg.host_url,
            )
            .into_response();
        }
    };

    // make sure email was confirmed
    if !email_preferences.email_confirmed {
        error!("notification preferences email was not confirmed");
        return build_html_error(
            StatusCode::BAD_REQUEST,
            "email has to be confirmed",
            &state.cfg.host_url,
        )
        .into_response();
    }

    build_template(
        template::PREFERENCES_TEMPLATE,
        PreferencesContext {
            content: PreferencesContextContent {
                enabled: email_preferences.enabled,
                preferences_token: token,
                anon_email: util::anonymize_email(&email_preferences.email),
                anon_npub: util::anonymize_npub(&email_preferences.npub),
                flags: email_preferences.flags.as_context_vec(),
            },
            title: "Email Preferences".to_owned(),
            logo_link: get_logo_link(&state.cfg.host_url),
        },
        StatusCode::OK,
    )
    .into_response()
}

pub async fn update_preferences(
    State(state): State<AppState>,
    Form(payload): Form<ChangePreferencesReq>,
) -> impl IntoResponse {
    let token = payload.preferences_token;
    if let Err(e) = URL_SAFE_NO_PAD.decode(&token) {
        error!("notification preferences called with invalid token: {e}");
        return build_html_error(
            StatusCode::BAD_REQUEST,
            "invalid token",
            &state.cfg.host_url,
        )
        .into_response();
    }

    // check email preferences exist
    let email_preferences = match state
        .notification_store
        .get_email_preferences_for_token(&token)
        .await
    {
        Ok(Some(p)) => p,
        _ => {
            error!("notification update preferences invalid token");
            return build_html_error(
                StatusCode::BAD_REQUEST,
                "invalid token",
                &state.cfg.host_url,
            )
            .into_response();
        }
    };

    // make sure email was confirmed
    if !email_preferences.email_confirmed {
        error!("notification preferences email was not confirmed");
        return build_html_error(
            StatusCode::BAD_REQUEST,
            "email has to be confirmed",
            &state.cfg.host_url,
        )
        .into_response();
    }

    let enabled = match payload.enabled {
        Some(e) => e.as_str() == "on",
        None => false,
    };

    let mut updated_flags = PreferencesFlags::empty();
    // set all selected flags
    if let Some(flags) = payload.flags {
        for flag in flags {
            if let Some(parsed) = PreferencesFlags::from_bits(flag) {
                updated_flags |= parsed;
            }
        }
    }

    if let Err(e) = state
        .notification_store
        .update_email_preferences_for_token(&token, enabled, updated_flags)
        .await
    {
        error!("notification update preferences error: {e}");
        return build_html_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "could not save changes",
            &state.cfg.host_url,
        )
        .into_response();
    }

    Redirect::to(&format!("/notifications/preferences/{}", token)).into_response()
}

#[derive(Debug, Serialize)]
struct PreferencesContext {
    pub content: PreferencesContextContent,
    pub title: String,
    pub logo_link: String,
}

#[derive(Debug, Serialize)]
struct PreferencesContextContent {
    pub enabled: bool,
    pub preferences_token: String,
    pub anon_email: String,
    pub anon_npub: String,
    pub flags: Vec<PreferencesContextContentFlag>,
}

#[derive(Debug, Serialize)]
struct ErrorSuccessContext {
    pub content: ErrorSuccessContextContent,
    pub title: String,
    pub logo_link: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorSuccessContextContent {
    pub msg: String,
}

fn build_html_success(msg: &str, host_url: &url::Url) -> impl IntoResponse {
    build_template(
        template::ERROR_SUCCESS_TEMPLATE,
        ErrorSuccessContext {
            content: ErrorSuccessContextContent {
                msg: msg.to_owned(),
            },
            title: "Success".to_owned(),
            logo_link: get_logo_link(host_url),
        },
        StatusCode::OK,
    )
}

fn build_html_error(status: StatusCode, msg: &str, host_url: &url::Url) -> impl IntoResponse {
    build_template(
        template::ERROR_SUCCESS_TEMPLATE,
        ErrorSuccessContext {
            content: ErrorSuccessContextContent {
                msg: msg.to_owned(),
            },
            title: "Error".to_owned(),
            logo_link: get_logo_link(host_url),
        },
        status,
    )
}

fn build_template<C>(content_tmpl: &str, ctx: C, status: StatusCode) -> impl IntoResponse
where
    C: Serialize,
{
    let mut tt = TinyTemplate::new();
    if let Err(e) = tt.add_template("base", template::TEMPLATE) {
        error!("error building base template: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response();
    }
    if let Err(e) = tt.add_template("content", content_tmpl) {
        error!("error building content template: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response();
    }

    let rendered = match tt.render("base", &ctx) {
        Ok(r) => r,
        Err(e) => {
            error!("error building template: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response();
        }
    };
    (status, Html(rendered)).into_response()
}
