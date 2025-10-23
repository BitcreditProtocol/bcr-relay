use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    sync::Arc,
};

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use clap::Parser;
use deadpool_postgres::Pool;
use nostr::{
    event::{Event, TagKind, TagStandard},
    filter::{Alphabet, SingleLetterTag},
    nips::nip73::ExternalContentId,
    types::Url,
    util::BoxedFuture,
};
use nostr_postgres_db::*;
use nostr_relay_builder::{
    LocalRelay, RelayBuilder,
    builder::{PolicyResult, RelayBuilderNip42, RelayBuilderNip42Mode, WritePolicy},
};
use tokio::sync::Mutex;

use crate::rate_limit::{PRUNE_INTERVAL, SlidingWindow};

const BCR_NOSTR_CHAIN_PREFIX: &str = "bitcredit";

pub async fn init(config: &RelayConfig, pool: Pool) -> Result<LocalRelay> {
    Ok(LocalRelay::new(builder(config, pool).await?).await?)
}

async fn builder(config: &RelayConfig, pool: Pool) -> Result<RelayBuilder> {
    let dba = database(pool).await?;
    Ok(RelayBuilder::default()
        .nip42(auth_mode())
        .database(dba)
        .write_policy(block_rate_limiter(config)))
}

fn auth_mode() -> RelayBuilderNip42 {
    RelayBuilderNip42 {
        // read and write requires client auth
        mode: RelayBuilderNip42Mode::Both,
    }
}

fn block_rate_limiter(config: &RelayConfig) -> BlockRateLimiter {
    let limiter = Arc::new(Mutex::new(NostrRateLimiter::new(
        config.chain_rate_limit,
        Duration::seconds(config.chain_rate_limit_window as i64),
    )));
    BlockRateLimiter::new(
        limiter.clone(),
        HashSet::from_iter([
            "bill".to_owned(),
            "identity".to_owned(),
            "company".to_owned(),
        ]),
    )
}

async fn database(pool: Pool) -> Result<NostrPostgres> {
    Ok(NostrPostgres::from_pool(pool).await?)
}

#[derive(Debug, Clone, Parser)]
pub struct RelayConfig {
    #[arg(default_value_t = String::from("localhost:8080"), long, env = "LISTEN_ADDRESS")]
    pub listen_address: String,
    #[arg(default_value_t = Url::parse("http://localhost:8080").unwrap(), long, env = "HOST_URL")]
    pub host_url: Url,

    #[arg(default_value_t = String::from("postgres"), long, env = "DB_USER")]
    pub db_user: String,
    #[arg(default_value_t = String::from("password"), long, env = "DB_PASSWORD")]
    pub db_password: String,
    #[arg(default_value_t = String::from(""), long, env = "DB_NAME")]
    pub db_name: String,
    #[arg(default_value_t = String::from("localhost"), long, env = "DB_HOST")]
    pub db_host: String,
    #[arg(default_value_t = String::from(""), long, env = "EMAIL_FROM_ADDRESS")]
    pub email_from_address: String,
    #[arg(default_value_t = String::from(""), long, env = "EMAIL_API_KEY")]
    pub email_api_key: String,
    #[arg(default_value_t = String::from(""), long, env = "EMAIL_API_SECRET_KEY")]
    pub email_api_secret_key: String,
    #[arg(default_value_t = Url::parse("https://api.mailjet.com").unwrap(), long, env = "EMAIL_URL")]
    pub email_url: Url,
    #[arg(default_value_t = 6, long, env = "BLOCKCHAIN_RATE_LIMIT")]
    pub chain_rate_limit: usize,
    #[arg(
        default_value_t = 60,
        long,
        env = "BLOCKCHAIN_RATE_LIMIT_WINDOW_SECONDS"
    )]
    pub chain_rate_limit_window: usize,
}

impl RelayConfig {
    pub fn db_connection_string(&self) -> String {
        let db_name = if self.db_name.is_empty() {
            "".to_string()
        } else {
            format!("/{}", self.db_name)
        };
        format!(
            "postgres://{}:{}@{}?host={}",
            self.db_user, self.db_password, db_name, self.db_host
        )
    }
}

#[derive(Clone, Debug)]
pub struct BlockRateLimiter {
    limiter: Arc<Mutex<dyn NostrRateLimiterApi>>,
    chains: HashSet<String>,
}

impl BlockRateLimiter {
    pub fn new(limiter: Arc<Mutex<dyn NostrRateLimiterApi>>, chains: HashSet<String>) -> Self {
        Self { chains, limiter }
    }
}

impl WritePolicy for BlockRateLimiter {
    fn admit_event<'a>(
        &'a self,
        event: &'a Event,
        addr: &'a std::net::SocketAddr,
    ) -> BoxedFuture<'a, nostr_relay_builder::builder::PolicyResult> {
        Box::pin(async move {
            if let Some(chain_key) = bcr_chain_key(event, &self.chains)
                && !self
                    .limiter
                    .lock()
                    .await
                    .allowed(format!("{}:{chain_key}", addr).as_str(), Utc::now())
            {
                PolicyResult::Reject(format!(
                    "Rate limit exceeded for BCR chain event {chain_key}"
                ))
            } else {
                PolicyResult::Accept
            }
        })
    }
}

pub trait NostrRateLimiterApi: Send + Sync + Debug {
    fn allowed(&mut self, key: &str, now: DateTime<Utc>) -> bool;
}

#[derive(Debug)]
struct NostrRateLimiter {
    keys: HashMap<String, SlidingWindow>,
    window: Duration,
    last_prune: DateTime<Utc>,
    limit: usize,
}

impl NostrRateLimiter {
    pub fn new(limit: usize, window: Duration) -> Self {
        Self {
            keys: HashMap::new(),
            window,
            last_prune: Utc::now(),
            limit,
        }
    }

    pub fn check(&mut self, key: &str, now: DateTime<Utc>) -> bool {
        self.prune(now);
        self.keys
            .entry(key.to_string())
            .or_insert_with(|| SlidingWindow::new(self.limit, self.window))
            .allow(now)
    }

    pub fn prune(&mut self, now: DateTime<Utc>) {
        if now - self.last_prune < PRUNE_INTERVAL {
            return;
        }
        self.last_prune = now;

        // only keep recent entries
        self.keys.retain(|_, win| win.should_prune(now));
    }
}

impl NostrRateLimiterApi for NostrRateLimiter {
    fn allowed(&mut self, key: &str, now: DateTime<Utc>) -> bool {
        self.check(key, now)
    }
}

/// Check if the event is a BCR chain event of one of the specified chains and if so, return the
/// rate limit key for the event.
fn bcr_chain_key(event: &Event, chains: &HashSet<String>) -> Option<String> {
    event
        .tags
        .filter_standardized(TagKind::SingleLetter(SingleLetterTag::lowercase(
            Alphabet::I,
        )))
        .find_map(|tag| match tag {
            TagStandard::ExternalContent {
                content:
                    ExternalContentId::BlockchainAddress {
                        chain,
                        chain_id,
                        address,
                        ..
                    },
                ..
            } if chain_id.is_some()
                && chain == BCR_NOSTR_CHAIN_PREFIX
                && chains.contains(chain_id.as_ref().unwrap()) =>
            {
                chain_id.as_ref().map(|id| format!("{id}:{address}"))
            }
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use super::*;
    use chrono::TimeZone;
    use nostr::{
        event::{EventBuilder, Tag},
        key::Keys,
    };

    #[test]
    fn test_rate_limiter_allows_within_limit() {
        let mut limiter = NostrRateLimiter::new(3, Duration::seconds(60));
        let now = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
        let key = "test-key";

        assert!(limiter.allowed(key, now));
        assert!(limiter.allowed(key, now));
        assert!(limiter.allowed(key, now));
    }

    #[test]
    fn test_rate_limiter_blocks_over_limit() {
        let mut limiter = NostrRateLimiter::new(2, Duration::seconds(60));
        let now = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
        let key = "test-key";

        assert!(limiter.allowed(key, now));
        assert!(limiter.allowed(key, now));
        assert!(!limiter.allowed(key, now));
    }

    #[test]
    fn test_rate_limiter_resets_after_window() {
        let mut limiter = NostrRateLimiter::new(2, Duration::seconds(10));
        let now = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
        let key = "test-key";

        assert!(limiter.allowed(key, now));
        assert!(limiter.allowed(key, now));
        assert!(!limiter.allowed(key, now));

        // Move time forward past window
        let later = now + Duration::seconds(11);
        assert!(limiter.allowed(key, later));
        assert!(limiter.allowed(key, later));
        assert!(!limiter.allowed(key, later));
    }

    pub fn tag_content(id: &str, blockchain: &str) -> ExternalContentId {
        ExternalContentId::BlockchainAddress {
            chain: BCR_NOSTR_CHAIN_PREFIX.to_string(),
            address: id.to_string(),
            chain_id: Some(blockchain.to_string()),
        }
    }

    pub fn bcr_nostr_tag(id: &str, blockchain: &str) -> Tag {
        TagStandard::ExternalContent {
            content: tag_content(id, blockchain),
            hint: None,
            uppercase: false,
        }
        .into()
    }
    // Create a test BCR chain event
    fn create_bcr_chain_event(chain_id: &str, address: &str) -> Event {
        let keys = Keys::generate();
        EventBuilder::text_note("This is a test BCR chain event")
            .tag(bcr_nostr_tag(address, chain_id))
            .sign_with_keys(&keys)
            .unwrap()
    }

    // Create a test non-BCR event
    fn create_non_bcr_event() -> Event {
        let keys = Keys::generate();
        EventBuilder::text_note("This is a regular event")
            .sign_with_keys(&keys)
            .unwrap()
    }

    #[tokio::test]
    async fn test_bcr_chain_key_extraction() {
        // Test chain ID and address extraction
        let chains = HashSet::from_iter([
            "bill".to_string(),
            "identity".to_string(),
            "company".to_string(),
        ]);

        // Test with valid BCR chain event
        let event = create_bcr_chain_event("bill", "addr123");
        let key = bcr_chain_key(&event, &chains);
        assert_eq!(key, Some("bill:addr123".to_string()));

        // Test with unsupported chain ID
        let event = create_bcr_chain_event("unsupported", "addr123");
        let key = bcr_chain_key(&event, &chains);
        assert_eq!(key, None);

        // Test with non-BCR event
        let event = create_non_bcr_event();
        let key = bcr_chain_key(&event, &chains);
        assert_eq!(key, None);
    }

    #[tokio::test]
    async fn test_block_rate_limiter_admit_event() {
        // Create a rate limiter with 2 requests per 10 seconds
        let limiter = Arc::new(Mutex::new(NostrRateLimiter::new(2, Duration::seconds(10))));

        let chains = HashSet::from_iter(["bill".to_string(), "identity".to_string()]);
        let block_limiter = BlockRateLimiter::new(limiter, chains);

        // Create test socket address
        let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

        // Test with BCR chain event
        let event1 = create_bcr_chain_event("bill", "addr123");
        let event2 = create_bcr_chain_event("bill", "addr123");
        let event3 = create_bcr_chain_event("bill", "addr123");

        // First two should be accepted
        let result1 = block_limiter.admit_event(&event1, &socket).await;
        assert!(matches!(result1, PolicyResult::Accept));

        let result2 = block_limiter.admit_event(&event2, &socket).await;
        assert!(matches!(result2, PolicyResult::Accept));

        // Third should be rejected due to rate limit
        let result3 = block_limiter.admit_event(&event3, &socket).await;
        assert!(matches!(result3, PolicyResult::Reject(_)));

        // Test with different address should be accepted (different rate limit key)
        let event_diff_addr = create_bcr_chain_event("bill", "addr456");
        let result_diff_addr = block_limiter.admit_event(&event_diff_addr, &socket).await;
        assert!(matches!(result_diff_addr, PolicyResult::Accept));

        // Test with non-BCR event should always be accepted
        let non_bcr_event = create_non_bcr_event();
        let non_bcr_result = block_limiter.admit_event(&non_bcr_event, &socket).await;
        assert!(matches!(non_bcr_result, PolicyResult::Accept));
    }

    #[tokio::test]
    async fn test_block_rate_limiter_different_ip_addresses() {
        // Create a rate limiter with 2 requests per minute
        let limiter = Arc::new(Mutex::new(NostrRateLimiter::new(2, Duration::seconds(60))));

        let chains = HashSet::from_iter(["bill".to_string()]);
        let block_limiter = BlockRateLimiter::new(limiter, chains);

        // Create different socket addresses
        let socket1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let socket2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)), 8080);

        // Create BCR chain event
        let event = create_bcr_chain_event("bill", "addr123");

        // First two events from socket1 should be accepted
        assert!(matches!(
            block_limiter.admit_event(&event, &socket1).await,
            PolicyResult::Accept
        ));
        assert!(matches!(
            block_limiter.admit_event(&event, &socket1).await,
            PolicyResult::Accept
        ));

        // Third should be rejected due to rate limit
        assert!(matches!(
            block_limiter.admit_event(&event, &socket1).await,
            PolicyResult::Reject(_)
        ));

        // But same event from socket2 should be accepted (different IP)
        assert!(matches!(
            block_limiter.admit_event(&event, &socket2).await,
            PolicyResult::Accept
        ));
    }
}
