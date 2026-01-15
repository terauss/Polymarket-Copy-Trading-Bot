/// PM Whale Follower - Main entry point
/// Monitors blockchain for whale trades and executes copy trades

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use dotenvy::dotenv;
use alloy::primitives::U256;
use futures::{SinkExt, StreamExt};
use rand::Rng;
use pm_whale_follower::{ApiCreds, OrderArgs, RustClobClient, PreparedCreds, OrderResponse};
use pm_whale_follower::settings::Config;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

mod models;

use pm_whale_follower::risk_guard::{RiskGuard, RiskGuardConfig, SafetyDecision, TradeSide, calc_liquidity_depth};
use pm_whale_follower::settings::*;
use pm_whale_follower::market_cache;
use pm_whale_follower::tennis_markets;
use pm_whale_follower::soccer_markets;
use models::*;
use std::sync::Arc;

const GAMMA_API_BASE: &str = "https://gamma-api.polymarket.com";

// ============================================================================
// Thread-local buffers 
// ============================================================================

thread_local! {
    static CSV_BUF: RefCell<String> = RefCell::new(String::with_capacity(512));
    static SANITIZE_BUF: RefCell<String> = RefCell::new(String::with_capacity(128));
    static TOKEN_ID_CACHE: RefCell<HashMap<[u8; 32], Arc<str>>> = RefCell::new(HashMap::with_capacity(256));
}

// ============================================================================
// Order Engine 
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

    // Initialize market data caches
    market_cache::init_caches();

    // Start background cache refresh task
    let _cache_refresh_handle = market_cache::spawn_cache_refresh_task();

    let cfg = Config::from_env().await?;
    
    let (client, creds) = build_worker_state(
        cfg.private_key.clone(),
        cfg.funder_address.clone(),
        ".clob_market_cache.json",
        ".clob_creds.json",
    ).await?;
    
    let prepared_creds = PreparedCreds::from_api_creds(&creds)?;
    let risk_config = cfg.risk_guard_config();

    let (order_tx, order_rx) = mpsc::channel(1024);
    let (resubmit_tx, resubmit_rx) = mpsc::unbounded_channel::<ResubmitRequest>();

    let client_arc = Arc::new(client);
    let creds_arc = Arc::new(prepared_creds.clone());

    start_order_worker(order_rx, client_arc.clone(), prepared_creds, cfg.enable_trading, cfg.mock_trading, risk_config, resubmit_tx.clone());

    tokio::spawn(resubmit_worker(resubmit_rx, client_arc, creds_arc));

    let order_engine = OrderEngine {
        tx: order_tx,
        resubmit_tx,
        enable_trading: cfg.enable_trading,
    };

    println!(
        "🚀 Starting trader. Trading: {}, Mock: {}",
        cfg.enable_trading, cfg.mock_trading
    );

    loop {
        if let Err(e) = run_ws_loop(&cfg.wss_url, &order_engine).await {
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
 
}

fn start_order_worker(
    rx: mpsc::Receiver<WorkItem>,
    client: Arc<RustClobClient>,
    creds: PreparedCreds,
    enable_trading: bool,
    mock_trading: bool,
    risk_config: RiskGuardConfig,
    resubmit_tx: mpsc::UnboundedSender<ResubmitRequest>,
) {
    
}

fn order_worker(
    mut rx: mpsc::Receiver<WorkItem>,
    client: Arc<RustClobClient>,
    creds: PreparedCreds,
    enable_trading: bool,
    mock_trading: bool,
    guard: &mut RiskGuard,
    resubmit_tx: mpsc::UnboundedSender<ResubmitRequest>,
) {
    
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
    guard: &mut RiskGuard,
    resubmit_tx: &mpsc::UnboundedSender<ResubmitRequest>,
    is_live: Option<bool>,
) -> String {
   
}

fn calculate_safe_size(whale_shares: f64, price: f64, size_multiplier: f64) -> (f64, SizeType) {
    
}

/// Get ANSI color code based on fill percentage
fn get_fill_color(filled: f64, requested: f64) -> &'static str {
    if requested <= 0.0 { return "\x1b[31m"; }  // Red if no request
    let pct = (filled / requested) * 100.0;
    if pct < 50.0 { "\x1b[31m" }                // Red
    else if pct < 75.0 { "\x1b[38;5;208m" }     // Orange
    else if pct < 90.0 { "\x1b[33m" }           // Yellow
    else { "\x1b[32m" }                          // Green
}

/// Get ANSI color code based on whale share count (gradient from small to large)
fn get_whale_size_color(shares: f64) -> &'static str {
    if shares < 500.0 { "\x1b[90m" }              // Gray (very small)
    else if shares < 1000.0 { "\x1b[36m" }        // Cyan (small)
    else if shares < 2000.0 { "\x1b[34m" }        // Blue (medium-small)
    else if shares < 5000.0 { "\x1b[32m" }        // Green (medium)
    else if shares < 8000.0 { "\x1b[33m" }        // Yellow (medium-large)
    else if shares < 15000.0 { "\x1b[38;5;208m" } // Orange (large)
    else { "\x1b[35m" }                           // Magenta (huge)
}

fn fetch_book_depth_blocking(
    client: &RustClobClient,
    token_id: &str,
    side: TradeSide,
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
    let key = if side == TradeSide::Buy { "asks" } else { "bids" };

    // Stack array instead of Vec - avoids heap allocation for max 10 items
    let mut levels: [(f64, f64); 10] = [(0.0, 0.0); 10];
    let mut count = 0;
    if let Some(arr) = book[key].as_array() {
        for lvl in arr.iter().take(10) {
            if let (Some(p), Some(s)) = (
                lvl["price"].as_str().and_then(|s| s.parse().ok()),
                lvl["size"].as_str().and_then(|s| s.parse().ok()),
            ) {
                levels[count] = (p, s);
                count += 1;
            }
        }
    }

    Ok(calc_liquidity_depth(side, &levels[..count], threshold))
}

// ============================================================================
// WebSocket Loop
// ============================================================================

async fn run_ws_loop(wss_url: &str, order_engine: &OrderEngine) -> Result<()> {
    let (mut ws, _) = connect_async(wss_url).await?;

    let sub = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "eth_subscribe",
        "params": ["logs", {
            "address": MONITORED_ADDRESSES,
            "topics": [[ORDERS_FILLED_EVENT_SIGNATURE], Value::Null, TARGET_TOPIC_HEX.as_str()]
        }]
    }).to_string();

    println!("🔌 Connected. Subscribing...");
    ws.send(Message::Text(sub)).await?;

    let http_client = reqwest::Client::builder().no_proxy().build()?;

    loop {
        let msg = tokio::time::timeout(WS_PING_TIMEOUT, ws.next()).await
            .map_err(|_| anyhow!("WS timeout"))?
            .ok_or_else(|| anyhow!("WS closed"))??;

        match msg {
            Message::Text(text) => {
                if let Some(evt) = parse_event(text) {
                    let engine = order_engine.clone();
                    let client = http_client.clone();
                    tokio::spawn(async move { handle_event(evt, &engine, &client).await });
                }
            }
            Message::Binary(bin) => {
                if let Ok(text) = String::from_utf8(bin) {
                    if let Some(evt) = parse_event(text) {
                        let engine = order_engine.clone();
                        let client = http_client.clone();
                        tokio::spawn(async move { handle_event(evt, &engine, &client).await });
                    }
                }
            }
            Message::Ping(d) => { ws.send(Message::Pong(d)).await?; }
            Message::Close(f) => return Err(anyhow!("WS closed: {:?}", f)),
            _ => {}
        }
    }
}

async fn handle_event(evt: ParsedEvent, order_engine: &OrderEngine, http_client: &reqwest::Client) {
    // Check live status from cache, fallback to API lookup
    let is_live = match market_cache::get_is_live(&evt.order.clob_token_id) {
        Some(v) => Some(v),
        None => fetch_is_live(&evt.order.clob_token_id, http_client).await,
    };

    let status = order_engine.submit(evt.clone(), is_live).await;

    tokio::time::sleep(Duration::from_secs_f32(2.8)).await;

    // Fetch order book for post-trade logging
    let bests = fetch_best_book(&evt.order.clob_token_id, &evt.order.order_type, http_client).await;
    let ((bp, bs), (sp, ss)) = bests.unwrap_or_else(|| (("N/A".into(), "N/A".into()), ("N/A".into(), "N/A".into())));
    let is_live = is_live.unwrap_or(false);

    // Highlight best price in bright pink
    let pink = "\x1b[38;5;199m";
    let reset = "\x1b[0m";
    let colored_bp = format!("{}{}{}", pink, bp, reset);

    let live_display = if is_live {
        format!("\x1b[34mlive: true\x1b[0m")
    } else {
        "live: false".to_string()
    };

    // Tennis market indicator (green)
    let tennis_display = if tennis_markets::get_tennis_token_buffer(&evt.order.clob_token_id) > 0.0 {
        "\x1b[32m(TENNIS)\x1b[0m "
    } else {
        ""
    };

    // Soccer market indicator (cyan)
    let soccer_display = if soccer_markets::get_soccer_token_buffer(&evt.order.clob_token_id) > 0.0 {
        "\x1b[36m(SOCCER)\x1b[0m "
    } else {
        ""
    };

    println!(
        "⚡ [B:{}] {}{}{} | ${:.0} | {} | best: {} @ {} | 2nd: {} @ {} | {}",
        evt.block_number, tennis_display, soccer_display, evt.order.order_type, evt.order.usd_value, status, colored_bp, bs, sp, ss, live_display
    );

    let ts: DateTime<Utc> = Utc::now();
    let row = CSV_BUF.with(|buf| {
        SANITIZE_BUF.with(|sbuf| {
            let mut b = buf.borrow_mut();
            let mut sb = sbuf.borrow_mut();
            sanitize_csv(&status, &mut sb);
            b.clear();
            let _ = write!(b,
                "{},{},{},{:.2},{:.6},{:.4},{},{},{},{},{},{},{},{}",
                ts.format("%Y-%m-%d %H:%M:%S%.3f"),
                evt.block_number, evt.order.clob_token_id, evt.order.usd_value,
                evt.order.shares, evt.order.price_per_share, evt.order.order_type,
                sb, bp, bs, sp, ss, evt.tx_hash, is_live
            );
            b.clone()
        })
    });
    let _ = tokio::task::spawn_blocking(move || append_csv_row(row)).await;
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

        let client_clone = Arc::clone(&client);
        let creds_clone = Arc::clone(&creds);
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
                            token_id: req.token_id,
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
                    let next_increment = if should_increment_price(req.whale_shares, attempt + 1) {
                        RESUBMIT_PRICE_INCREMENT
                    } else {
                        0.0
                    };
                    println!(
                        "🔄 Resubmit attempt {} failed (FAK), retrying @ {:.2} (max: {})",
                        attempt, new_price + next_increment, max_attempts
                    );
                    if req.whale_shares < 1000.0 {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                    let _ = process_resubmit_chain(
                        &client,
                        &creds,
                        next_req,
                    ).await;
                } else {
                    let total_filled = req.cumulative_filled + filled_this_attempt;
                    let fill_pct = if req.original_size > 0.0 { (total_filled / req.original_size) * 100.0 } else { 0.0 };
                    let error_msg = if DEBUG_FULL_ERRORS { body.clone() } else { body.chars().take(80).collect::<String>() };
                    println!(
                        "🔄 Resubmit FAILED: attempt {} @ {:.2} | filled {:.2}/{:.2} ({:.0}%) | {}",
                        attempt, new_price, total_filled, req.original_size, fill_pct, error_msg
                    );
                }
            }
            Ok(Err(e)) => {
                let fill_pct = if req.original_size > 0.0 { (req.cumulative_filled / req.original_size) * 100.0 } else { 0.0 };
                println!(
                    "🔄 Resubmit ERROR: attempt {} | filled {:.2}/{:.2} ({:.0}%) | {}",
                    attempt, req.cumulative_filled, req.original_size, fill_pct, e
                );
            }
            Err(e) => {
                let fill_pct = if req.original_size > 0.0 { (req.cumulative_filled / req.original_size) * 100.0 } else { 0.0 };
                println!(
                    "🔄 Resubmit TASK ERROR: filled {:.2}/{:.2} ({:.0}%) | {}",
                    req.cumulative_filled, req.original_size, fill_pct, e
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
                "🔄 Resubmit chain ABORT: attempt {} price {:.2} > max {:.2} | filled {:.2}/{:.2} ({:.0}%)",
                req.attempt, new_price, req.max_price, req.cumulative_filled, req.original_size, fill_pct
            );
            return;
        }

        let client_clone = Arc::clone(&client);
        let creds_clone = Arc::clone(&creds);
        let token_id = req.token_id.clone();
        let size = req.size;
        let attempt = req.attempt;
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
                        attempt, new_price, req.size, req.cumulative_filled, req.original_size
                    );
                    return;
                } else {
                    // FAK order - check if partial fill
                    let total_filled = req.cumulative_filled + filled_this_attempt;
                    let fill_pct = if req.original_size > 0.0 { (total_filled / req.original_size) * 100.0 } else { 0.0 };
                    let remaining = req.size - filled_this_attempt;

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
                // Small trades get 50ms delay to let orderbook refresh
                if req.whale_shares < 1000.0 {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                continue;
            }
            Ok(Ok((false, body, filled_this_attempt))) => {
                let total_filled = req.cumulative_filled + filled_this_attempt;
                let fill_pct = if req.original_size > 0.0 { (total_filled / req.original_size) * 100.0 } else { 0.0 };
                let fill_color = get_fill_color(total_filled, req.original_size);
                let reset = "\x1b[0m";
                let error_msg = if DEBUG_FULL_ERRORS { body.clone() } else { body.chars().take(80).collect::<String>() };
                println!(
                    "🔄 Resubmit chain FAILED: attempt {}/{} @ {:.2} | {}filled {:.2}/{:.2} ({:.0}%){} | {}",
                    attempt, max_attempts, new_price, fill_color, total_filled, req.original_size, fill_pct, reset, error_msg
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

// ============================================================================
// Event Parsing
// ============================================================================

fn parse_event(message: String) -> Option<ParsedEvent> {
    let msg: WsMessage = serde_json::from_str(&message).ok()?;
    let result = msg.params?.result?;
    
    // just to double check! 
    if result.topics.len() < 3 { return None; }
    
    let has_target = result.topics.get(2)
        .map(|t| t.eq_ignore_ascii_case(TARGET_TOPIC_HEX.as_str()))
        .unwrap_or(false);
    if !has_target { return None; }

    let hex_data = &result.data;
    if hex_data.len() < 2 + 64 * 4 { return None; }

    let (maker_id, maker_bytes) = parse_u256_hex_slice_with_bytes(hex_data, 2, 66)?;
    let (taker_id, taker_bytes) = parse_u256_hex_slice_with_bytes(hex_data, 66, 130)?;

    let (clob_id, token_bytes, maker_amt, taker_amt, base_type) =
        if maker_id.is_zero() && !taker_id.is_zero() {
            let m = parse_u256_hex_slice(hex_data, 130, 194)?;
            let t = parse_u256_hex_slice(hex_data, 194, 258)?;
            (taker_id, taker_bytes, m, t, "BUY")
        } else if taker_id.is_zero() && !maker_id.is_zero() {
            let m = parse_u256_hex_slice(hex_data, 130, 194)?;
            let t = parse_u256_hex_slice(hex_data, 194, 258)?;
            (maker_id, maker_bytes, m, t, "SELL")
        } else {
            return None;
        };

    let shares = if base_type == "BUY" { u256_to_f64(&taker_amt)? } else { u256_to_f64(&maker_amt)? } / 1e6;
    if shares <= 0.0 { return None; }
    
    let usd = if base_type == "BUY" { u256_to_f64(&maker_amt)? } else { u256_to_f64(&taker_amt)? } / 1e6;
    let price = usd / shares;
    
    let mut order_type = base_type.to_string();
    if result.topics[0].eq_ignore_ascii_case(ORDERS_FILLED_EVENT_SIGNATURE) {
        order_type.push_str("_FILL");
    }

    Some(ParsedEvent {
        block_number: result.block_number.as_deref()
            .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or_default(),
        tx_hash: result.transaction_hash.unwrap_or_default(),
        order: OrderInfo {
            order_type,
            clob_token_id: u256_to_dec_cached(&token_bytes, &clob_id),
            usd_value: usd,
            shares,
            price_per_share: price,
        },
    })
}

// ============================================================================
// Hex Parsing Helpers
// ============================================================================

#[inline]
fn parse_u256_hex_slice_with_bytes(full: &str, start: usize, end: usize) -> Option<(U256, [u8; 32])> {
    let slice = full.get(start..end)?;
    let clean = slice.strip_prefix("0x").unwrap_or(slice);
    if clean.len() > 64 { return None; }

    let mut hex_buf = [b'0'; 64];
    hex_buf[64 - clean.len()..].copy_from_slice(clean.as_bytes());

    let mut out = [0u8; 32];
    for i in 0..32 {
        let hi = hex_nibble(hex_buf[i * 2])?;
        let lo = hex_nibble(hex_buf[i * 2 + 1])?;
        out[i] = (hi << 4) | lo;
    }
    Some((U256::from_be_slice(&out), out))
}

#[inline]
fn parse_u256_hex_slice(full: &str, start: usize, end: usize) -> Option<U256> {
    parse_u256_hex_slice_with_bytes(full, start, end).map(|(v, _)| v)
}

fn u256_to_dec_cached(bytes: &[u8; 32], val: &U256) -> Arc<str> {
    TOKEN_ID_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(s) = cache.get(bytes) { return Arc::clone(s); }  // Cheap Arc clone
        let s: Arc<str> = val.to_string().into();
        cache.insert(*bytes, Arc::clone(&s));
        s
    })
}

fn u256_to_f64(v: &U256) -> Option<f64> {
    if v.bit_len() <= 64 { Some(v.as_limbs()[0] as f64) }
    else { v.to_string().parse().ok() }
}

// Hex nibble lookup table - 2-3x faster than branching
const HEX_NIBBLE_LUT: [u8; 256] = {
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