/// Market cache management with automatic refresh
/// Handles caching of market data, tokens, and live status

use rustc_hash::FxHashMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
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

/// Price buffer adjustments for specialized markets
const TENNIS_BUFFER: f64 = 0.01;
const SOCCER_BUFFER: f64 = 0.01;

// ============================================================================
// Cache Data Structures
// ============================================================================

/// Holds all cached market data
pub struct MarketCaches {
    /// Token ID -> neg_risk boolean
    pub neg_risk: RwLock<FxHashMap<String, bool>>,
    /// Token ID -> market slug
    pub slugs: RwLock<FxHashMap<String, String>>,
    /// Tennis (ATP) token IDs (for price buffer calculation)
    pub tennis_tokens: RwLock<FxHashMap<String, String>>,
    /// Soccer (Ligue 1) token IDs (for price buffer calculation)
    pub soccer_tokens: RwLock<FxHashMap<String, ()>>,
    /// Token ID -> live status (for GTD expiry calculation)
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
    pub tennis_count: AtomicU64,
    pub soccer_count: AtomicU64,
    pub live_count: AtomicU64,
    pub refresh_count: AtomicU64,
    pub last_refresh_duration_ms: AtomicU64,
}

impl MarketCaches {
    pub fn new() -> Self {
        Self {
            neg_risk: RwLock::new(FxHashMap::default()),
            slugs: RwLock::new(FxHashMap::default()),
            tennis_tokens: RwLock::new(FxHashMap::default()),
            soccer_tokens: RwLock::new(FxHashMap::default()),
            live_status: RwLock::new(FxHashMap::default()),
            last_refresh: AtomicU64::new(0),
            stats: CacheStats::default(),
        }
    }

    /// Load all caches from disk
    pub fn load_all(&self) -> CacheLoadResult {
        let start = Instant::now();
        let mut result = CacheLoadResult::default();

        // Load neg_risk cache
        if let Ok(data) = std::fs::read_to_string(NEG_RISK_CACHE_PATH) {
            if let Ok(map) = serde_json::from_str::<HashMap<String, bool>>(&data) {
                let count = map.len();
                if let Ok(mut cache) = self.neg_risk.write() {
                    cache.clear();
                    cache.extend(map.into_iter());
                    result.neg_risk_loaded = count;
                    self.stats.neg_risk_count.store(count as u64, Ordering::Relaxed);
                }
            }
        }

        // Load slug cache
        if let Ok(data) = std::fs::read_to_string(SLUG_CACHE_PATH) {
            if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&data) {
                let count = map.len();
                if let Ok(mut cache) = self.slugs.write() {
                    cache.clear();
                    cache.extend(map.into_iter());
                    result.slugs_loaded = count;
                    self.stats.slug_count.store(count as u64, Ordering::Relaxed);
                }
            }
        }

        // Load ATP tokens cache
        if let Ok(data) = std::fs::read_to_string(ATP_TOKENS_CACHE_PATH) {
            if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&data) {
                let count = map.len();
                if let Ok(mut cache) = self.tennis_tokens.write() {
                    cache.clear();
                    cache.extend(map.into_iter());
                    result.atp_loaded = count;
                    self.stats.tennis_count.store(count as u64, Ordering::Relaxed);
                }
            }
        }

        // Load Ligue 1 tokens cache
        if let Ok(data) = std::fs::read_to_string(LIGUE1_TOKENS_CACHE_PATH) {
            if let Ok(tokens) = serde_json::from_str::<Vec<String>>(&data) {
                let count = tokens.len();
                if let Ok(mut cache) = self.soccer_tokens.write() {
                    cache.clear();
                    cache.extend(tokens.into_iter().map(|t| (t, ())));
                    result.ligue1_loaded = count;
                    self.stats.soccer_count.store(count as u64, Ordering::Relaxed);
                }
            }
        }

        // Load live status cache
        if let Ok(data) = std::fs::read_to_string(LIVE_CACHE_PATH) {
            if let Ok(map) = serde_json::from_str::<HashMap<String, bool>>(&data) {
                let count = map.len();
                if let Ok(mut cache) = self.live_status.write() {
                    cache.clear();
                    cache.extend(map.into_iter());
                    result.live_loaded = count;
                    self.stats.live_count.store(count as u64, Ordering::Relaxed);
                }
            }
        }

        let elapsed = start.elapsed();
        result.load_time_ms = elapsed.as_millis() as u64;

        // Update last refresh timestamp
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.last_refresh.store(now, Ordering::Relaxed);
        self.stats.refresh_count.fetch_add(1, Ordering::Relaxed);
        self.stats.last_refresh_duration_ms.store(result.load_time_ms, Ordering::Relaxed);

        result
    }

    /// Check if token is neg_risk
    #[inline]
    pub fn is_neg_risk(&self, token_id: &str) -> Option<bool> {
        self.neg_risk.read().ok()?.get(token_id).copied()
    }

    /// Get slug for token
    #[inline]
    pub fn get_slug(&self, token_id: &str) -> Option<String> {
        self.slugs.read().ok()?.get(token_id).cloned()
    }

    /// Check if token is ATP market
    #[inline]
    pub fn is_atp_token(&self, token_id: &str) -> bool {
        self.tennis_tokens.read().map(|c| c.contains_key(token_id)).unwrap_or(false)
    }

    /// Check if token is Ligue 1 market
    #[inline]
    pub fn is_ligue1_token(&self, token_id: &str) -> bool {
        self.soccer_tokens.read().map(|c| c.contains_key(token_id)).unwrap_or(false)
    }

    /// Get ATP buffer for token (0.01 if ATP, 0.0 otherwise)
    #[inline]
    pub fn get_atp_buffer(&self, token_id: &str) -> f64 {
        if self.is_atp_token(token_id) { TENNIS_BUFFER } else { 0.0 }
    }

    /// Get Ligue 1 buffer for token (0.01 if Ligue1, 0.0 otherwise)
    #[inline]
    pub fn get_ligue1_buffer(&self, token_id: &str) -> f64 {
        if self.is_ligue1_token(token_id) { SOCCER_BUFFER } else { 0.0 }
    }

    /// Get live status for token (for GTD expiry calculation)
    #[inline]
    pub fn get_is_live(&self, token_id: &str) -> Option<bool> {
        self.live_status.read().ok()?.get(token_id).copied()
    }

    /// Insert neg_risk value for a token (for dynamic updates)
    pub fn set_neg_risk(&self, token_id: String, neg_risk: bool) {
        if let Ok(mut cache) = self.neg_risk.write() {
            cache.insert(token_id, neg_risk);
        }
    }

    /// Insert slug for a token (for dynamic updates)
    pub fn set_slug(&self, token_id: String, slug: String) {
        if let Ok(mut cache) = self.slugs.write() {
            cache.insert(token_id, slug);
        }
    }

    /// Get cache statistics summary
    pub fn get_stats_summary(&self) -> String {
        format!(
            "Caches: neg_risk={}, slugs={}, atp={}, ligue1={}, refreshes={}",
            self.stats.neg_risk_count.load(Ordering::Relaxed),
            self.stats.slug_count.load(Ordering::Relaxed),
            self.stats.tennis_count.load(Ordering::Relaxed),
            self.stats.soccer_count.load(Ordering::Relaxed),
            self.stats.refresh_count.load(Ordering::Relaxed),
        )
    }

    /// Check if cache refresh is needed
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

/// Get the global cache instance
pub fn global_caches() -> &'static MarketCaches {
    GLOBAL_CACHES.get_or_init(MarketCaches::new)
}

/// Initialize and load all caches (call once at startup)
pub fn init_caches() -> CacheLoadResult {
    let caches = global_caches();
    let result = caches.load_all();
    println!("📦 {}", result);
    result
}

/// Refresh all caches (call periodically)
pub fn refresh_caches() -> CacheLoadResult {
    let caches = global_caches();
    let result = caches.load_all();
    println!("🔄 Cache refresh: {}", result);
    result
}

// ============================================================================
// Async Cache Refresh Task
// ============================================================================

/// Spawn a background task that periodically refreshes caches
/// Returns a handle that can be used to abort the task
pub fn spawn_cache_refresh_task() -> tokio::task::JoinHandle<()> {
    tokio::spawn(async {
        let interval = Duration::from_secs(CACHE_REFRESH_INTERVAL_SECS);
        println!("🔄 Cache refresh task started (interval: {}s)", CACHE_REFRESH_INTERVAL_SECS);

        loop {
            tokio::time::sleep(interval).await;

            // Run cache refresh in blocking task to avoid blocking async runtime
            let result = tokio::task::spawn_blocking(refresh_caches).await;

            if let Err(e) = result {
                eprintln!("⚠️ Cache refresh task error: {}", e);
            }
        }
    })
}

// ============================================================================
// Convenience Functions (for backwards compatibility)
// ============================================================================

/// Get ATP buffer for a token (convenience function)
#[inline]
pub fn get_atp_token_buffer(token_id: &str) -> f64 {
    global_caches().get_atp_buffer(token_id)
}

/// Get Ligue 1 buffer for a token (convenience function)
#[inline]
pub fn get_ligue1_token_buffer(token_id: &str) -> f64 {
    global_caches().get_ligue1_buffer(token_id)
}

/// Get tennis buffer for a token (convenience function, alias for ATP)
#[inline]
pub fn get_tennis_token_buffer(token_id: &str) -> f64 {
    global_caches().get_atp_buffer(token_id)
}

/// Get soccer buffer for a token (convenience function, alias for Ligue 1)
#[inline]
pub fn get_soccer_token_buffer(token_id: &str) -> f64 {
    global_caches().get_ligue1_buffer(token_id)
}

/// Check if token is tennis market (convenience function, alias for ATP)
#[inline]
pub fn is_tennis_token(token_id: &str) -> bool {
    global_caches().is_atp_token(token_id)
}

/// Check if token is soccer market (convenience function, alias for Ligue 1)
#[inline]
pub fn is_soccer_token(token_id: &str) -> bool {
    global_caches().is_ligue1_token(token_id)
}

/// Get slug for a token (convenience function)
#[inline]
pub fn get_slug(token_id: &str) -> Option<String> {
    global_caches().get_slug(token_id)
}

/// Check if token is neg_risk (convenience function)
#[inline]
pub fn is_neg_risk(token_id: &str) -> Option<bool> {
    global_caches().is_neg_risk(token_id)
}

/// Get is_live for a token (convenience function)
#[inline]
pub fn get_is_live(token_id: &str) -> Option<bool> {
    global_caches().get_is_live(token_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_creation() {
        let caches = MarketCaches::new();
        assert!(caches.neg_risk.read().unwrap().is_empty());
        assert!(caches.slugs.read().unwrap().is_empty());
        assert!(caches.tennis_tokens.read().unwrap().is_empty());
        assert!(caches.soccer_tokens.read().unwrap().is_empty());
    }

    #[test]
    fn test_set_and_get_neg_risk() {
        let caches = MarketCaches::new();
        caches.set_neg_risk("token123".to_string(), true);
        assert_eq!(caches.is_neg_risk("token123"), Some(true));
        assert_eq!(caches.is_neg_risk("unknown"), None);
    }

    #[test]
    fn test_set_and_get_slug() {
        let caches = MarketCaches::new();
        caches.set_slug("token123".to_string(), "market-slug".to_string());
        assert_eq!(caches.get_slug("token123"), Some("market-slug".to_string()));
        assert_eq!(caches.get_slug("unknown"), None);
    }

    #[test]
    fn test_buffer_values() {
        assert_eq!(ATP_BUFFER, 0.01);
        assert_eq!(LIGUE1_BUFFER, 0.01);
    }

    #[test]
    fn test_non_existent_returns_zero_buffer() {
        let caches = MarketCaches::new();
        assert_eq!(caches.get_atp_buffer("nonexistent"), 0.0);
        assert_eq!(caches.get_ligue1_buffer("nonexistent"), 0.0);
    }
}
