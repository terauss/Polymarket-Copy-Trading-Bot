/// Market cache management with automatic refresh
/// Handles caching of market data, tokens, and live status
use rustc_hash::FxHashMap;
use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

// ============================================================================
// Cache Configuration
// ============================================================================

/// How often to refresh caches (in seconds)
pub const CACHE_REFRESH_INTERVAL_SECS: u64 = 30 * 60; // 30 minutes

/// Cache file paths
const NEG_RISK_CACHE_PATH: &str = ".clob_market_cache.json";
const SLUG_CACHE_PATH: &str = ".clob_slug_cache.json";
const ATP_TOKENS_CACHE_PATH: &str = ".atp_token_categories.json";
const LIGUE1_TOKENS_CACHE_PATH: &str = ".ligue1_tokens.json";
const LIVE_CACHE_PATH: &str = ".live_cache.json";

/// Price buffer adjustments
const ATP_BUFFER: f64 = 0.01;
const LIGUE1_BUFFER: f64 = 0.01;

// ============================================================================
// Cache Data Structures
// ============================================================================

/// Holds all cached market data
pub struct MarketCaches {
    /// Token ID -> neg_risk boolean
    pub neg_risk: RwLock<FxHashMap<String, bool>>,
    /// Token ID -> market slug
    pub slugs: RwLock<FxHashMap<String, String>>,
    /// ATP token IDs
    pub atp_tokens: RwLock<FxHashMap<String, String>>,
    /// Ligue 1 token IDs
    pub ligue1_tokens: RwLock<FxHashMap<String, ()>>,
    /// Token ID -> live status
    pub live_status: RwLock<FxHashMap<String, bool>>,
    /// Last refresh timestamp (Unix seconds)
    pub last_refresh: AtomicU64,
    /// Cache statistics
    pub stats: CacheStats,
}

#[derive(Default)]
pub struct CacheStats {
    pub neg_risk_count: AtomicU64,
    pub slug_count: AtomicU64,
    pub atp_count: AtomicU64,
    pub ligue1_count: AtomicU64,
    pub live_count: AtomicU64,
    pub refresh_count: AtomicU64,
    pub last_refresh_duration_ms: AtomicU64,
}

// ============================================================================
// MarketCaches impl
// ============================================================================

impl MarketCaches {
    pub fn new() -> Self {
        Self {
            neg_risk: RwLock::new(FxHashMap::default()),
            slugs: RwLock::new(FxHashMap::default()),
            atp_tokens: RwLock::new(FxHashMap::default()),
            ligue1_tokens: RwLock::new(FxHashMap::default()),
            live_status: RwLock::new(FxHashMap::default()),
            last_refresh: AtomicU64::new(0),
            stats: CacheStats::default(),
        }
    }

    /// Load all caches from disk
    pub fn load_all(&self) -> CacheLoadResult {
        let start = Instant::now();
        let mut result = CacheLoadResult::default();

        // neg_risk
        if let Ok(data) = std::fs::read_to_string(NEG_RISK_CACHE_PATH) {
            if let Ok(map) = serde_json::from_str::<HashMap<String, bool>>(&data) {
                let count = map.len();
                if let Ok(mut cache) = self.neg_risk.write() {
                    cache.clear();
                    cache.extend(map);
                    result.neg_risk_loaded = count;
                    self.stats
                        .neg_risk_count
                        .store(count as u64, Ordering::Relaxed);
                }
            }
        }

        // slugs
        if let Ok(data) = std::fs::read_to_string(SLUG_CACHE_PATH) {
            if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&data) {
                let count = map.len();
                if let Ok(mut cache) = self.slugs.write() {
                    cache.clear();
                    cache.extend(map);
                    result.slugs_loaded = count;
                    self.stats.slug_count.store(count as u64, Ordering::Relaxed);
                }
            }
        }

        // ATP tokens
        if let Ok(data) = std::fs::read_to_string(ATP_TOKENS_CACHE_PATH) {
            if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&data) {
                let count = map.len();
                if let Ok(mut cache) = self.atp_tokens.write() {
                    cache.clear();
                    cache.extend(map);
                    result.atp_loaded = count;
                    self.stats.atp_count.store(count as u64, Ordering::Relaxed);
                }
            }
        }

        // Ligue 1 tokens
        if let Ok(data) = std::fs::read_to_string(LIGUE1_TOKENS_CACHE_PATH) {
            if let Ok(tokens) = serde_json::from_str::<Vec<String>>(&data) {
                let count = tokens.len();
                if let Ok(mut cache) = self.ligue1_tokens.write() {
                    cache.clear();
                    cache.extend(tokens.into_iter().map(|t| (t, ())));
                    result.ligue1_loaded = count;
                    self.stats
                        .ligue1_count
                        .store(count as u64, Ordering::Relaxed);
                }
            }
        }

        // live status
        if let Ok(data) = std::fs::read_to_string(LIVE_CACHE_PATH) {
            if let Ok(map) = serde_json::from_str::<HashMap<String, bool>>(&data) {
                let count = map.len();
                if let Ok(mut cache) = self.live_status.write() {
                    cache.clear();
                    cache.extend(map);
                    result.live_loaded = count;
                    self.stats.live_count.store(count as u64, Ordering::Relaxed);
                }
            }
        }

        let elapsed_ms = start.elapsed().as_millis() as u64;
        result.load_time_ms = elapsed_ms;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.last_refresh.store(now, Ordering::Relaxed);
        self.stats.refresh_count.fetch_add(1, Ordering::Relaxed);
        self.stats
            .last_refresh_duration_ms
            .store(elapsed_ms, Ordering::Relaxed);

        result
    }

    // ------------------------------------------------------------------------
    // Lookups
    // ------------------------------------------------------------------------

    #[inline]
    pub fn is_neg_risk(&self, token_id: &str) -> Option<bool> {
        self.neg_risk.read().ok()?.get(token_id).copied()
    }

    #[inline]
    pub fn get_slug(&self, token_id: &str) -> Option<String> {
        self.slugs.read().ok()?.get(token_id).cloned()
    }

    #[inline]
    pub fn is_atp_token(&self, token_id: &str) -> bool {
        self.atp_tokens
            .read()
            .map(|c| c.contains_key(token_id))
            .unwrap_or(false)
    }

    #[inline]
    pub fn is_ligue1_token(&self, token_id: &str) -> bool {
        self.ligue1_tokens
            .read()
            .map(|c| c.contains_key(token_id))
            .unwrap_or(false)
    }

    #[inline]
    pub fn get_atp_buffer(&self, token_id: &str) -> f64 {
        if self.is_atp_token(token_id) {
            ATP_BUFFER
        } else {
            0.0
        }
    }

    #[inline]
    pub fn get_ligue1_buffer(&self, token_id: &str) -> f64 {
        if self.is_ligue1_token(token_id) {
            LIGUE1_BUFFER
        } else {
            0.0
        }
    }

    #[inline]
    pub fn get_is_live(&self, token_id: &str) -> Option<bool> {
        self.live_status.read().ok()?.get(token_id).copied()
    }

    // ------------------------------------------------------------------------
    // Backward compatibility (do NOT remove yet)
    // ------------------------------------------------------------------------

    #[inline]
    pub fn is_soccer_token(&self, token_id: &str) -> bool {
        self.is_ligue1_token(token_id)
    }

    #[inline]
    pub fn is_tennis_token(&self, token_id: &str) -> bool {
        self.is_atp_token(token_id)
    }

    pub fn set_neg_risk(&self, token_id: String, neg_risk: bool) {
        if let Ok(mut cache) = self.neg_risk.write() {
            cache.insert(token_id, neg_risk);
        }
    }

    pub fn needs_refresh(&self) -> bool {
        let last = self.last_refresh.load(Ordering::Relaxed);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now - last >= CACHE_REFRESH_INTERVAL_SECS
    }
}

impl Default for MarketCaches {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Load Result
// ============================================================================

#[derive(Debug, Default)]
pub struct CacheLoadResult {
    pub neg_risk_loaded: usize,
    pub slugs_loaded: usize,
    pub atp_loaded: usize,
    pub ligue1_loaded: usize,
    pub live_loaded: usize,
    pub load_time_ms: u64,
}

impl std::fmt::Display for CacheLoadResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Loaded caches in {}ms: neg_risk={}, slugs={}, atp={}, ligue1={}, live={}",
            self.load_time_ms,
            self.neg_risk_loaded,
            self.slugs_loaded,
            self.atp_loaded,
            self.ligue1_loaded,
            self.live_loaded
        )
    }
}

// ============================================================================
// Global Cache Instance
// ============================================================================

use std::sync::OnceLock;

static GLOBAL_CACHES: OnceLock<MarketCaches> = OnceLock::new();

pub fn global_caches() -> &'static MarketCaches {
    GLOBAL_CACHES.get_or_init(MarketCaches::new)
}

pub fn init_caches() -> CacheLoadResult {
    let caches = global_caches();
    let result = caches.load_all();
    println!("📦 {}", result);
    result
}

pub fn refresh_caches() -> CacheLoadResult {
    let caches = global_caches();
    let result = caches.load_all();
    println!("🔄 Cache refresh: {}", result);
    result
}

// ============================================================================
// Async Refresh Task
// ============================================================================

pub fn spawn_cache_refresh_task() -> tokio::task::JoinHandle<()> {
    tokio::spawn(async {
        let interval = Duration::from_secs(CACHE_REFRESH_INTERVAL_SECS);
        loop {
            tokio::time::sleep(interval).await;
            let _ = tokio::task::spawn_blocking(refresh_caches).await;
        }
    })
}

// ============================================================================
// Convenience Functions
// ============================================================================

#[inline]
pub fn get_atp_token_buffer(token_id: &str) -> f64 {
    global_caches().get_atp_buffer(token_id)
}

#[inline]
pub fn get_ligue1_token_buffer(token_id: &str) -> f64 {
    global_caches().get_ligue1_buffer(token_id)
}

#[inline]
pub fn get_slug(token_id: &str) -> Option<String> {
    global_caches().get_slug(token_id)
}

#[inline]
pub fn is_neg_risk(token_id: &str) -> Option<bool> {
    global_caches().is_neg_risk(token_id)
}

#[inline]
pub fn get_is_live(token_id: &str) -> Option<bool> {
    global_caches().get_is_live(token_id)
}
