/// Soccer market detection and price buffer adjustments
/// Uses market cache for efficient token lookups
use crate::market_cache;

/// Get soccer market price buffer (0.01 for soccer tokens, 0.0 otherwise)
/// Uses the global refreshable market cache
#[inline]
pub fn get_soccer_token_buffer(token_id: &str) -> f64 {
    market_cache::get_ligue1_token_buffer(token_id)
}

/// Check if token represents a soccer market
#[inline]
pub fn is_soccer_token(token_id: &str) -> bool {
    market_cache::global_caches().is_ligue1_token(token_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_non_ligue1_token_returns_zero() {
        let buffer = get_ligue1_token_buffer("fake_non_ligue1_token_12345");
        assert_eq!(buffer, 0.0, "Non-Ligue 1 token should have 0 buffer");
    }

    #[test]
    fn test_is_ligue1_market_false_for_unknown() {
        assert!(!is_ligue1_market("unknown_token_xyz"));
    }
}
