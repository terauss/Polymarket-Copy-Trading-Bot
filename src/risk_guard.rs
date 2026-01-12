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
pub enum SafetyReason {
    Tripped { secs_left: u32 },
    SmallTrade,
    SeqOk { count: u8 },
    SeqNeedBook { count: u8 },
    Trap { seq: u8, depth_usd: u16 },
    DepthOk { seq: u8, depth_usd: u16 },
    BookFetchFailed,
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
            trip_duration: Duration::from_secs(60 * 60 * 5),
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
// Risk Guard
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
    
    pub fn trip(&mut self, token_id: &str) {
        if let Some(state) = self.tokens.get_mut(token_id) {
            state.tripped_until =
                Some(std::time::Instant::now() + self.config.trip_duration);
        }
    }


    #[inline]
    pub fn check_fast(&mut self, token_id: &str, whale_shares: f64) -> SafetyEvaluation {
        let now = Instant::now();
        let state = self.tokens.entry(token_id.to_string()).or_default();

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

        if whale_shares < self.config.large_trade_shares {
            return SafetyEvaluation {
                decision: SafetyDecision::Allow,
                reason: SafetyReason::SmallTrade,
                consecutive_large: 0,
            };
        }

        let consecutive = Self::count_large_in_window(
            state,
            now,
            self.config.sequence_window,
            self.config.large_trade_shares,
        ) + 1;

        state.large_trades.push((now, whale_shares));

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
        let depth_u16 = depth_beyond_usd.min(65535.0) as u16;

        if depth_beyond_usd < self.config.min_depth_beyond_usd {
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

    #[inline]
    fn count_large_in_window(
        state: &TokenState,
        now: Instant,
        window: Duration,
        threshold: f64,
    ) -> usize {
        let cutoff = now - window;
        state
            .large_trades
            .iter()
            .filter(|(ts, shares)| *ts > cutoff && *shares >= threshold)
            .count()
    }
}

// =============================================================================
// Book depth
// =============================================================================

#[inline]
pub fn calc_liquidity_depth(side: TradeSide, levels: &[(f64, f64)], threshold: f64) -> f64 {
    let threshold_adj = if side == TradeSide::Buy {
        threshold * 1.005
    } else {
        threshold * 0.995
    };

    levels
        .iter()
        .filter(|(price, _)| {
            if side == TradeSide::Buy {
                *price > threshold_adj
            } else {
                *price < threshold_adj
            }
        })
        .map(|(price, size)| price * size)
        .sum()
}
