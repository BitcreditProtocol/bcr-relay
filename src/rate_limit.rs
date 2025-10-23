use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::{StatusCode, request::Parts},
};
use chrono::{DateTime, Duration, Utc};
use std::{
    collections::{HashMap, VecDeque},
    net::{IpAddr, SocketAddr},
};

/// How often do we allow the same ip in the time frame
const IP_LIMIT: usize = 100;
const IP_WINDOW: Duration = Duration::seconds(10 * 60); // 10 minutes

/// How often do we allow the same email to be registered in the time frame
const EMAIL_LIMIT: usize = 30;
const EMAIL_WINDOW: Duration = Duration::seconds(24 * 3600); //  1 day

/// How often do we allow the same npub in the time frame
const NPUB_LIMIT: usize = 100;
const NPUB_WINDOW: Duration = Duration::seconds(10 * 60); // 10 minutes

const MAX_IDLE: Duration = Duration::seconds(24 * 3600); // remove after 24h idle
pub const PRUNE_INTERVAL: Duration = Duration::seconds(10 * 60); // check every 10 minutes

#[derive(Debug)]
pub struct SlidingWindow {
    hits: VecDeque<DateTime<Utc>>,
    window: Duration,
    limit: usize,
    last_seen: DateTime<Utc>,
}

impl SlidingWindow {
    pub fn new(limit: usize, window: Duration) -> Self {
        Self {
            hits: VecDeque::with_capacity(limit),
            window,
            limit,
            last_seen: Utc::now(),
        }
    }

    pub fn allow(&mut self, now: DateTime<Utc>) -> bool {
        // Remove expired hits
        while let Some(&ts) = self.hits.front() {
            if now - ts > self.window {
                self.hits.pop_front();
            } else {
                break;
            }
        }
        self.last_seen = now;

        if self.hits.len() < self.limit {
            self.hits.push_back(now);
            true
        } else {
            false
        }
    }

    pub fn retain(&self, now: DateTime<Utc>) -> bool {
        now - self.last_seen <= MAX_IDLE
    }
}

#[derive(Debug)]
pub struct RateLimiter {
    by_ip: HashMap<String, SlidingWindow>,
    by_email: HashMap<String, SlidingWindow>,
    by_npub_sender: HashMap<String, SlidingWindow>,
    by_npub_receiver: HashMap<String, SlidingWindow>,
    last_prune: DateTime<Utc>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            by_ip: HashMap::new(),
            by_email: HashMap::new(),
            by_npub_sender: HashMap::new(),
            by_npub_receiver: HashMap::new(),
            last_prune: Utc::now(),
        }
    }

    /// Check if the request is allowed
    /// There is always an IP, but not always an email, or npub - everything that's set has to be allowed
    /// The values are expected to be validated before getting in here
    pub fn check(
        &mut self,
        ip: &str,
        email: Option<&str>,
        npub_sender: Option<&str>,
        npub_receiver: Option<&str>,
    ) -> bool {
        let now = Utc::now();
        self.prune_if_needed(now);

        let ip_ok = self
            .by_ip
            .entry(ip.to_string())
            .or_insert_with(|| SlidingWindow::new(IP_LIMIT, IP_WINDOW))
            .allow(now);

        let email_ok = if let Some(email) = email {
            let key = email.to_lowercase();
            self.by_email
                .entry(key)
                .or_insert_with(|| SlidingWindow::new(EMAIL_LIMIT, EMAIL_WINDOW))
                .allow(now)
        } else {
            true // no email provided -> skip check
        };

        let npub_sender_ok = if let Some(npub) = npub_sender {
            self.by_npub_sender
                .entry(npub.to_string())
                .or_insert_with(|| SlidingWindow::new(NPUB_LIMIT, NPUB_WINDOW))
                .allow(now)
        } else {
            true // no sender npub provided -> skip check
        };

        let npub_receiver_ok = if let Some(npub) = npub_receiver {
            self.by_npub_receiver
                .entry(npub.to_string())
                .or_insert_with(|| SlidingWindow::new(NPUB_LIMIT, NPUB_WINDOW))
                .allow(now)
        } else {
            true // no received npub provided -> skip check
        };

        ip_ok && email_ok && npub_sender_ok && npub_receiver_ok
    }

    /// Every PRUNE_INTERVAL, remove outdated entries
    fn prune_if_needed(&mut self, now: DateTime<Utc>) {
        if now - self.last_prune < PRUNE_INTERVAL {
            return;
        }

        self.last_prune = now;

        // only keep recent entries
        self.by_ip.retain(|_, win| win.retain(now));
        self.by_email.retain(|_, win| win.retain(now));
        self.by_npub_sender.retain(|_, win| win.retain(now));
    }
}

pub struct RealIp(pub IpAddr);

impl<S> FromRequestParts<S> for RealIp
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Check X-FORWARDED-FOR header and take the first value for gcp as per
        // Cloud Run: https://cloud.google.com/functions/docs/reference/headers#x-forwarded-for
        // GCP LB: https://cloud.google.com/load-balancing/docs/https
        if let Some(forwarded) = parts.headers.get("x-forwarded-for")
            && let Ok(s) = forwarded.to_str()
            && let Some(ip_str) = s.split(',').next()
            && let Ok(ip) = ip_str.trim().parse()
        {
            return Ok(RealIp(ip));
        }

        // Fallback to socket addr for local dev
        if let Some(addr) = parts.extensions.get::<ConnectInfo<SocketAddr>>() {
            return Ok(RealIp(addr.ip()));
        }

        Err((StatusCode::BAD_REQUEST, "No request IP"))
    }
}
