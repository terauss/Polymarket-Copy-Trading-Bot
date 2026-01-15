// src/bin/test_fak_response.rs
// Test FAK order response format with a minimal $1 order
//
// Usage: cargo run --bin test_fak_response
//
// This submits a real FAK order to observe the response format.
// We fetch the best ask and bid at that price to ensure a fill.

use anyhow::{Result, anyhow};
use dotenvy::dotenv;
use pm_whale_follower::{ApiCreds, OrderArgs, RustClobClient, PreparedCreds, OrderResponse};
use std::path::Path;

const CLOB_API_BASE: &str = "https://clob.polymarket.com";

// NBA Rockets vs Nuggets 2025-12-20 - Rockets token
const TEST_TOKEN_ID: &str = "54829853978330669429551251905778214074128014124609781186771015417529556703558";

fn fetch_best_ask(token_id: &str) -> Result<(f64, f64)> {
    let url = format!("{}/book?token_id={}", CLOB_API_BASE, token_id);
    // Use a no-proxy client to avoid timeout issues
    let http = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    let resp = http
        .get(&url)
        .send()?;

    if !resp.status().is_success() {
        return Err(anyhow!("Failed to fetch book: {}", resp.status()));
    }

    let book: serde_json::Value = resp.json()?;
    let asks = book["asks"].as_array().ok_or_else(|| anyhow!("No asks"))?;

    if asks.is_empty() {
        return Err(anyhow!("Empty order book"));
    }

    // Find the best (lowest) ask
    let best_ask = asks.iter()
        .filter_map(|a| {
            let price: f64 = a["price"].as_str()?.parse().ok()?;
            let size: f64 = a["size"].as_str()?.parse().ok()?;
            Some((price, size))
        })
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
        .ok_or_else(|| anyhow!("No valid asks found"))?;

    Ok(best_ask)
}

fn main() -> Result<()> {
    dotenv().ok();

    let private_key = std::env::var("PRIVATE_KEY")?;
    let funder = std::env::var("FUNDER_ADDRESS")?;

    println!("🧪 FAK Response Test");
    println!("====================\n");

    // Build client
    let mut client = RustClobClient::new(CLOB_API_BASE, 137, &private_key, &funder)?
        .with_cache_path(".clob_market_cache.json");
    let _ = client.load_cache();

    // Load or derive creds
    let creds_path = ".clob_creds.json";
    let creds: ApiCreds = if Path::new(creds_path).exists() {
        let data = std::fs::read_to_string(creds_path)?;
        serde_json::from_str(&data)?
    } else {
        let derived = client.derive_api_key(0)?;
        std::fs::write(creds_path, serde_json::to_string_pretty(&derived)?)?;
        derived
    };
    let prepared = PreparedCreds::from_api_creds(&creds)?;

    // Fetch current best ask
    println!("📖 Fetching order book...");
    let (best_ask_price, best_ask_size) = fetch_best_ask(TEST_TOKEN_ID)?;
    println!("   Best ask: {:.2} @ {} shares available\n", best_ask_price, best_ask_size);

    // Buy a small amount at the best ask price (should fill immediately)
    // Request MORE than available to test underfill scenario
    let order_size = (best_ask_size + 5.0).min(20.0);  // Try to buy more than available, max 20
    let order_price = best_ask_price;  // Match the best ask exactly

    let args = OrderArgs {
        token_id: TEST_TOKEN_ID.to_string(),
        price: order_price,
        size: order_size,
        side: "BUY".into(),
        fee_rate_bps: None,
        nonce: Some(0),
        expiration: Some("0".into()),
        taker: None,
        order_type: Some("FAK".to_string()),
    };

    println!("📤 Submitting FAK order:");
    println!("   Token: {}...", &TEST_TOKEN_ID[..20]);
    println!("   Size: {} shares", args.size);
    println!("   Price: ${:.2}", args.price);
    println!("   Max cost: ${:.2}\n", args.size * args.price);

    // Submit order
    let signed = client.create_order(args)?;
    let body = signed.post_body(&creds.api_key, "FAK");

    println!("📨 Request body:\n{}\n", body);

    let resp = client.post_order_fast(body, &prepared)?;

    let status = resp.status();
    let body_text = resp.text().unwrap_or_default();

    println!("📥 Response:");
    println!("   Status: {}", status);
    println!("   Body: {}\n", body_text);

    // Try to parse as OrderResponse
    if let Ok(parsed) = serde_json::from_str::<OrderResponse>(&body_text) {
        println!("✅ Parsed OrderResponse:");
        println!("   success: {}", parsed.success);
        println!("   errorMsg: \"{}\"", parsed.error_msg);
        println!("   orderID: \"{}\"", parsed.order_id);
        println!("   status: \"{}\"", parsed.status);
        println!("   takingAmount: \"{}\"", parsed.taking_amount);
        println!("   makingAmount: \"{}\"", parsed.making_amount);
        println!("   transactionsHashes: {:?}", parsed.transactions_hashes);

        // Calculate fill info
        // NOTE: API returns human-readable values, NOT micro-units!
        let shares_filled: f64 = parsed.taking_amount.parse().unwrap_or(0.0);
        let usdc_spent: f64 = parsed.making_amount.parse().unwrap_or(0.0);

        if shares_filled > 0.0 {
            let avg_price = usdc_spent / shares_filled;

            println!("\n📊 Fill Analysis:");
            println!("   Shares filled: {:.2}", shares_filled);
            println!("   USDC spent: ${:.2}", usdc_spent);
            println!("   Avg price: ${:.4}", avg_price);
            println!("   Requested: {:.2} shares", order_size);
            println!("   Unfilled: {:.2} shares", order_size - shares_filled);

            if shares_filled < order_size {
                println!("\n🔶 UNDERFILL DETECTED!");
                println!("   This confirms takingAmount represents actual fill, not requested amount.");
            } else {
                println!("\n🟢 FULL FILL - try with larger order_size to test underfill");
            }
        } else {
            println!("\n📊 No fill (takingAmount = 0)");
        }
    } else {
        println!("❌ Failed to parse as OrderResponse");

        // Try pretty-print the raw JSON
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body_text) {
            println!("   Raw JSON: {}", serde_json::to_string_pretty(&v)?);
        }
    }

    Ok(())
}
