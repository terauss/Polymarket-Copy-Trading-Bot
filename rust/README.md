# 🐋 Polymarket Copy Trading Bot

> A high-performance Rust-based automated trading bot that copies trades from successful Polymarket traders (whales) in real-time. 🚀

## 📑 Table of Contents

1. [Quick Start Guide](#1-quick-start-guide-for-beginners-)
2. [Documentation](#2-documentation-)
3. [Requirements](#3-requirements-)
4. [Security Notes](#4-security-notes-)
5. [How It Works](#5-how-it-works-)
6. [Features](#6-features-)
7. [Advanced Usage](#7-advanced-usage-)
8. [Output Files](#8-output-files-)
9. [Getting Help](#9-getting-help-)
10. [Disclaimer](#10-disclaimer-)

---

## 1. Quick Start Guide (For Beginners) 🎯

### 1.1 Step 1: Install Rust ⚙️

**Windows:**
1. Download & run the installer from https://rustup.rs/
2. Follow the installation wizard
3. Restart ur terminal/PowerShell

**macOS/Linux:**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 1.2 Step 2: Clone This Repo 📥

```bash
git clone https://github.com/soulcrancerdev/polymarket-kalshi-copy-trading-arbitrage-bot
cd polymarket-kalshi-copy-trading-arbitrage-bot/rust
```

### 1.3 Step 3: Configure Ur Settings 🔧

1. **Copy the example env file:**
   ```bash
   # Windows (PowerShell)
   Copy-Item .env.example .env
   
   # macOS/Linux
   cp .env.example .env
   ```

2. **Open `.env`** in any text editor (Notepad, VS Code, whatever u prefer)

3. **Fill in the required values** (check out [Configuration Guide](docs/03_CONFIGURATION.md) for deets):
   - `PRIVATE_KEY` - Ur wallet's private key (keep this SECRET! 🔒)
   - `FUNDER_ADDRESS` - Ur wallet address (same wallet as private key)
   - `TARGET_WHALE_ADDRESS` - The whale address u wanna copy (40-char hex, no 0x)
   - `ALCHEMY_API_KEY` - Get from https://www.alchemy.com/ (or use CHAINSTACK_API_KEY)

4. **Optional:** Adjust trading settings (see [Configuration Guide](docs/03_CONFIGURATION.md))

### 1.4 Step 4: Validate Ur Config ✅

Before running the bot, make sure everything's set up correctly:

```bash
cargo run --release --bin validate_setup
```

This'll check if all required settings are correct & give u helpful error messages if something's wrong.

### 1.5 Step 5: Test Mode (Recommended First) 🧪

Run in test mode to see what the bot would do without actually trading:

```bash
# Set MOCK_TRADING=true in ur .env file, then:
cargo run --release
```

### 1.6 Step 6: Run the Bot 🚀

Once u're confident everything works:

```bash
# Enable trading in .env (ENABLE_TRADING=true, MOCK_TRADING=false)
cargo run --release
```

**Windows users:** U can also double-click `run.bat` after setting up ur `.env` file. 🪟

---

## 2. Documentation 📚

- **[01. Quick Start Guide](docs/01_QUICK_START.md)** - 5-minute setup guide ⚡
- **[02. Complete Setup Guide](docs/02_SETUP_GUIDE.md)** - Detailed step-by-step instructions 📖
- **[03. Configuration Guide](docs/03_CONFIGURATION.md)** - All settings explained 🔍
- **[04. Features Overview](docs/04_FEATURES.md)** - What the bot does & how it works 🤖
- **[05. Trading Strategy](docs/05_STRATEGY.md)** - Complete strategy logic & decision-making 🧠
- **[06. Troubleshooting](docs/06_TROUBLESHOOTING.md)** - Common issues & solutions 🔧

---

## 3. Requirements 📋

### 3.1 Required Stuff 💯

1. **A Polymarket Account** - Sign up at https://polymarket.com
2. **A Web3 Wallet** - MetaMask recommended (with some USDC/USDC.e on Polygon) 💰
3. **RPC Provider API Key** - Free tier from [Alchemy](https://www.alchemy.com/) or [Chainstack](https://chainstack.com/) 🔑
4. **The Whale Address** - The trader u wanna copy (40-character hex address) 🐋

### 3.2 Recommended (But Not Required) 💡

- **Some Coding Knowledge** - Not required, but helpful for troubleshooting
- **Sufficient Funds** - The bot uses 2% of whale trade size by default (configurable) 💵

---

## 4. Security Notes 🔒

⚠️ **IMPORTANT STUFF:**
- Never share ur `PRIVATE_KEY` with anyone (seriously, don't do it!)
- Never commit ur `.env` file to git (it's already in `.gitignore`)
- Start with small amounts to test
- Use `MOCK_TRADING=true` first to verify everything works

---

## 5. How It Works 🎮

Here's the lowdown on what this bot does:

1. **Monitors** 🔍 blockchain events for trades from ur target whale (real-time via WebSocket)
2. **Analyzes** 🧠 each trade (size, price, market conditions) using multi-layer risk checks
3. **Calculates** 📊 position size (2% default, with tier-based multipliers) & price (whale price + buffer)
4. **Executes** ⚡ a scaled copy of the trade with optimized order types (FAK/GTD)
5. **Retries** 🔄 failed orders with intelligent resubmission logic (up to 4-5 attempts)
6. **Protects** 🛡️ u with risk guards (circuit breakers) & safety features
7. **Logs** 📝 everything to CSV files for analysis

**Strategy Highlights:**
- **2% Position Scaling:** Reduces risk while maintaining meaningful positions 📉
- **Tiered Execution:** Different strategies for large (4000+), medium (2000-3999), & small (<2000) trades 🎯
- **Multi-Layer Risk Management:** 4 layers of safety checks prevent dangerous trades 🛡️
- **Intelligent Pricing:** Price buffers optimize fill rates (higher for large trades, none for small) 💡
- **Sport-Specific Adjustments:** Additional buffers for tennis & soccer markets 🎾⚽

Check out [Features Overview](docs/04_FEATURES.md) for feature deets & [Strategy Guide](docs/05_STRATEGY.md) for complete trading logic.

---

## 6. Features ✨

- ✅ Real-time trade copying 🔄
- ✅ Intelligent position sizing (2% default, configurable) 📊
- ✅ Circuit breakers for risk management 🛡️
- ✅ Automatic order resubmission on failures 🔄
- ✅ Market cache system for fast lookups ⚡
- ✅ CSV logging for all trades 📝
- ✅ Live market detection 🔍
- ✅ Tiered execution based on trade size 🎯

---

## 7. Advanced Usage 🚀

### 7.1 Running Different Modes 🎛️

```bash
# Standard mode (monitors confirmed blocks)
cargo run --release

# Mempool mode (faster, but less reliable)
cargo run --release --bin mempool_monitor

# Monitor ur own fills only (no trading)
cargo run --release --bin trade_monitor

# Validate configuration
cargo run --release --bin validate_setup
```

### 7.2 Building for Production 🏗️

```bash
# Optimized release build
cargo build --release

# The binary will be at: target/release/pm_bot.exe (Windows)
#                        target/release/pm_bot (macOS/Linux)
```

---

## 8. Output Files 📁

- `matches_optimized.csv` - All detected & executed trades 📊
- `.clob_creds.json` - Auto-generated API credentials (don't modify) 🔑
- `.clob_market_cache.json` - Market data cache (auto-updated) 💾

---

## 9. Getting Help 🆘

If u're stuck, try these:

1. Check [Troubleshooting Guide](docs/06_TROUBLESHOOTING.md) 🔧
2. Run the config validator: `cargo run --release --bin validate_setup` ✅
3. Review ur `.env` file against `.env.example` 📋
4. Check console output for error messages 🐛
5. Review [Strategy Guide](docs/05_STRATEGY.md) to understand bot logic 🧠

---

## 10. Disclaimer ⚠️

This bot is provided as-is. Trading involves financial risk. Use at ur own discretion. Test thoroughly before using real funds. The authors are not responsible for any losses. 💸

---

## 📄 Contact

Got questions or issues? Hit me up on Telegram: [@soulcrancerdev](https://t.me/soulcrancerdev) 💬
