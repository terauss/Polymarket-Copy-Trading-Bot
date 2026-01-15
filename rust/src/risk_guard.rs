/// Risk management and safety guard for trade execution
/// Provides protection against dangerous market conditions
#[allow(dead_code)]

use rustc_hash::FxHashMap;
use std::time::{Duration, Instant};

// =============================================================================
// Type Definitions
// =============================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TradeSide {
    Buy,
    Sell,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SafetyDecision {
    Allow,
    Block,
    FetchBook,
}

#[derive(Clone, Copy)]
pub enum SafetyReason {
    Tripped { secs_left: u32 },
    SmallTrade,
    SeqOk { count: u8 },
    SeqNeedBook { count: u8 },
    Trap { seq: u8, depth_usd: u16 },
    DepthOk { seq: u8, depth_usd: u16 },
    BookFetchFailed,
}

impl SafetyReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            SafetyReason::Tripped { .. } => "TRIPPED",
            SafetyReason::SmallTrade => "SMALL_TRADE",
            SafetyReason::SeqOk { .. } => "SEQ_OK",
            SafetyReason::SeqNeedBook { .. } => "SEQ_NEED_BOOK",
            SafetyReason::Trap { .. } => "TRAP",
            SafetyReason::DepthOk { .. } => "DEPTH_OK",
            SafetyReason::BookFetchFailed => "BOOK_FETCH_FAILED",
        }
    }
}

#[derive(Clone, Copy)]
pub struct SafetyEvaluation {
    pub decision: SafetyDecision,
    pub reason: SafetyReason,
    pub consecutive_large: u8,
}

// =============================================================================
// Config
// =============================================================================

#[derive(Clone)]
pub struct RiskGuardConfig {
    pub large_trade_shares: f64,
    pub consecutive_trigger: u8,
    pub sequence_window: Duration,
    pub min_depth_beyond_usd: f64,
    pub trip_duration: Duration,
}

impl Default for RiskGuardConfig {
    fn default() -> Self {
        Self {
            large_trade_shares: 2000.0,
            consecutive_trigger: 5,
            sequence_window: Duration::from_secs(40),
            min_depth_beyond_usd: 200.0,
            trip_duration: Duration::from_secs(60 * 60 * 5), // 5 hours
        }
    }
}

// =============================================================================
// State
// =============================================================================

struct TokenState {
    large_trades: Vec<(Instant, f64)>,
    tripped_until: Option<Instant>,
}

impl TokenState {
    fn new() -> Self {
        Self {
            large_trades: Vec::with_capacity(8),
            tripped_until: None,
        }
    }
}

impl Default for TokenState {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Circuit Breaker
// =============================================================================

pub struct RiskGuard {
    config: RiskGuardConfig,
    tokens: FxHashMap<String, TokenState>,
}

impl RiskGuard {
    pub fn new(config: RiskGuardConfig) -> Self {
        Self {
            config,
            tokens: FxHashMap::default(),
        }
    }
    
    /// Hot path - no allocations if token exists
    #[inline]
    pub fn check_fast(&mut self, token_id: &str, whale_shares: f64) -> SafetyEvaluation {
        let now = Instant::now();
        
        // Use entry API - single lookup instead of get_mut + insert + get_mut
        let state = self.tokens.entry(token_id.to_string()).or_insert_with(TokenState::new);
        
        // Check trip
        if let Some(until) = state.tripped_until {
            if now < until {
                return SafetyEvaluation {
                    decision: SafetyDecision::Block,
                    reason: SafetyReason::Tripped {
                        secs_left: (until - now).as_secs() as u32,
                    },
                    consecutive_large: 0,
                };
            }
            state.tripped_until = None;
        }
        
        // Small trade - fast path
        if whale_shares < self.config.large_trade_shares {
            return SafetyEvaluation {
                decision: SafetyDecision::Allow,
                reason: SafetyReason::SmallTrade,
                consecutive_large: 0,
            };
        }
        
        // Count consecutive (no pruning on hot path)
        let consecutive = Self::count_large_in_window(
            state,
            now,
            self.config.sequence_window,
            self.config.large_trade_shares,
        ) + 1;
        
        // Record trade
        state.large_trades.push((now, whale_shares));
        
        // Lazy prune
        if state.large_trades.len() > 16 {
            let cutoff = now - self.config.sequence_window;
            state.large_trades.retain(|(ts, _)| *ts > cutoff);
        }
        
        let count = consecutive.min(255) as u8;
        
        if consecutive >= self.config.consecutive_trigger as usize {
            SafetyEvaluation {
                decision: SafetyDecision::FetchBook,
                reason: SafetyReason::SeqNeedBook { count },
                consecutive_large: count,
            }
        } else {
            SafetyEvaluation {
                decision: SafetyDecision::Allow,
                reason: SafetyReason::SeqOk { count },
                consecutive_large: count,
            }
        }
    }
    
    #[inline]
    pub fn check_with_book(
        &mut self,
        token_id: &str,
        consecutive: u8,
        depth_beyond_usd: f64,
    ) -> SafetyEvaluation {
        let depth_u16 = (depth_beyond_usd.min(65535.0)) as u16;
        
        if depth_beyond_usd < self.config.min_depth_beyond_usd {
            // Trip - create state if needed
            let state = self.tokens.entry(token_id.to_string()).or_default();
            state.tripped_until = Some(Instant::now() + self.config.trip_duration);
            
            SafetyEvaluation {
                decision: SafetyDecision::Block,
                reason: SafetyReason::Trap {
                    seq: consecutive,
                    depth_usd: depth_u16,
                },
                consecutive_large: consecutive,
            }
        } else {
            SafetyEvaluation {
                decision: SafetyDecision::Allow,
                reason: SafetyReason::DepthOk {
                    seq: consecutive,
                    depth_usd: depth_u16,
                },
                consecutive_large: consecutive,
            }
        }
    }
    
    pub fn trip(&mut self, token_id: &str) {
        if let Some(state) = self.tokens.get_mut(token_id) {
            state.tripped_until = Some(Instant::now() + self.config.trip_duration);
        }
    }
    
    #[inline]
    fn count_large_in_window(
        state: &TokenState,
        now: Instant,
        sequence_window: Duration,
        large_trade_shares: f64,
    ) -> usize {
        let cutoff = now - sequence_window;
        
        state
            .large_trades
            .iter()
            .filter(|(ts, shares)| *ts > cutoff && *shares >= large_trade_shares)
            .count()
    }
}

// =============================================================================
// Book depth - separate from hot path
// =============================================================================

#[inline]
pub fn calc_liquidity_depth(side: TradeSide, levels: &[(f64, f64)], threshold: f64) -> f64 {
    let threshold_adj = if side == TradeSide::Buy {
        threshold * 1.005
    } else {
        threshold * 0.995
    };
    
    let mut total = 0.0;
    for &(price, size) in levels {
        let beyond = if side == TradeSide::Buy {
            price > threshold_adj
        } else {
            price < threshold_adj
        };
        if beyond {
            total += price * size;
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_small_trade_allows() {
        let mut guard = RiskGuard::new(RiskGuardConfig::default());
        let eval = guard.check_fast("token1", 100.0);
        assert_eq!(eval.decision, SafetyDecision::Allow);
    }

    #[test]
    fn test_single_large_allows() {
        let mut cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let eval = cb.check_fast("token1", 2000.0);
        assert_eq!(eval.decision, Decision::Allow);
        assert_eq!(eval.consecutive_large, 1);
    }

    #[test]
    fn test_two_large_triggers_fetch() {
        let mut cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        cb.check_fast("token1", 2000.0);  // First
        cb.check_fast("token1", 2000.0);
        cb.check_fast("token1", 2000.0);
        cb.check_fast("token1", 2000.0);
        let eval = cb.check_fast("token1", 2000.0);  // Second
        assert_eq!(eval.decision, Decision::FetchBook);
        assert_eq!(eval.consecutive_large, 5);
    }

    #[test]
    fn test_thin_book_blocks() {
        let mut cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let eval = cb.check_with_book("token1", 2, 50.0);  // $50 < $200 threshold
        assert_eq!(eval.decision, Decision::Block);
    }

    #[test]
    fn test_good_depth_allows() {
        let mut cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        let eval = cb.check_with_book("token1", 2, 500.0);  // $500 > $200 threshold
        assert_eq!(eval.decision, Decision::Allow);
    }

    #[test]
    fn test_tripped_token_stays_blocked() {
        let mut cb = CircuitBreaker::new(CircuitBreakerConfig {
            trip_duration: Duration::from_secs(10),
            ..Default::default()
        });
        
        // Trip it
        cb.check_with_book("token1", 2, 50.0);
        
        // Should still be blocked
        let eval = cb.check_fast("token1", 100.0);  // Even small trade
        assert_eq!(eval.decision, Decision::Block);
    }

    #[test]
    fn test_different_tokens_independent() {
        let mut cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        cb.check_fast("token1", 2000.0);
        cb.check_fast("token1", 2000.0);
        cb.check_fast("token1", 2000.0);
        cb.check_fast("token1", 2000.0);
        cb.check_fast("token1", 2000.0); 
        
        let eval = cb.check_fast("token2", 2000.0);  // token2 first large
        assert_eq!(eval.decision, Decision::Allow);
        assert_eq!(eval.consecutive_large, 1);
    }

    #[test]
    fn test_depth_calculation() {
        let asks = vec![
            (0.54, 100.0),  // At threshold
            (0.55, 200.0),  // Beyond
            (0.60, 150.0),  // Beyond
        ];
        let depth = calc_liquidity_depth(TradeSide::Buy, &asks, 0.54);
        // 0.55 * 200 + 0.60 * 150 = 110 + 90 = 200
        assert!((depth - 200.0).abs() < 1.0);
    }
}