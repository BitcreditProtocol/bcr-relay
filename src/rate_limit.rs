use chrono::{DateTime, Duration, Utc};
use std::collections::VecDeque;

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
