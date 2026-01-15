/// Tennis market detection and price buffer adjustments
/// Uses market cache for efficient token lookups

use crate::market_cache;

/// Get tennis market price buffer (0.01 for tennis tokens, 0.0 otherwise)
/// Uses the global refreshable market cache
#[inline]
pub fn get_tennis_token_buffer(token_id: &str) -> f64 {
    market_cache::get_tennis_token_buffer(token_id)
}

/// Check if token represents a tennis market
#[inline]
pub fn is_tennis_token(token_id: &str) -> bool {
    market_cache::is_tennis_token(token_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_non_tennis_token_returns_zero() {
        // Non-tennis tokens should return 0.0 buffer
        let buffer = get_tennis_token_buffer("fake_non_tennis_token_12345");
        assert_eq!(buffer, 0.0, "Non-tennis token should have 0 buffer");
    }

    #[test]
    fn test_is_tennis_token_false_for_unknown() {
        assert!(!is_tennis_token("unknown_token_xyz"));
    }
}
