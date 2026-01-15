// src/main_mempool.rs
// Mempool-based whale copy-trading bot

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use dotenvy::dotenv;
use alloy::primitives::U256;
use futures::{SinkExt, StreamExt};
use memchr::memmem;
use once_cell::sync::Lazy;
use rand::Rng;
use pm_whale_follower::{ApiCreds, OrderArgs, RustClobClient, PreparedCreds, OrderResponse};
use pm_whale_follower::settings::{
    Config, CLOB_API_BASE, CSV_FILE, ORDER_REPLY_TIMEOUT, WS_RECONNECT_DELAY, WS_PING_TIMEOUT,
    BOOK_REQ_TIMEOUT, MIN_WHALE_SHARES_TO_COPY, MIN_SHARE_COUNT, MIN_CASH_VALUE,
    SCALING_RATIO, USE_PROBABILISTIC_SIZING, RESUBMIT_PRICE_INCREMENT,
    should_skip_trade, get_tier_params, get_resubmit_max_buffer,
    get_max_resubmit_attempts, should_increment_price, get_gtd_expiry_secs,
};
use pm_whale_follower::risk_guard::{RiskGuard, RiskGuardConfig, SafetyDecision, SafetyEvaluation};
use pm_whale_follower::models::{OrderInfo, SizeType, ResubmitRequest};
use pm_whale_follower::tennis_markets;
use pm_whale_follower::soccer_markets;
use pm_whale_follower::market_cache;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

// ============================================================================
// Mempool-specific constants (not in shared config)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Buy,
    Sell,
}

fn calculate_depth_beyond(side: Side, levels: &[(f64, f64)], threshold: f64) -> f64 {
    let mut total_usd = 0.0;
    for &(price, size) in levels {
        total_usd += price * size;
        if side == Side::Buy && price >= threshold {
            break;
        } else if side == Side::Sell && price <= threshold {
            break;
        }
    }
    total_usd
}

const GAMMA_API_BASE: &str = "https://gamma-api.polymarket.com";

const WATCH_ADDRESSES: [&str; 4] = [
    "0xB768891e3130F6dF18214Ac804d4DB76c2C37730",
    "0xaB45c5A4B0c941a2F231C04C3f49182e1A254052",
    "0x125914B1d921D83367A0478f4ea6447AFf2Ee9DA",
    "0xE3f18aCc55091e2c48d883fc8C8413319d4Ab7b0",
];

// Whale patterns loaded from environment variable
// TARGET_WHALE_ADDRESS should be 40-char hex without 0x prefix
static WHALE_FILTER_LOWER: Lazy<Vec<u8>> = Lazy::new(|| {
    let addr = std::env::var("TARGET_WHALE_ADDRESS")
        .expect("TARGET_WHALE_ADDRESS env var is required (40-char hex without 0x)");
    addr.trim_start_matches("0x").to_lowercase().into_bytes()
});

static WHALE_PADDED_LOWER: Lazy<Vec<u8>> = Lazy::new(|| {
    let addr = std::env::var("TARGET_WHALE_ADDRESS")
        .expect("TARGET_WHALE_ADDRESS env var is required (40-char hex without 0x)");
    format!("000000000000000000000000{}", addr.trim_start_matches("0x").to_lowercase()).into_bytes()
});

// Pre-built SIMD finders (initialized lazily with env var values)
static WHALE_FILTER_FINDER: Lazy<memmem::Finder<'static>> =
    Lazy::new(|| {
        // Leak the Vec to get a 'static reference - this is a one-time allocation
        let bytes: &'static [u8] = Box::leak(WHALE_FILTER_LOWER.clone().into_boxed_slice());
        memmem::Finder::new(bytes)
    });
static WHALE_PADDED_FINDER: Lazy<memmem::Finder<'static>> =
    Lazy::new(|| {
        // Leak the Vec to get a 'static reference - this is a one-time allocation
        let bytes: &'static [u8] = Box::leak(WHALE_PADDED_LOWER.clone().into_boxed_slice());
        memmem::Finder::new(bytes)
    });

// ============================================================================
// Thread-local buffers (avoid allocation on hot path)
// ============================================================================

thread_local! {
    static CSV_BUF: RefCell<String> = RefCell::new(String::with_capacity(512));
    static SANITIZE_BUF: RefCell<String> = RefCell::new(String::with_capacity(128));
    static TOKEN_ID_CACHE: RefCell<HashMap<[u8; 32], String>> = RefCell::new(HashMap::with_capacity(256));
}

// ============================================================================
// Mempool-specific types (ParsedEvent differs from main.rs)
// ============================================================================

#[derive(Debug, Clone)]
struct ParsedEvent {
    pub tx_hash: String,
    #[allow(dead_code)]
    pub timestamp: DateTime<Utc>,
    pub order: OrderInfo,
}

#[derive(Debug)]
struct WorkItem {
    pub event: ParsedEvent,
    pub respond_to: oneshot::Sender<String>,
    pub is_live: Option<bool>,
}

// ============================================================================
// Order Engine (async -> sync bridge)
// ============================================================================

#[derive(Clone)]
struct OrderEngine {
    tx: mpsc::Sender<WorkItem>,
    #[allow(dead_code)]
    resubmit_tx: mpsc::UnboundedSender<ResubmitRequest>,
    enable_trading: bool,
}

impl OrderEngine {
    async fn submit(&self, evt: ParsedEvent, is_live: Option<bool>) -> String {
        if !self.enable_trading {
            return "SKIPPED_DISABLED".into();
        }

        let (resp_tx, resp_rx) = oneshot::channel();
        if let Err(e) = self.tx.try_send(WorkItem { event: evt, respond_to: resp_tx, is_live }) {
            return format!("QUEUE_ERR: {e}");
        }

        match tokio::time::timeout(ORDER_REPLY_TIMEOUT, resp_rx).await {
            Ok(Ok(msg)) => msg,
            Ok(Err(_)) => "WORKER_DROPPED".into(),
            Err(_) => "WORKER_TIMEOUT".into(),
        }
    }
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    ensure_csv()?;

    // Initialize all market caches at startup
    market_cache::init_caches();

    // Spawn background task to periodically refresh caches
    let _cache_refresh_handle = market_cache::spawn_cache_refresh_task();

    let cfg = Config::from_env().await?;

    let (client, creds) = build_worker_state(
        cfg.private_key.clone(),
        cfg.funder_address.clone(),
        ".clob_market_cache.json",
        ".clob_creds.json",
    ).await?;

    let prepared_creds = PreparedCreds::from_api_creds(&creds)?;
    let cb_config = cfg.risk_guard_config();

    let (order_tx, order_rx) = mpsc::channel(1024);
    let (resubmit_tx, resubmit_rx) = mpsc::unbounded_channel::<ResubmitRequest>();

    // Wrap client and creds in Arc for sharing with resubmitter
    let client_arc = Arc::new(client);
    let creds_arc = Arc::new(prepared_creds.clone());

    start_order_worker(order_rx, client_arc.clone(), prepared_creds, cfg.enable_trading, cfg.mock_trading, cb_config, resubmit_tx.clone());

    // Spawn async resubmitter worker
    tokio::spawn(resubmit_worker(resubmit_rx, client_arc, creds_arc));

    let order_engine = OrderEngine {
        tx: order_tx,
        resubmit_tx,
        enable_trading: cfg.enable_trading,
    };

    println!(
        "🚀 MEMPOOL TRADER | Trading: {}, Mock: {}",
        cfg.enable_trading, cfg.mock_trading
    );
    println!("   Whale: 0x{}", std::str::from_utf8(&WHALE_FILTER_LOWER).unwrap());

    loop {
        if let Err(e) = run_mempool_loop(&cfg.wss_url, &order_engine).await {
            eprintln!("⚠️ WS error: {e}. Reconnecting...");
            tokio::time::sleep(WS_RECONNECT_DELAY).await;
        }
    }
}

// ============================================================================
// Worker Setup
// ============================================================================

async fn build_worker_state(
    private_key: String,
    funder: String,
    cache_path: &str,
    creds_path: &str,
) -> Result<(RustClobClient, ApiCreds)> {
    let cache_path = cache_path.to_string();
    let creds_path = creds_path.to_string();
    let host = CLOB_API_BASE.to_string();

    tokio::task::spawn_blocking(move || -> Result<(RustClobClient, ApiCreds)> {
        let mut client = RustClobClient::new(&host, 137, &private_key, &funder)?
            .with_cache_path(&cache_path);
        let _ = client.load_cache();
        let _ = client.get_time(); // Pre-warm TLS

        let creds: ApiCreds = if Path::new(&creds_path).exists() {
            let data = std::fs::read_to_string(&creds_path)?;
            serde_json::from_str(&data)?
        } else {
            let derived = client.derive_api_key(0)?;
            std::fs::write(&creds_path, serde_json::to_string_pretty(&derived)?)?;
            derived
        };

        Ok((client, creds))
    }).await?
}

fn start_order_worker(
    rx: mpsc::Receiver<WorkItem>,
    client: Arc<RustClobClient>,
    creds: PreparedCreds,
    enable_trading: bool,
    mock_trading: bool,
    cb_config: RiskGuardConfig,
    resubmit_tx: mpsc::UnboundedSender<ResubmitRequest>,
) {
    std::thread::spawn(move || {
        let mut cb = RiskGuard::new(cb_config);
        order_worker(rx, client, creds, enable_trading, mock_trading, &mut cb, resubmit_tx);
    });
}

fn order_worker(
    mut rx: mpsc::Receiver<WorkItem>,
    client: Arc<RustClobClient>,
    creds: PreparedCreds,
    enable_trading: bool,
    mock_trading: bool,
    cb: &mut RiskGuard,
    resubmit_tx: mpsc::UnboundedSender<ResubmitRequest>,
) {
    // Clone Arc for mutable access pattern
    let mut client_mut = (*client).clone();
    while let Some(work) = rx.blocking_recv() {
        let status = process_order(&work.event.order, &mut client_mut, &creds, enable_trading, mock_trading, cb, &resubmit_tx, work.is_live);
        let _ = work.respond_to.send(status);
    }
}

// ============================================================================
// Order Processing
// ============================================================================

fn process_order(
    info: &OrderInfo,
    client: &mut RustClobClient,
    creds: &PreparedCreds,
    enable_trading: bool,
    mock_trading: bool,
    cb: &mut RiskGuard,
    resubmit_tx: &mpsc::UnboundedSender<ResubmitRequest>,
    is_live: Option<bool>,
) -> String {
    if !enable_trading { return "SKIPPED_DISABLED".into(); }
    if mock_trading { return "MOCK_ONLY".into(); }

    let side_is_buy = info.order_type.starts_with("BUY");
    let whale_shares = info.shares;
    let whale_price = info.price_per_share;

    // Skip small trades (<500 shares) - negative expected value after costs
    if should_skip_trade(whale_shares) {
        return format!("SKIPPED_SMALL (<{:.0} shares)", MIN_WHALE_SHARES_TO_COPY);
    }

    let (buffer, order_action, size_multiplier) = get_tier_params(whale_shares, side_is_buy, &info.clob_token_id);

    // Polymarket valid price range: 0.01 to 0.99 (tick size 0.01)
    let limit_price = if side_is_buy {
        (whale_price + buffer).min(0.99)
    } else {
        (whale_price - buffer).max(0.01)
    };

    // Circuit breaker check
    let eval = cb.check_fast(&info.clob_token_id, whale_shares);
    match eval.decision {
        SafetyDecision::Block => return format!("CB_BLOCKED:{}", eval.reason.as_str()),
        SafetyDecision::FetchBook => {
            let side = if side_is_buy { Side::Buy } else { Side::Sell };
            match fetch_book_depth_blocking(client, &info.clob_token_id, side, limit_price) {
                Ok(depth) => {
                    let final_eval = cb.check_with_book(&info.clob_token_id, eval.consecutive_large, depth);
                    if final_eval.decision == SafetyDecision::Block {
                        return format!("CB_BLOCKED:{}", final_eval.reason.as_str());
                    }
                }
                Err(e) => {
                    cb.trip(&info.clob_token_id);
                    return format!("CB_BOOK_FAIL:{e}");
                }
            }
        }
        SafetyDecision::Allow => {}
    }

    let (my_shares, size_type) = calculate_safe_size(whale_shares, limit_price, size_multiplier);
    if my_shares == 0.0 {
        return format!("SKIPPED_PROBABILITY ({})", size_type);
    }

    let args = OrderArgs {
        token_id: info.clob_token_id.to_string(),  // Arc<str> -> String
        price: limit_price,
        size: (my_shares * 100.0).floor() / 100.0,
        side: if side_is_buy { "BUY".into() } else { "SELL".into() },
        fee_rate_bps: None,
        nonce: Some(0),
        expiration: Some("0".into()),
        taker: None,
        order_type: Some(order_action.to_string()),
    };

    match client.create_order(args).and_then(|signed| {
        let body = signed.post_body(&creds.api_key, order_action);
        // NEVER use retry for order submission - could create duplicates!
        client.post_order_fast(body, creds)
    }) {
        Ok(resp) => {
            let status = resp.status();
            let body_text = resp.text().unwrap_or_default();

            // Check for underfill on successful FAK orders (buys only)
            // FAK orders return 200 OK even for partial fills - need to check takingAmount
            // NOTE: API returns human-readable values (e.g., "5" = 5 shares), NOT micro-units
            let mut underfill_msg: Option<String> = None;
            if status.is_success() && side_is_buy && order_action == "FAK" {
                if let Ok(order_resp) = serde_json::from_str::<OrderResponse>(&body_text) {
                    let filled_shares: f64 = order_resp.taking_amount.parse().unwrap_or(0.0);
                    let requested_shares = (my_shares * 100.0).floor() / 100.0;

                    if filled_shares < requested_shares && filled_shares > 0.0 {
                        let remaining_shares = requested_shares - filled_shares;

                        // Only resubmit if remaining is above minimum threshold
                        let min_threshold = MIN_SHARE_COUNT.max(MIN_CASH_VALUE / limit_price);
                        if remaining_shares >= min_threshold {
                            let resubmit_buffer = get_resubmit_max_buffer(whale_shares);
                            let max_price = (limit_price + resubmit_buffer).min(0.99);
                            let req = ResubmitRequest {
                                token_id: info.clob_token_id.to_string(),  // Arc<str> -> String
                                whale_price,
                                failed_price: limit_price,  // Start at same price (already filled some)
                                size: (remaining_shares * 100.0).floor() / 100.0,
                                whale_shares,
                                side_is_buy: true,
                                attempt: 1,
                                max_price,
                                cumulative_filled: filled_shares,
                                original_size: requested_shares,
                                is_live: is_live.unwrap_or(false),
                            };
                            let _ = resubmit_tx.send(req);
                            underfill_msg = Some(format!(
                                " | \x1b[33mUNDERFILL: {:.2}/{:.2} filled, resubmit {:.2}\x1b[0m",
                                filled_shares, my_shares, remaining_shares
                            ));
                        }
                    }
                }
            }

            // Check for FAK failure and queue resubmit (buys only) - zero fill case
            if status.as_u16() == 400 && body_text.contains("FAK") && side_is_buy {
                // Use tier-based max buffer (8000+ gets 0.02 for 2 retries, others get 0.01)
                let resubmit_buffer = get_resubmit_max_buffer(whale_shares);
                let max_price = (limit_price + resubmit_buffer).min(0.99);
                let rounded_size = (my_shares * 100.0).floor() / 100.0;
                let req = ResubmitRequest {
                    token_id: info.clob_token_id.to_string(),  // Arc<str> -> String
                    whale_price,
                    failed_price: limit_price,
                    size: rounded_size,
                    whale_shares,  // Pass whale size for tier-based max attempts
                    side_is_buy: true,
                    attempt: 1,
                    max_price,
                    cumulative_filled: 0.0,
                    original_size: rounded_size,
                    is_live: is_live.unwrap_or(false),
                };
                let _ = resubmit_tx.send(req);
            }

            // Format with fixed precision to avoid floating point artifacts
            // Red ANSI color for my price (limit_price)
            let red = "\x1b[31m";
            let reset = "\x1b[0m";
            let mut base = format!(
                "{} [{}] | my {:.2} @ {}{:.2}{} | whale {:.1} @ {:.2}",
                status, size_type, my_shares, red, limit_price, reset, whale_shares, whale_price
            );
            if let Some(msg) = underfill_msg {
                base.push_str(&msg);
            }
            if status.is_success() { base } else { format!("{} | {}", base, body_text) }
        }
        Err(e) => {
            let chain: Vec<_> = e.chain().map(|c| c.to_string()).collect();
            format!("EXEC_FAIL: {} | chain: {}", e, chain.join(" -> "))
        }
    }
}

fn calculate_safe_size(whale_shares: f64, price: f64, size_multiplier: f64) -> (f64, SizeType) {
    let target_scaled = whale_shares * SCALING_RATIO * size_multiplier;
    let safe_price = price.max(0.0001);
    let required_floor = (MIN_CASH_VALUE / safe_price).max(MIN_SHARE_COUNT);

    if target_scaled >= required_floor {
        return (target_scaled, SizeType::Scaled);
    }

    if !USE_PROBABILISTIC_SIZING {
        return (required_floor, SizeType::Scaled);
    }

    let probability = target_scaled / required_floor;
    let pct = (probability * 100.0) as u8;
    if rand::thread_rng().r#gen::<f64>() < probability {
        (required_floor, SizeType::ProbHit(pct))
    } else {
        (0.0, SizeType::ProbSkip(pct))
    }
}

/// Get ANSI color code based on fill percentage
#[allow(dead_code)]
fn get_fill_color(filled: f64, requested: f64) -> &'static str {
    if requested <= 0.0 { return "\x1b[31m"; }  // Red if no request
    let pct = (filled / requested) * 100.0;
    if pct < 50.0 { "\x1b[31m" }                // Red
    else if pct < 75.0 { "\x1b[38;5;208m" }     // Orange
    else if pct < 90.0 { "\x1b[33m" }           // Yellow
    else { "\x1b[32m" }                          // Green
}

fn fetch_book_depth_blocking(
    client: &RustClobClient,
    token_id: &str,
    side: Side,
    threshold: f64,
) -> Result<f64, &'static str> {
    let url = format!("{}/book?token_id={}", CLOB_API_BASE, token_id);
    let resp = client.http_client()
        .get(&url)
        .timeout(Duration::from_millis(500))
        .send()
        .map_err(|_| "NETWORK")?;

    if !resp.status().is_success() { return Err("HTTP_ERROR"); }

    let book: Value = resp.json().map_err(|_| "PARSE")?;
    let key = if side == Side::Buy { "asks" } else { "bids" };
    let levels: Vec<(f64, f64)> = book[key]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .take(10)
        .filter_map(|lvl| {
            let p: f64 = lvl["price"].as_str()?.parse().ok()?;
            let s: f64 = lvl["size"].as_str()?.parse().ok()?;
            Some((p, s))
        })
        .collect();

    Ok(calculate_depth_beyond(side, &levels, threshold))
}

// ============================================================================
// Mempool WebSocket Loop
// ============================================================================

async fn run_mempool_loop(wss_url: &str, order_engine: &OrderEngine) -> Result<()> {
    let (mut ws, _) = connect_async(wss_url).await?;

    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_subscribe",
        "params": ["alchemy_pendingTransactions", {
            "toAddress": WATCH_ADDRESSES,
            "hashesOnly": false
        }]
    }).to_string();

    println!("🔌 Connected. Subscribing to mempool...");
    ws.send(Message::Text(payload)).await?;

    let http_client = reqwest::Client::builder().no_proxy().build()?;
    let mut sub_id: Option<String> = None;

    loop {
        let msg = tokio::time::timeout(WS_PING_TIMEOUT, ws.next()).await;

        match msg {
            Ok(Some(Ok(Message::Text(text)))) => {
                let text_bytes = text.as_bytes();

                // Fast path: check for whale before parsing
                if !contains_whale(text_bytes) {
                    if sub_id.is_none() && text.contains("\"result\"") {
                        if let Some(id) = extract_subscription_id(&text) {
                            sub_id = Some(id.to_string());
                            println!("✅ Subscribed: {}", id);
                        }
                    }
                    continue;
                }

                // Whale detected - timestamp immediately
                let timestamp = Utc::now();

                if let Some(evt) = parse_mempool_tx(&text, &sub_id, timestamp) {
                    let engine = order_engine.clone();
                    let client = http_client.clone();
                    tokio::spawn(async move {
                        handle_event(evt, &engine, &client).await;
                    });
                }
            }
            Ok(Some(Ok(Message::Binary(bin)))) => {
                if let Ok(text) = String::from_utf8(bin) {
                    let text_bytes = text.as_bytes();
                    if !contains_whale(text_bytes) { continue; }

                    let timestamp = Utc::now();
                    if let Some(evt) = parse_mempool_tx(&text, &sub_id, timestamp) {
                        let engine = order_engine.clone();
                        let client = http_client.clone();
                        tokio::spawn(async move {
                            handle_event(evt, &engine, &client).await;
                        });
                    }
                }
            }
            Ok(Some(Ok(Message::Ping(d)))) => {
                ws.send(Message::Pong(d)).await?;
            }
            Ok(Some(Ok(Message::Close(f)))) => return Err(anyhow!("WS closed: {:?}", f)),
            Ok(None) => return Err(anyhow!("WS stream ended")),
            Err(_) => return Err(anyhow!("WS timeout")),
            _ => {}
        }
    }
}

// ============================================================================
// Mempool Parsing
// ============================================================================

#[inline(always)]
fn contains_whale(haystack: &[u8]) -> bool {
    if WHALE_FILTER_FINDER.find(haystack).is_some() {
        return true;
    }
    contains_bytes_ignore_case_slow(haystack, &WHALE_FILTER_LOWER)
}

#[inline(always)]
fn find_whale_padded(haystack: &[u8]) -> Option<usize> {
    if let Some(pos) = WHALE_PADDED_FINDER.find(haystack) {
        return Some(pos);
    }
    find_bytes_ignore_case_slow(haystack, &WHALE_PADDED_LOWER)
}

#[inline(never)]
fn contains_bytes_ignore_case_slow(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.len() > haystack.len() { return false; }
    haystack.windows(needle.len()).any(|w| {
        w.iter().zip(needle.iter()).all(|(h, n)| h.to_ascii_lowercase() == *n)
    })
}

#[inline(never)]
fn find_bytes_ignore_case_slow(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.len() > haystack.len() { return None; }
    haystack.windows(needle.len()).position(|w| {
        w.iter().zip(needle.iter()).all(|(h, n)| h.to_ascii_lowercase() == *n)
    })
}

#[inline]
fn extract_subscription_id(text: &str) -> Option<&str> {
    let idx = text.find("\"result\":\"")?;
    let start = idx + 10;
    let rest = text.get(start..)?;
    let end = rest.find('"')?;
    Some(&rest[..end])
}

fn parse_mempool_tx(text: &str, sub_id: &Option<String>, timestamp: DateTime<Utc>) -> Option<ParsedEvent> {
    let val: Value = serde_json::from_str(text).ok()?;
    let params = val.get("params")?;

    let msg_sub = params.get("subscription")?.as_str()?;
    if sub_id.as_deref() != Some(msg_sub) {
        return None;
    }

    let result = params.get("result")?;
    let tx_hash = result.get("hash")?.as_str()?;
    let input = result.get("input")?.as_str()?;
    let input_bytes = input.as_bytes();

    let whale_pos = find_whale_padded(input_bytes)?;

    // Whale is always maker, base offset is whale position
    let base = whale_pos;

    if input.len() < base + 640 {
        return None;
    }

    let token_id_hex = &input[base + 192..base + 256];
    let maker_amt = parse_hex_u64(&input[base + 256..base + 320]);
    let taker_amt = parse_hex_u64(&input[base + 320..base + 384]);
    let side_val = parse_hex_u64(&input[base + 576..base + 640]);

    let side_str = if side_val == 0 { "BUY" } else { "SELL" };
    let (usd, shares) = if side_val == 0 {
        (maker_amt as f64 / 1e6, taker_amt as f64 / 1e6)
    } else {
        (taker_amt as f64 / 1e6, maker_amt as f64 / 1e6)
    };

    if shares <= 0.0 { return None; }

    let price = usd / shares;
    let token_id_decimal = hex_to_decimal_cached(token_id_hex);

    Some(ParsedEvent {
        tx_hash: tx_hash.to_string(),
        timestamp,
        order: OrderInfo {
            order_type: format!("{}_FILL", side_str),
            clob_token_id: token_id_decimal.into(),
            usd_value: usd,
            shares,
            price_per_share: price,
        },
    })
}

#[inline(always)]
fn parse_hex_u64(s: &str) -> u64 {
    let trimmed = s.trim_start_matches('0');
    if trimmed.is_empty() { 0 } else { u64::from_str_radix(trimmed, 16).unwrap_or(0) }
}

fn hex_to_decimal_cached(hex: &str) -> String {
    if hex.len() <= 32 {
        let trimmed = hex.trim_start_matches('0');
        if trimmed.is_empty() { return "0".to_string(); }
        if let Ok(v) = u128::from_str_radix(trimmed, 16) {
            return v.to_string();
        }
    }

    if hex.len() == 64 {
        let mut bytes = [0u8; 32];
        if hex_decode_32(hex.as_bytes(), &mut bytes) {
            return TOKEN_ID_CACHE.with(|cache| {
                let mut cache = cache.borrow_mut();
                if let Some(cached) = cache.get(&bytes) {
                    return cached.clone();
                }
                let val = U256::from_be_slice(&bytes);
                let s = val.to_string();
                cache.insert(bytes, s.clone());
                s
            });
        }
    }
    String::new()
}

static HEX_NIBBLE_LUT: [u8; 256] = {
    let mut lut = [255u8; 256];
    let mut i = b'0';
    while i <= b'9' {
        lut[i as usize] = i - b'0';
        i += 1;
    }
    let mut i = b'a';
    while i <= b'f' {
        lut[i as usize] = i - b'a' + 10;
        i += 1;
    }
    let mut i = b'A';
    while i <= b'F' {
        lut[i as usize] = i - b'A' + 10;
        i += 1;
    }
    lut
};

#[inline(always)]
fn hex_nibble(b: u8) -> Option<u8> {
    let val = HEX_NIBBLE_LUT[b as usize];
    if val == 255 { None } else { Some(val) }
}

#[inline]
fn hex_decode_32(hex: &[u8], out: &mut [u8; 32]) -> bool {
    if hex.len() != 64 { return false; }
    for i in 0..32 {
        let hi = hex_nibble(hex[i * 2]);
        let lo = hex_nibble(hex[i * 2 + 1]);
        match (hi, lo) {
            (Some(h), Some(l)) => out[i] = (h << 4) | l,
            _ => return false,
        }
    }
    true
}

// ============================================================================
// Event Handling & Logging
// ============================================================================

async fn handle_event(evt: ParsedEvent, order_engine: &OrderEngine, http_client: &reqwest::Client) {
    // Get is_live from cache first, fallback to API if cache miss
    let is_live = match market_cache::get_is_live(&evt.order.clob_token_id) {
        Some(v) => Some(v),
        None => fetch_is_live(&evt.order.clob_token_id, http_client).await,
    };

    let status = order_engine.submit(evt.clone(), is_live).await;

    tokio::time::sleep(Duration::from_secs_f32(2.8)).await;

    let bests = fetch_best_book(&evt.order.clob_token_id, &evt.order.order_type, http_client).await;
    let ((bp, bs), (sp, ss)) = bests.unwrap_or_else(|| (("N/A".into(), "N/A".into()), ("N/A".into(), "N/A".into())));

    let is_live = is_live.unwrap_or(false);

    // Color best price in red (my price is already colored in process_order)
    let red = "\x1b[31m";
    let reset = "\x1b[0m";
    let colored_bp = format!("{}{}{}", red, bp, reset);

    let live_display = if is_live {
        format!("\x1b[34mlive: true\x1b[0m")  // Blue if live
    } else {
        "live: false".to_string()
    };

    // Dark green "(ATP)" indicator if this is an ATP market
    let atp_display = if tennis_markets::get_tennis_token_buffer(&evt.order.clob_token_id) > 0.0 {
        "\x1b[32m(ATP)\x1b[0m "  // Dark green
    } else {
        ""
    };

    // Cyan "(L1)" indicator if this is a Ligue 1 market
    let ligue1_display = if soccer_markets::get_soccer_token_buffer(&evt.order.clob_token_id) > 0.0 {
        "\x1b[36m(L1)\x1b[0m "  // Cyan
    } else {
        ""
    };

    println!(
        "⚡ [MEMPOOL] {}{}{} | ${:.0} | {} | best: {} @ {} | 2nd: {} @ {} | {}",
        atp_display, ligue1_display, evt.order.order_type, evt.order.usd_value, status, colored_bp, bs, sp, ss, live_display
    );

    let ts: DateTime<Utc> = Utc::now();
    let row = CSV_BUF.with(|buf| {
        SANITIZE_BUF.with(|sbuf| {
            let mut b = buf.borrow_mut();
            let mut sb = sbuf.borrow_mut();
            sanitize_csv(&status, &mut sb);
            b.clear();
            let _ = write!(b,
                "{},MEMPOOL,{},{:.2},{:.6},{:.4},{},{},{},{},{},{},{},{}",
                ts.format("%Y-%m-%d %H:%M:%S%.3f"),
                evt.order.clob_token_id, evt.order.usd_value,
                evt.order.shares, evt.order.price_per_share, evt.order.order_type,
                sb, bp, bs, sp, ss, evt.tx_hash, is_live
            );
            b.clone()
        })
    });
    let _ = tokio::task::spawn_blocking(move || append_csv_row(row)).await;
}

async fn fetch_best_book(token_id: &str, order_type: &str, client: &reqwest::Client) -> Option<((String, String), (String, String))> {
    let url = format!("{}/book?token_id={}", CLOB_API_BASE, token_id);
    let resp = client.get(&url).timeout(BOOK_REQ_TIMEOUT).send().await.ok()?;
    if !resp.status().is_success() { return None; }

    let val: Value = resp.json().await.ok()?;
    let key = if order_type.starts_with("BUY") { "asks" } else { "bids" };
    let entries = val.get(key)?.as_array()?;

    let is_buy = order_type.starts_with("BUY");

    let (best, second): (Option<(&Value, f64)>, Option<(&Value, f64)>) =
        entries.iter().fold((None, None), |(best, second), entry| {
            let price: f64 = match entry.get("price").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()) {
                Some(p) => p,
                None => return (best, second),
            };

            let better = |candidate: f64, current: f64| {
                if is_buy { candidate < current } else { candidate > current }
            };

            match best {
                Some((_, bp)) if better(price, bp) => (Some((entry, price)), best),
                Some((_, _bp)) => {
                    let new_second = match second {
                        Some((_, sp)) if better(price, sp) => Some((entry, price)),
                        None => Some((entry, price)),
                        _ => second,
                    };
                    (best, new_second)
                }
                None => (Some((entry, price)), second),
            }
        });

    let b = best?.0;
    let best_price = b.get("price")?.to_string();
    let best_size = b.get("size")?.to_string();

    let (second_price, second_size) = second
        .and_then(|(e, _)| {
            let p = e.get("price")?.to_string();
            let s = e.get("size")?.to_string();
            Some((p, s))
        })
        .unwrap_or_else(|| ("N/A".into(), "N/A".into()));

    Some(((best_price, best_size), (second_price, second_size)))
}

async fn fetch_is_live(token_id: &str, client: &reqwest::Client) -> Option<bool> {
    // Fetch market info to get slug
    let market_url = format!("{}/markets?clob_token_ids={}", GAMMA_API_BASE, token_id);
    let resp = client.get(&market_url).timeout(Duration::from_secs(2)).send().await.ok()?;
    let val: Value = resp.json().await.ok()?;
    let slug = val.get(0)?.get("slug")?.as_str()?.to_string();

    // Fetch live status from events API
    let event_url = format!("{}/events/slug/{}", GAMMA_API_BASE, slug);
    let resp = client.get(&event_url).timeout(Duration::from_secs(2)).send().await.ok()?;
    let val: Value = resp.json().await.ok()?;

    Some(val["live"].as_bool().unwrap_or(false))
}

// ============================================================================
// Resubmitter Worker (handles FAK failures with price escalation)
// ============================================================================

async fn resubmit_worker(
    mut rx: mpsc::UnboundedReceiver<ResubmitRequest>,
    client: Arc<RustClobClient>,
    creds: Arc<PreparedCreds>,
) {
    println!("🔄 Resubmitter worker started");

    while let Some(req) = rx.recv().await {
        // Calculate max attempts and is_last_attempt FIRST (needed for max_price check)
        let max_attempts = get_max_resubmit_attempts(req.whale_shares);
        let is_last_attempt = req.attempt >= max_attempts;

        // Calculate increment: chase only if should_increment_price returns true
        let increment = if should_increment_price(req.whale_shares, req.attempt) {
            RESUBMIT_PRICE_INCREMENT
        } else {
            0.0  // Flat retry
        };
        let new_price = if req.side_is_buy {
            (req.failed_price + increment).min(0.99)
        } else {
            (req.failed_price - increment).max(0.01)
        };

        // Check if we've exceeded max buffer (skip check for GTD - last attempt always goes through)
        if !is_last_attempt && req.side_is_buy && new_price > req.max_price {
            let fill_pct = if req.original_size > 0.0 { (req.cumulative_filled / req.original_size) * 100.0 } else { 0.0 };
            println!(
                "🔄 Resubmit ABORT: attempt {} price {:.2} > max {:.2} | filled {:.2}/{:.2} ({:.0}%)",
                req.attempt, new_price, req.max_price, req.cumulative_filled, req.original_size, fill_pct
            );
            continue;
        }

        // Clone what we need for spawn_blocking
        let client_clone = (*client).clone();
        let creds_clone = (*creds).clone();
        let token_id = req.token_id.clone();
        let size = req.size;
        let attempt = req.attempt;
        let whale_price = req.whale_price;
        let max_price = req.max_price;
        let is_live = req.is_live;

        // Submit order: FAK for early attempts, GTD with expiry for last attempt
        let result = tokio::task::spawn_blocking(move || {
            submit_resubmit_order_sync(&client_clone, &creds_clone, &token_id, new_price, size, is_live, is_last_attempt)
        }).await;

        match result {
            Ok(Ok((true, _, filled_this_attempt))) => {
                if is_last_attempt {
                    // GTD order placed on book - we don't know fill amount yet
                    println!(
                        "\x1b[32m🔄 Resubmit GTD SUBMITTED: attempt {} @ {:.2} | size {:.2} | prior filled {:.2}/{:.2}\x1b[0m",
                        attempt, new_price, size, req.cumulative_filled, req.original_size
                    );
                } else {
                    // FAK order - check if partial fill
                    let total_filled = req.cumulative_filled + filled_this_attempt;
                    let fill_pct = if req.original_size > 0.0 { (total_filled / req.original_size) * 100.0 } else { 0.0 };
                    let remaining = size - filled_this_attempt;

                    // If partial fill, continue with remaining size
                    if remaining > 1.0 && filled_this_attempt > 0.0 {
                        println!(
                            "\x1b[33m🔄 Resubmit PARTIAL: attempt {} @ {:.2} | filled {:.2}/{:.2} ({:.0}%) | remaining {:.2}\x1b[0m",
                            attempt, new_price, total_filled, req.original_size, fill_pct, remaining
                        );
                        let next_req = ResubmitRequest {
                            token_id: req.token_id.clone(),
                            whale_price,
                            failed_price: new_price,
                            size: remaining,
                            whale_shares: req.whale_shares,
                            side_is_buy: req.side_is_buy,
                            attempt: attempt + 1,
                            max_price,
                            cumulative_filled: total_filled,
                            original_size: req.original_size,
                            is_live: req.is_live,
                        };
                        let _ = process_resubmit_chain(&client, &creds, next_req).await;
                    } else {
                        println!(
                            "\x1b[32m🔄 Resubmit SUCCESS: attempt {} @ {:.2} | filled {:.2}/{:.2} ({:.0}%)\x1b[0m",
                            attempt, new_price, total_filled, req.original_size, fill_pct
                        );
                    }
                }
            }
            Ok(Ok((false, body, filled_this_attempt))) => {
                // GTD orders either fill or expire - retrying usually not needed
                if attempt < max_attempts {
                    // Re-queue with updated price
                    let next_req = ResubmitRequest {
                        token_id: req.token_id,
                        whale_price,
                        failed_price: new_price,
                        size: req.size,
                        whale_shares: req.whale_shares,
                        side_is_buy: req.side_is_buy,
                        attempt: attempt + 1,
                        max_price,
                        cumulative_filled: req.cumulative_filled + filled_this_attempt,
                        original_size: req.original_size,
                        is_live: req.is_live,
                    };
                    // Process remaining attempts inline (no delay for speed)
                    let next_increment = if should_increment_price(req.whale_shares, attempt + 1) {
                        RESUBMIT_PRICE_INCREMENT
                    } else {
                        0.0
                    };
                    println!(
                        "🔄 Resubmit attempt {} failed (FAK), retrying immediately @ {:.2} (max: {})",
                        attempt, new_price + next_increment, max_attempts
                    );
                    let _ = process_resubmit_chain(
                        &client,
                        &creds,
                        next_req,
                    ).await;
                } else {
                    let total_filled = req.cumulative_filled + filled_this_attempt;
                    let fill_pct = if req.original_size > 0.0 { (total_filled / req.original_size) * 100.0 } else { 0.0 };
                    let fill_color = get_fill_color(total_filled, req.original_size);
                    let reset = "\x1b[0m";
                    println!(
                        "🔄 Resubmit FAILED: attempt {} @ {:.2} | {}filled {:.2}/{:.2} ({:.0}%){} | {}",
                        attempt, new_price, fill_color, total_filled, req.original_size, fill_pct, reset,
                        body.chars().take(80).collect::<String>()
                    );
                }
            }
            Ok(Err(e)) => {
                let fill_pct = if req.original_size > 0.0 { (req.cumulative_filled / req.original_size) * 100.0 } else { 0.0 };
                let fill_color = get_fill_color(req.cumulative_filled, req.original_size);
                let reset = "\x1b[0m";
                println!(
                    "🔄 Resubmit ERROR: attempt {} | {}filled {:.2}/{:.2} ({:.0}%){} | {}",
                    attempt, fill_color, req.cumulative_filled, req.original_size, fill_pct, reset, e
                );
            }
            Err(e) => {
                let fill_pct = if req.original_size > 0.0 { (req.cumulative_filled / req.original_size) * 100.0 } else { 0.0 };
                let fill_color = get_fill_color(req.cumulative_filled, req.original_size);
                let reset = "\x1b[0m";
                println!(
                    "🔄 Resubmit TASK ERROR: {}filled {:.2}/{:.2} ({:.0}%){} | {}",
                    fill_color, req.cumulative_filled, req.original_size, fill_pct, reset, e
                );
            }
        }
    }
}

async fn process_resubmit_chain(
    client: &Arc<RustClobClient>,
    creds: &Arc<PreparedCreds>,
    mut req: ResubmitRequest,
) {
    let max_attempts = get_max_resubmit_attempts(req.whale_shares);

    while req.attempt <= max_attempts {
        // Calculate is_last_attempt FIRST (needed for max_price check and order type)
        let attempt = req.attempt;
        let is_last_attempt = attempt >= max_attempts;

        // Calculate increment: chase only if should_increment_price returns true
        let increment = if should_increment_price(req.whale_shares, req.attempt) {
            RESUBMIT_PRICE_INCREMENT
        } else {
            0.0  // Flat retry
        };
        let new_price = if req.side_is_buy {
            (req.failed_price + increment).min(0.99)
        } else {
            (req.failed_price - increment).max(0.01)
        };

        // Check if we've exceeded max buffer (skip check for GTD - last attempt always goes through)
        if !is_last_attempt && req.side_is_buy && new_price > req.max_price {
            let fill_pct = if req.original_size > 0.0 { (req.cumulative_filled / req.original_size) * 100.0 } else { 0.0 };
            println!(
                "🔄 Resubmit chain ABORT: attempt {} price {:.2} > max {:.2} | filled {:.2}/{:.2} ({:.0}%)",
                req.attempt, new_price, req.max_price, req.cumulative_filled, req.original_size, fill_pct
            );
            return;
        }

        let client_clone = (*client).clone();
        let creds_clone = (*creds).clone();
        let token_id = req.token_id.clone();
        let size = req.size;
        let is_live = req.is_live;

        // Submit order: FAK for early attempts, GTD with expiry for last attempt
        let result = tokio::task::spawn_blocking(move || {
            submit_resubmit_order_sync(&client_clone, &creds_clone, &token_id, new_price, size, is_live, is_last_attempt)
        }).await;

        match result {
            Ok(Ok((true, _, filled_this_attempt))) => {
                if is_last_attempt {
                    // GTD order placed on book - we don't know fill amount yet
                    println!(
                        "\x1b[32m🔄 Resubmit chain GTD SUBMITTED: attempt {} @ {:.2} | size {:.2} | prior filled {:.2}/{:.2}\x1b[0m",
                        attempt, new_price, size, req.cumulative_filled, req.original_size
                    );
                    return;
                } else {
                    // FAK order - check if partial fill
                    let total_filled = req.cumulative_filled + filled_this_attempt;
                    let fill_pct = if req.original_size > 0.0 { (total_filled / req.original_size) * 100.0 } else { 0.0 };
                    let remaining = size - filled_this_attempt;

                    // If partial fill, continue with remaining size
                    if remaining > 1.0 && filled_this_attempt > 0.0 {
                        println!(
                            "\x1b[33m🔄 Resubmit chain PARTIAL: attempt {} @ {:.2} | filled {:.2}/{:.2} ({:.0}%) | remaining {:.2}\x1b[0m",
                            attempt, new_price, total_filled, req.original_size, fill_pct, remaining
                        );
                        req.cumulative_filled = total_filled;
                        req.size = remaining;
                        req.failed_price = new_price;
                        req.attempt += 1;
                        continue;
                    } else {
                        println!(
                            "\x1b[32m🔄 Resubmit chain SUCCESS: attempt {} @ {:.2} | filled {:.2}/{:.2} ({:.0}%)\x1b[0m",
                            attempt, new_price, total_filled, req.original_size, fill_pct
                        );
                        return;
                    }
                }
            }
            Ok(Ok((false, body, filled_this_attempt))) if body.contains("FAK") && attempt < max_attempts => {
                req.cumulative_filled += filled_this_attempt;
                req.failed_price = new_price;
                req.attempt += 1;
                // No delay - continue immediately
                continue;
            }
            Ok(Ok((false, body, filled_this_attempt))) => {
                let total_filled = req.cumulative_filled + filled_this_attempt;
                let fill_pct = if req.original_size > 0.0 { (total_filled / req.original_size) * 100.0 } else { 0.0 };
                let fill_color = get_fill_color(total_filled, req.original_size);
                let reset = "\x1b[0m";
                println!(
                    "🔄 Resubmit chain FAILED: attempt {}/{} @ {:.2} | {}filled {:.2}/{:.2} ({:.0}%){} | {}",
                    attempt, max_attempts, new_price, fill_color, total_filled, req.original_size, fill_pct, reset,
                    body.chars().take(80).collect::<String>()
                );
                return;
            }
            Ok(Err(e)) => {
                let fill_pct = if req.original_size > 0.0 { (req.cumulative_filled / req.original_size) * 100.0 } else { 0.0 };
                let fill_color = get_fill_color(req.cumulative_filled, req.original_size);
                let reset = "\x1b[0m";
                println!(
                    "🔄 Resubmit chain ERROR: attempt {} | {}filled {:.2}/{:.2} ({:.0}%){} | {}",
                    attempt, fill_color, req.cumulative_filled, req.original_size, fill_pct, reset, e
                );
                return;
            }
            Err(e) => {
                let fill_pct = if req.original_size > 0.0 { (req.cumulative_filled / req.original_size) * 100.0 } else { 0.0 };
                let fill_color = get_fill_color(req.cumulative_filled, req.original_size);
                let reset = "\x1b[0m";
                println!(
                    "🔄 Resubmit chain TASK ERROR: {}filled {:.2}/{:.2} ({:.0}%){} | {}",
                    fill_color, req.cumulative_filled, req.original_size, fill_pct, reset, e
                );
                return;
            }
        }
    }
}

/// Synchronous order submission for resubmits (called via spawn_blocking)
/// Now uses GTD with dynamic expiry based on market liveness (only on last attempt)
/// Returns (success, body_text, filled_shares)
fn submit_resubmit_order_sync(
    client: &RustClobClient,
    creds: &PreparedCreds,
    token_id: &str,
    price: f64,
    size: f64,
    is_live: bool,
    is_last_attempt: bool,
) -> anyhow::Result<(bool, String, f64)> {
    use std::time::{SystemTime, UNIX_EPOCH};
    use pm_whale_follower::settings::get_gtd_expiry_secs;

    let mut client = client.clone();

    // Only use GTD with expiry on the LAST attempt; earlier attempts use FAK
    let (expiration, order_type) = if is_last_attempt {
        let expiry_secs = get_gtd_expiry_secs(is_live);
        let expiry_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() + expiry_secs;
        (Some(expiry_timestamp.to_string()), "GTD")
    } else {
        (None, "FAK")
    };

    // Round to micro-units (6 decimals) then back to avoid floating-point truncation issues
    // e.g., 40.80 stored as 40.7999999... would truncate to 40799999 instead of 40800000
    let price_micro = (price * 1_000_000.0).round() as i64;
    let size_micro = (size * 1_000_000.0).round() as i64;
    let rounded_price = price_micro as f64 / 1_000_000.0;
    let rounded_size = size_micro as f64 / 1_000_000.0;

    let args = OrderArgs {
        token_id: token_id.to_string(),
        price: rounded_price,
        size: rounded_size,
        side: "BUY".into(),
        fee_rate_bps: None,
        nonce: Some(0),
        expiration,
        taker: None,
        order_type: Some(order_type.to_string()),
    };

    let signed = client.create_order(args)?;
    let body = signed.post_body(&creds.api_key, order_type);
    let resp = client.post_order_fast(body, creds)?;

    let status = resp.status();
    let body_text = resp.text().unwrap_or_default();

    // Parse filled amount from successful responses
    // GTD orders return taking_amount=0 since they're placed on book, not immediately filled
    // For GTD, return 0 - caller handles GTD success messaging separately
    let filled_shares = if status.is_success() && order_type == "FAK" {
        serde_json::from_str::<OrderResponse>(&body_text)
            .ok()
            .and_then(|r| r.taking_amount.parse::<f64>().ok())
            .unwrap_or(0.0)
    } else {
        0.0
    };

    Ok((status.is_success(), body_text, filled_shares))
}

// ============================================================================
// CSV Helpers
// ============================================================================

fn ensure_csv() -> Result<()> {
    if !Path::new(CSV_FILE).exists() {
        let mut f = File::create(CSV_FILE)?;
        writeln!(f, "timestamp,block,clob_asset_id,usd_value,shares,price_per_share,direction,order_status,best_price,best_size,second_price,second_size,tx_hash,is_live")?;
    }
    Ok(())
}

fn append_csv_row(row: String) {
    if let Ok(mut f) = OpenOptions::new().append(true).create(true).open(CSV_FILE) {
        let _ = writeln!(f, "{}", row);
    }
}

#[inline]
fn sanitize_csv(value: &str, out: &mut String) {
    out.clear();
    if !value.bytes().any(|b| b == b',' || b == b'\n' || b == b'\r') {
        out.push_str(value);
        return;
    }
    out.reserve(value.len());
    for &b in value.as_bytes() {
        out.push(match b { b',' => ';', b'\n' | b'\r' => ' ', _ => b as char });
    }
}