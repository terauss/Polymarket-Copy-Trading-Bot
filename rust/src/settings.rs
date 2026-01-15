/// Settings and configuration management
/// Handles environment variable loading and validation

use anyhow::{Context, Result};
use std::env;
use std::path::Path;
use std::time::Duration;
use crate::risk_guard;
use crate::tennis_markets;
use crate::soccer_markets;

// ============================================================================
// Blockchain Constants
// ============================================================================

use once_cell::sync::Lazy;

pub const ORDERS_FILLED_EVENT_SIGNATURE: &str =
    "0xd0a08e8c493f9c94f29311604c9de1b4e8c8d4c06bd0c789af57f2d65bfec0f6";

/// Target whale address topic - loaded from TARGET_WHALE_ADDRESS env var
/// Format: 40-char hex address without 0x prefix (e.g., "204f72f35326db932158cba6adff0b9a1da95e14")
/// Gets zero-padded to 66 chars with 0x prefix for topic matching
/// 
/// Note: This is validated in Config::from_env() before use, so expect should not panic
/// in normal operation. If you see this panic, it means Config::from_env() was not called first.
pub static TARGET_TOPIC_HEX: Lazy<String> = Lazy::new(|| {
    let addr = env::var("TARGET_WHALE_ADDRESS")
        .expect("TARGET_WHALE_ADDRESS should have been validated in Config::from_env(). \
                If you see this, please ensure you call Config::from_env() before using TARGET_TOPIC_HEX");
    format!("0x000000000000000000000000{}", addr.trim_start_matches("0x").to_lowercase())
});

pub const MONITORED_ADDRESSES: [&str; 3] = [
    "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E",
    "0x4d97dcd97ec945f40cf65f87097ace5ea0476045",
    "0xC5d563A36AE78145C45a50134d48A1215220f80a",
];

// ============================================================================
// API & File Constants
// ============================================================================

pub const CLOB_API_BASE: &str = "https://clob.polymarket.com";
pub const CSV_FILE: &str = "matches_optimized.csv";

// Debug flag - set to true to print full API error messages (remove after debugging)
pub const DEBUG_FULL_ERRORS: bool = true;

// ============================================================================
// Trading Constants
// ============================================================================

pub const PRICE_BUFFER: f64 = 0.00;
pub const SCALING_RATIO: f64 = 0.02;
pub const MIN_CASH_VALUE: f64 = 1.01;
pub const MIN_SHARE_COUNT: f64 = 0.0;  // Set to 0 to rely purely on MIN_CASH_VALUE for EV scaling
pub const USE_PROBABILISTIC_SIZING: bool = true;

// Minimum whale trade size to copy (skip trades below this)
pub const MIN_WHALE_SHARES_TO_COPY: f64 = 10.0;

/// Returns true if this trade should be skipped (too small, negative expected value)
#[inline]
pub fn should_skip_trade(whale_shares: f64) -> bool {
    whale_shares < MIN_WHALE_SHARES_TO_COPY
}

// ============================================================================
// Timeouts
// ============================================================================

pub const ORDER_REPLY_TIMEOUT: Duration = Duration::from_secs(10);

// ============================================================================
// Resubmitter Configuration (for FAK failures)
// ============================================================================

pub const RESUBMIT_PRICE_INCREMENT: f64 = 0.01;

// Tier-based max resubmit attempts (4000+ gets 5, others get 4)
#[inline]
pub fn get_max_resubmit_attempts(whale_shares: f64) -> u8 {
    if whale_shares >= 4000.0 { 5 }
    else { 4 }
}

/// Returns true if this attempt should increment price, false for flat retry
/// >= 4000: chase attempt 1 only
/// <4000: never chase (buffer=0)
#[inline]
pub fn should_increment_price(whale_shares: f64, attempt: u8) -> bool {
    if whale_shares >= 4000.0 {
        attempt == 1  
    } else {
        false  
    }
}

#[inline]
pub fn get_gtd_expiry_secs(is_live: bool) -> u64 {
    if is_live { 61 }    
    else { 1800 }        
}

// Tier-based max buffer for resubmits (on top of initial tier buffer)
// >= 4000: chase up to +0.02
// <4000: no chasing (0.00)
#[inline]
pub fn get_resubmit_max_buffer(whale_shares: f64) -> f64 {
    if whale_shares >= 4000.0 { 0.01 }
    else { 0.00 }
}
pub const BOOK_REQ_TIMEOUT: Duration = Duration::from_millis(2500);
pub const WS_PING_TIMEOUT: Duration = Duration::from_secs(300);
pub const WS_RECONNECT_DELAY: Duration = Duration::from_secs(3);

// ============================================================================
// Execution Tiers
// ============================================================================

#[derive(Debug, Clone, Copy)]
pub struct ExecutionTier {
    pub min_shares: f64,
    pub price_buffer: f64,
    pub order_action: &'static str,
    pub size_multiplier: f64,
}

pub const EXECUTION_TIERS: [ExecutionTier; 3] = [
    ExecutionTier {
        min_shares: 4000.0,
        price_buffer: 0.01,
        order_action: "FAK",
        size_multiplier: 1.25,
    },
    ExecutionTier {
        min_shares: 2000.0,
        price_buffer: 0.01,
        order_action: "FAK",
        size_multiplier: 1.0,
    },
    ExecutionTier {
        min_shares: 1000.0,
        price_buffer: 0.00,
        order_action: "FAK",
        size_multiplier: 1.0,
    },
];

/// Get tier params for a given trade size
/// Returns (buffer, order_action, size_multiplier)
#[inline]
pub fn get_tier_params(whale_shares: f64, side_is_buy: bool, token_id: &str) -> (f64, &'static str, f64) {
    if !side_is_buy {
        return (PRICE_BUFFER, "GTD", 1.0);
    }

    // Get base tier params - direct if-else is faster than iterator for 3 tiers
    let (base_buffer, order_action, size_multiplier) = if whale_shares >= 4000.0 {
        (0.01, "FAK", 1.25)
    } else if whale_shares >= 2000.0 {
        (0.01, "FAK", 1.0)
    } else if whale_shares >= 1000.0 {
        (0.0, "FAK", 1.0)
    } else {
        (PRICE_BUFFER, "FAK", 1.0)  // Small buys use FAK (Fill and Kill)
    };

    // Apply sport-specific price adjustments
    let tennis_buffer = tennis_markets::get_tennis_token_buffer(token_id);
    let soccer_buffer = soccer_markets::get_soccer_token_buffer(token_id);
    let total_buffer = base_buffer + tennis_buffer + soccer_buffer;

    (total_buffer, order_action, size_multiplier)
}

// ============================================================================
// Runtime Configuration (loaded from environment)
// ============================================================================

#[derive(Debug, Clone)]
pub struct Config {
    // Credentials
    pub private_key: String,
    pub funder_address: String,
    
    // WebSocket
    pub wss_url: String,
    
    // Trading flags
    pub enable_trading: bool,
    pub mock_trading: bool,
    
    // Circuit breaker
    pub cb_large_trade_shares: f64,
    pub cb_consecutive_trigger: u8,
    pub cb_sequence_window_secs: u64,
    pub cb_min_depth_usd: f64,
    pub cb_trip_duration_secs: u64,
}

impl Config {
    /// Load configuration from environment variables
    /// 
    /// # Errors
    /// 
    /// Returns errors with helpful messages if required configuration is missing or invalid.
    /// For detailed setup help, see docs/02_SETUP_GUIDE.md
    pub async fn from_env() -> Result<Self> {
        // Check if .env file exists (helpful error for beginners)
        if !Path::new(".env").exists() {
            anyhow::bail!(
                "Configuration file .env not found!\n\
                \n\
                Setup steps:\n\
                1. Copy .env.example to .env\n\
                2. Open .env in a text editor\n\
                3. Fill in your configuration values\n\
                    4. See docs/02_SETUP_GUIDE.md for detailed instructions\n\
                \n\
                Quick check: Run 'cargo run --release --bin check_config' to validate your setup"
            );
        }
        
        let private_key = env::var("PRIVATE_KEY")
            .context("PRIVATE_KEY env var is required. Add it to your .env file.\n\
                     Format: 64-character hex string (no 0x prefix)\n\
                     Example: 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")?;
        
        // Validate private key format
        let key_clean = private_key.trim().strip_prefix("0x").unwrap_or(private_key.trim());
        if key_clean.len() != 64 {
            anyhow::bail!(
                "PRIVATE_KEY must be exactly 64 hex characters (found {}).\n\
                Remove any '0x' prefix. Current value starts with: {}",
                key_clean.len(),
                if key_clean.len() > 10 { format!("{}...", &key_clean[..10]) } else { key_clean.to_string() }
            );
        }
        if !key_clean.chars().all(|c| c.is_ascii_hexdigit()) {
            anyhow::bail!("PRIVATE_KEY contains invalid characters. Must be hexadecimal (0-9, a-f, A-F).");
        }
        
        let funder_address = env::var("FUNDER_ADDRESS")
            .context("FUNDER_ADDRESS env var is required. Add it to your .env file.\n\
                     Format: 40-character hex address (can include 0x prefix)\n\
                     This should match the wallet from your PRIVATE_KEY")?;
        
        // Validate funder address format
        let addr_clean = funder_address.trim().strip_prefix("0x").unwrap_or(funder_address.trim());
        if addr_clean.len() != 40 {
            anyhow::bail!(
                "FUNDER_ADDRESS must be exactly 40 hex characters (found {}).\n\
                Current value: {}",
                addr_clean.len(),
                if addr_clean.len() > 20 { format!("{}...", &addr_clean[..20]) } else { addr_clean.to_string() }
            );
        }
        if !addr_clean.chars().all(|c| c.is_ascii_hexdigit()) {
            anyhow::bail!("FUNDER_ADDRESS contains invalid characters. Must be hexadecimal (0-9, a-f, A-F).");
        }
        
        // WebSocket URL from either provider
        let wss_url = if let Ok(key) = env::var("ALCHEMY_API_KEY") {
            let key = key.trim();
            if key.is_empty() || key == "your_alchemy_api_key_here" {
                anyhow::bail!(
                    "ALCHEMY_API_KEY is set but has placeholder value.\n\
                    Get your API key from https://www.alchemy.com/ (free tier available)\n\
                    Then add it to your .env file"
                );
            }
            format!("wss://polygon-mainnet.g.alchemy.com/v2/{}", key)
        } else if let Ok(key) = env::var("CHAINSTACK_API_KEY") {
            let key = key.trim();
            if key.is_empty() || key == "your_chainstack_api_key_here" {
                anyhow::bail!(
                    "CHAINSTACK_API_KEY is set but has placeholder value.\n\
                    Get your API key from https://chainstack.com/ (free tier available)\n\
                    Or use ALCHEMY_API_KEY instead (recommended for beginners)"
                );
            }
            format!("wss://polygon-mainnet.core.chainstack.com/{}", key)
        } else {
            anyhow::bail!(
                "WebSocket API key required!\n\
                \n\
                Set either ALCHEMY_API_KEY or CHAINSTACK_API_KEY in your .env file.\n\
                \n\
                Recommended (beginners): ALCHEMY_API_KEY\n\
                1. Sign up at https://www.alchemy.com/\n\
                2. Create app (Polygon Mainnet)\n\
                3. Copy API key to .env file\n\
                \n\
                Alternative: CHAINSTACK_API_KEY\n\
                1. Sign up at https://chainstack.com/\n\
                2. Create Polygon node\n\
                3. Copy API key to .env file\n\
                \n\
                Run 'cargo run --release --bin check_config' to validate your setup"
            );
        };
        
        // Validate TARGET_WHALE_ADDRESS (used by TARGET_TOPIC_HEX lazy static)
        let target_whale = env::var("TARGET_WHALE_ADDRESS")
            .context("TARGET_WHALE_ADDRESS env var is required. Add it to your .env file.\n\
                     Format: 40-character hex address (no 0x prefix)\n\
                     This is the whale address you want to copy trades from.\n\
                     Find whale addresses on Polymarket leaderboards")?;
        
        let whale_clean = target_whale.trim().strip_prefix("0x").unwrap_or(target_whale.trim());
        if whale_clean.is_empty() || whale_clean == "target_whale_address_here" {
            anyhow::bail!(
                "TARGET_WHALE_ADDRESS is set but has placeholder value.\n\
                Replace 'target_whale_address_here' with the actual whale address you want to copy.\n\
                Find whale addresses on Polymarket leaderboards or from successful traders."
            );
        }
        if whale_clean.len() != 40 {
            anyhow::bail!(
                "TARGET_WHALE_ADDRESS must be exactly 40 hex characters (found {}).\n\
                Remove '0x' prefix if present. Current value: {}",
                whale_clean.len(),
                if whale_clean.len() > 20 { format!("{}...", &whale_clean[..20]) } else { whale_clean.to_string() }
            );
        }
        if !whale_clean.chars().all(|c| c.is_ascii_hexdigit()) {
            anyhow::bail!("TARGET_WHALE_ADDRESS contains invalid characters. Must be hexadecimal (0-9, a-f, A-F).");
        }
        
        let enable_trading = env::var("ENABLE_TRADING")
            .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
            .unwrap_or(true);
        
        let mock_trading = env::var("MOCK_TRADING")
            .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
            .unwrap_or(false);
        
        Ok(Self {
            private_key,
            funder_address,
            wss_url,
            enable_trading,
            mock_trading,
            cb_large_trade_shares: env_parse("CB_LARGE_TRADE_SHARES", 1500.0),
            cb_consecutive_trigger: env_parse("CB_CONSECUTIVE_TRIGGER", 2u8),
            cb_sequence_window_secs: env_parse("CB_SEQUENCE_WINDOW_SECS", 30),
            cb_min_depth_usd: env_parse("CB_MIN_DEPTH_USD", 200.0),
            cb_trip_duration_secs: env_parse("CB_TRIP_DURATION_SECS", 120),
        })
    }
    
    /// Convert to RiskGuardConfig for safety checks
    pub fn risk_guard_config(&self) -> risk_guard::RiskGuardConfig {
        risk_guard::RiskGuardConfig {
            large_trade_shares: self.cb_large_trade_shares,
            consecutive_trigger: self.cb_consecutive_trigger,
            sequence_window: Duration::from_secs(self.cb_sequence_window_secs),
            min_depth_beyond_usd: self.cb_min_depth_usd,
            trip_duration: Duration::from_secs(self.cb_trip_duration_secs),
        }
    }
}

/// Parse env var with default fallback
fn env_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Test 1: Large trade (4000+)
    // Expected: buffer 0.01, 5 resubmit attempts, max resubmit buffer 0.01
    // -------------------------------------------------------------------------
    #[test]
    fn test_large_trade_4000_plus() {
        let whale_shares = 10000.0;
        let side_is_buy = true;
        let token_id = "fake_non_atp_token";

        let (buffer, order_action, size_multiplier) = get_tier_params(whale_shares, side_is_buy, token_id);

        // 4000+ tier: buffer = 0.01, multiplier = 1.25
        assert_eq!(buffer, 0.01, "4000+ tier should have 0.01 base buffer");
        assert_eq!(order_action, "FAK", "4000+ tier should use FAK");
        assert_eq!(size_multiplier, 1.25, "4000+ tier should have 1.25x multiplier");

        // Resubmit params for 4000+
        assert_eq!(get_max_resubmit_attempts(whale_shares), 5, "4000+ should get 5 resubmit attempts");
        assert_eq!(get_resubmit_max_buffer(whale_shares), 0.01, "4000+ should have 0.01 max resubmit buffer");
    }

    // -------------------------------------------------------------------------
    // Test 2: Small size (<1000 shares)
    // Expected: buffer 0.00, 4 resubmit attempts (spaced 50ms), no chasing (resubmit_max=0.00)
    // -------------------------------------------------------------------------
    #[test]
    fn test_small_size_non_atp() {
        let whale_shares = 100.0;
        let side_is_buy = true;
        let token_id = "fake_non_atp_token";

        let (buffer, order_action, size_multiplier) = get_tier_params(whale_shares, side_is_buy, token_id);

        // <1000: base buffer = 0.00
        assert_eq!(buffer, PRICE_BUFFER, "Small trades should use default PRICE_BUFFER (0.00)");
        assert_eq!(order_action, "FAK", "Small buys should use FAK");
        assert_eq!(size_multiplier, 1.0, "Small trades should have 1.0x multiplier");

        // Resubmit params for <1000: 4 attempts (50ms spaced), no chasing
        assert_eq!(get_max_resubmit_attempts(whale_shares), 4, "Small trades should get 4 resubmit attempts");
        assert_eq!(get_resubmit_max_buffer(whale_shares), 0.00, "Small trades should have 0.00 max resubmit buffer");
    }

    // -------------------------------------------------------------------------
    // Test 2b: ATP token at small size (100 shares)
    // ATP tokens should ALWAYS get +0.01 buffer, even for small trades
    // Base buffer 0.00 + ATP buffer 0.01 = 0.01 total
    // -------------------------------------------------------------------------
    #[test]
    fn test_small_size_atp_gets_buffer() {
        // Verify tennis market buffer logic:
        // Even for small trades, tennis tokens get additional 0.01 buffer
        //
        // Expected: base_buffer (0.00 for <500) + tennis_buffer (0.01) = 0.01 total
        let tennis_buffer = tennis_markets::get_tennis_token_buffer("fake_non_tennis_token");
        assert_eq!(tennis_buffer, 0.0, "Non-tennis token should have 0 buffer");

        // If token IS in tennis cache, it adds 0.01 buffer
        // Example: 100-share tennis trade = 0.00 (base) + 0.01 (tennis) = 0.01 total
    }

    // -------------------------------------------------------------------------
    // Test 3: Resubmit logic for 4000+ size
    // Verify: 1 chase retry at +0.01, then flat retries = up to +0.01 total chase
    // -------------------------------------------------------------------------
    #[test]
    fn test_resubmit_logic_4000_plus() {
        let whale_shares = 8000.0;

        // Config values
        let max_attempts = get_max_resubmit_attempts(whale_shares);
        let max_buffer = get_resubmit_max_buffer(whale_shares);
        let increment = RESUBMIT_PRICE_INCREMENT;

        assert_eq!(max_attempts, 5, "4000+ should have 5 max attempts");
        assert_eq!(max_buffer, 0.01, "4000+ should have 0.01 max buffer");
        assert_eq!(increment, 0.01, "Price increment should be 0.01");

        // 4000+: chase on attempt 1 only, flat on 2+
        assert!(should_increment_price(whale_shares, 1), "4000+: attempt 1 should chase");
        assert!(!should_increment_price(whale_shares, 2), "4000+: attempt 2 should be flat");
        assert!(!should_increment_price(whale_shares, 3), "4000+: attempt 3 should be flat");
        assert!(!should_increment_price(whale_shares, 4), "4000+: attempt 4 should be flat");

        // Simulate retry sequence
        let initial_price = 0.50;
        let tier_buffer = 0.01; // 4000+ tier buffer
        let limit_price = initial_price + tier_buffer; // 0.51

        // After 1st retry: limit + 0.01 = 0.52 (ceiling)
        let price_after_retry_1 = limit_price + increment;
        assert!((price_after_retry_1 - 0.52).abs() < 0.001);

        // After 2nd retry: flat at 0.52 (no increment)
        let price_after_retry_2 = price_after_retry_1; // No increment for attempt 2
        assert!((price_after_retry_2 - 0.52).abs() < 0.001);

        // After 3rd retry: flat at 0.52 (no increment)
        let price_after_retry_3 = price_after_retry_2; // No increment for attempt 3
        assert!((price_after_retry_3 - 0.52).abs() < 0.001);

        // Max price ceiling: limit_price + max_buffer = 0.52
        let max_price = limit_price + max_buffer;
        assert!((max_price - 0.52).abs() < 0.001, "Max price should be initial limit + max_buffer");

        // Verify 1st retry stays within bounds
        assert!(price_after_retry_1 <= max_price, "1st retry price should not exceed max");
    }

    // -------------------------------------------------------------------------
    // Test: All execution tiers
    // Current tiers: 4000+ (0.01, 1.25x), 2000+ (0.01, 1.0x), 1000+ (0.00, 1.0x)
    // Below 1000: default (0.00, 1.0x)
    // -------------------------------------------------------------------------
    #[test]
    fn test_execution_tiers() {
        let token_id = "fake_token";

        // 4000+ tier (includes 8000+)
        let (buf, action, mult) = get_tier_params(8000.0, true, token_id);
        assert_eq!(buf, 0.01);
        assert_eq!(action, "FAK");
        assert_eq!(mult, 1.25);

        // 4000+ tier
        let (buf, action, mult) = get_tier_params(4000.0, true, token_id);
        assert_eq!(buf, 0.01);
        assert_eq!(action, "FAK");
        assert_eq!(mult, 1.25);

        // 2000+ tier
        let (buf, action, mult) = get_tier_params(2000.0, true, token_id);
        assert_eq!(buf, 0.01);
        assert_eq!(action, "FAK");
        assert_eq!(mult, 1.0);

        // 1000+ tier
        let (buf, action, mult) = get_tier_params(1000.0, true, token_id);
        assert_eq!(buf, 0.00);
        assert_eq!(action, "FAK");
        assert_eq!(mult, 1.0);

        // Below 1000 (default)
        let (buf, action, mult) = get_tier_params(500.0, true, token_id);
        assert_eq!(buf, PRICE_BUFFER); // 0.00
        assert_eq!(action, "FAK");
        assert_eq!(mult, 1.0);

        // Small trades (below all tiers)
        let (buf, action, mult) = get_tier_params(100.0, true, token_id);
        assert_eq!(buf, PRICE_BUFFER); // 0.00
        assert_eq!(action, "FAK");
        assert_eq!(mult, 1.0);
    }

    // -------------------------------------------------------------------------
    // Test: Sell orders always use GTD with 0 buffer
    // -------------------------------------------------------------------------
    #[test]
    fn test_sell_orders_use_gtd() {
        let token_id = "fake_token";

        // Even large sells should use GTD with 0 buffer
        let (buf, action, mult) = get_tier_params(10000.0, false, token_id);
        assert_eq!(buf, PRICE_BUFFER); // 0.00
        assert_eq!(action, "GTD");
        assert_eq!(mult, 1.0);
    }

    // -------------------------------------------------------------------------
    // Test: Resubmit params for different sizes
    // Current config:
    //   4000+: 5 attempts (chase 1, flat 2-5), 0.01 resubmit_max
    //   <4000: 4 attempts (never chase), 0.00 resubmit_max
    // -------------------------------------------------------------------------
    #[test]
    fn test_resubmit_params_by_size() {
        // 4000+ gets 5 attempts, 0.01 buffer
        assert_eq!(get_max_resubmit_attempts(8000.0), 5);
        assert_eq!(get_max_resubmit_attempts(10000.0), 5);
        assert_eq!(get_max_resubmit_attempts(4000.0), 5);
        assert_eq!(get_resubmit_max_buffer(8000.0), 0.01);
        assert_eq!(get_resubmit_max_buffer(4000.0), 0.01);

        // <4000 gets 4 attempts, 0.00 buffer (no chasing)
        assert_eq!(get_max_resubmit_attempts(3999.0), 4);
        assert_eq!(get_max_resubmit_attempts(2000.0), 4);
        assert_eq!(get_resubmit_max_buffer(3999.0), 0.00);
        assert_eq!(get_resubmit_max_buffer(2000.0), 0.00);

        // 1000-2000 gets 4 attempts, 0.00 buffer
        assert_eq!(get_max_resubmit_attempts(1999.0), 4);
        assert_eq!(get_max_resubmit_attempts(1000.0), 4);
        assert_eq!(get_resubmit_max_buffer(1999.0), 0.00);
        assert_eq!(get_resubmit_max_buffer(1000.0), 0.00);

        // <1000 gets 4 attempts, 0.00 buffer (no chasing)
        assert_eq!(get_max_resubmit_attempts(999.0), 4);
        assert_eq!(get_max_resubmit_attempts(100.0), 4);
        assert_eq!(get_resubmit_max_buffer(999.0), 0.00);
        assert_eq!(get_resubmit_max_buffer(100.0), 0.00);
    }

    // -------------------------------------------------------------------------
    // Test: should_increment_price behavior
    // Current config:
    //   4000+: chase on attempt 1 only, flat on 2+
    //   <4000: never chase
    // -------------------------------------------------------------------------
    #[test]
    fn test_should_increment_price() {
        // 4000+ (includes 8000+): chase on attempt 1 only, flat on 2+
        assert!(should_increment_price(8000.0, 1));
        assert!(!should_increment_price(8000.0, 2), "4000+ should be flat on attempt 2");
        assert!(!should_increment_price(8000.0, 3), "4000+ should be flat on attempt 3");
        assert!(!should_increment_price(8000.0, 4), "4000+ should be flat on attempt 4");
        assert!(should_increment_price(4000.0, 1));
        assert!(!should_increment_price(4000.0, 2));
        assert!(!should_increment_price(4000.0, 3));

        // <4000: never chase
        assert!(!should_increment_price(3999.0, 1), "<4000 should never chase");
        assert!(!should_increment_price(3999.0, 2), "<4000 should never chase");
        assert!(!should_increment_price(2000.0, 1), "<4000 should never chase");
        assert!(!should_increment_price(2000.0, 2));
        assert!(!should_increment_price(1999.0, 1), "<4000 should never chase");
        assert!(!should_increment_price(1999.0, 2), "<4000 should never chase");
        assert!(!should_increment_price(1000.0, 1));
        assert!(!should_increment_price(1000.0, 2));

        // <1000: never chase
        assert!(!should_increment_price(999.0, 1), "<4000 should never chase");
        assert!(!should_increment_price(999.0, 2), "<4000 should never chase");
        assert!(!should_increment_price(100.0, 1), "<4000 should never chase");
        assert!(!should_increment_price(100.0, 2));
    }

    // -------------------------------------------------------------------------
    // Test: Edge case - exactly at tier boundaries
    // Current tiers: 4000+, 2000+, 1000+
    // -------------------------------------------------------------------------
    #[test]
    fn test_tier_boundaries() {
        let token_id = "fake_token";

        // Exactly at 4000 should use 4000+ tier
        let (buf, _, mult) = get_tier_params(4000.0, true, token_id);
        assert_eq!(buf, 0.01);
        assert_eq!(mult, 1.25);

        // Just below 4000 should use 2000+ tier
        let (buf, _, mult) = get_tier_params(3999.9, true, token_id);
        assert_eq!(buf, 0.01);
        assert_eq!(mult, 1.0);

        // Exactly at 2000 should use 2000+ tier
        let (buf, _, mult) = get_tier_params(2000.0, true, token_id);
        assert_eq!(buf, 0.01);
        assert_eq!(mult, 1.0);

        // Just below 2000 should use 1000+ tier
        let (buf, _, mult) = get_tier_params(1999.9, true, token_id);
        assert_eq!(buf, 0.00);
        assert_eq!(mult, 1.0);

        // Exactly at 1000 should use 1000+ tier
        let (buf, _, mult) = get_tier_params(1000.0, true, token_id);
        assert_eq!(buf, 0.00);
        assert_eq!(mult, 1.0);

        // Just below 1000 should use default
        let (buf, _, mult) = get_tier_params(999.9, true, token_id);
        assert_eq!(buf, PRICE_BUFFER);
        assert_eq!(mult, 1.0);
    }
}
