# Polymarket Copy Trading Bot

A high-performance automated trading bot built with Rust that copies trades from successful Polymarket traders (whales) in real-time.

---

## Table of Contents

1. [Requirements](#1-requirements)
2. [Quick Start (For Beginners)](#2-quick-start-for-beginners)
3. [Documentation/Tutorials](#3-documentationtutorials)
4. [Security Considerations](#4-security-considerations)
5. [How It Works](#5-how-it-works)
6. [Features](#6-features)
7. [Advanced Usage](#7-advanced-usage)
8. [Output Files](#8-output-files)
9. [Getting Help](#9-getting-help)
10. [Disclaimer](#10-disclaimer)

---

## 1. Requirements

### 1.1 Required

1. **Polymarket Account** - Sign up at https://polymarket.com
2. **Web3 Wallet** - MetaMask recommended (with some USDC/USDC.e on Polygon)
3. **RPC Provider API Key** - Get a free tier from [Alchemy](https://www.alchemy.com/) or [Chainstack](https://chainstack.com/)
4. **Whale Address** - The trader you want to copy (40-character hex address)

### 1.2 Recommended

- **Some programming knowledge** - Not required, but helpful for troubleshooting
- **Sufficient capital** - Bot uses 2% of whale trade size by default (configurable)

---

## 2. Quick Start (**Supports Windows, Linux, WSL, and macOS**)

### 1️⃣ Clone the Project
(Make sure you have `git` installed. If not, refer to ➡️[Git Installation Tutorial](./安装git教程.md))

```
# Clone the repository
git clone https://github.com/oxmoei/Polymarket-Copy-Trading-Bot.git

# Enter the project directory
cd Polymarket-Copy-Trading-Bot

```

### 2️⃣ Quick Install Dependencies

One-click check and install missing prerequisites.

#### 📌 Linux / macOS / WSL Users

```bash
# Execute in the project root directory
./install.sh
```

#### 📌 Windows Users

```powershell
# Run PowerShell as Administrator, then execute in the project root directory
Set-ExecutionPolicy Bypass -Scope CurrentUser
.\install.ps1
```

### 3️⃣ Configure Environment Variables

#### 📌 Linux / macOS / WSL Users
```bash
# Copy the example environment file and edit settings
cp .env.example .env && nano .env # Press Ctrl+O to save, Ctrl+X to exit after editing
```

#### 📌 Windows Users
```powershell
# Copy the example environment file
Copy-Item .env.example .env

# Edit settings
notepad .env  # Save and close after editing
```

Fill in the required values (see [Configuration Guide](docs/03_CONFIGURATION.md) for details):
   - `PRIVATE_KEY` - Your wallet private key (keep it secret!)
   - `FUNDER_ADDRESS` - Your wallet address (the wallet corresponding to the private key)
   - `TARGET_WHALE_ADDRESS` - The whale address you want to copy (40-character hex, without 0x)
   - `ALCHEMY_API_KEY` - Get from https://www.alchemy.com/ (or use CHAINSTACK_API_KEY)

Optional: Adjust trading settings (see [Configuration Guide](docs/03_CONFIGURATION.md))

### 4️⃣ Verify Your Configuration

Before running the bot, verify that your settings are correct:

```
cargo run --release --bin validate_setup
```

This will check that all required settings are correct and provide helpful error messages if something is wrong.

### 5️⃣ Test Mode (Recommended First)

Run in test mode to see what the bot would do without actually trading:

```
# Set MOCK_TRADING=true in the .env file, then:
cargo run --release
```

### 6️⃣ Run the Bot

#### 📌 Linux / macOS / WSL Users
```bash
# Enable trading in .env (ENABLE_TRADING=true, MOCK_TRADING=false)
cargo run --release
```

#### 📌 Windows Users
```powershell
# Enable trading in .env (ENABLE_TRADING=true, MOCK_TRADING=false)
.\run.bat
```

---

## 3. Documentation/Tutorials

- **[01. Quick Start Guide](docs/01_QUICK_START.md)** - 5-minute setup guide
- **[02. Complete Setup Guide](docs/02_SETUP_GUIDE.md)** - Detailed step-by-step instructions
- **[03. Configuration Guide](docs/03_CONFIGURATION.md)** - All settings explained
- **[04. Features Overview](docs/04_FEATURES.md)** - Bot features and how they work
- **[05. Trading Strategy](docs/05_STRATEGY.md)** - Complete strategy logic and decision process
- **[06. Troubleshooting](docs/06_TROUBLESHOOTING.md)** - Common issues and solutions

---

## 4. Security Considerations

⚠️ **Important:**
- Never share your `PRIVATE_KEY` with anyone
- Never commit your `.env` file to git (it's already in `.gitignore`)
- Test with small amounts first
- Use `MOCK_TRADING=true` first to verify everything works

---

## 5. How It Works

1. **Monitors** blockchain events for trades from target whale (real-time via WebSocket)
2. **Analyzes** each trade (size, price, market conditions) using multi-layer risk checks
3. **Calculates** position size (default 2%, with tiered multipliers) and price (whale price + buffer)
4. **Executes** proportionally scaled trades using optimized order types (FAK/GTD)
5. **Retries** failed orders with intelligent resubmission logic (up to 4-5 attempts)
6. **Protects** you with risk guards (circuit breakers) and safety features
7. **Logs** everything to CSV files for analysis

**Strategy Highlights:**
- **2% Position Scaling:** Reduces risk while maintaining meaningful positions
- **Tiered Execution:** Different strategies for large (4000+), medium (2000-3999), and small (<2000) trades
- **Multi-Layer Risk Management:** 4 layers of safety checks prevent dangerous trades
- **Smart Pricing:** Price buffers optimize fill rates (higher for large trades, none for small)
- **Sport-Specific Adjustments:** Additional buffers for tennis and soccer markets

See [Features Overview](docs/04_FEATURES.md) for detailed feature information, and [Strategy Guide](docs/05_STRATEGY.md) for complete trading logic.

---

## 6. Features

- ✅ Real-time trade copying
- ✅ Intelligent position management (default 2%, configurable)
- ✅ Risk management circuit breakers
- ✅ Automatic order resubmission on failure
- ✅ Market cache system for fast lookups
- ✅ CSV logging of all trades
- ✅ Real-time market detection
- ✅ Tiered execution based on trade size

---

## 7. Advanced Usage

### 7.1 Running Different Modes

```bash
# Standard mode (monitors confirmed blocks)
cargo run --release

# Mempool mode (faster, but less reliable)
cargo run --release --bin mempool_monitor

# Monitor only your own fills (no trading)
cargo run --release --bin trade_monitor

# Validate configuration
cargo run --release --bin validate_setup
```

### 7.2 Building Production Version

```bash
# Optimized release build
cargo build --release

# Binary located at: target/release/pm_bot.exe (Windows)
#                        target/release/pm_bot (macOS/Linux)
```

---

## 8. Output Files

- `matches_optimized.csv` - All detected and executed trades
- `.clob_creds.json` - Auto-generated API credentials (do not modify)
- `.clob_market_cache.json` - Market data cache (auto-updated)

---

## 9. Getting Help

1. Check the [Troubleshooting Guide](docs/06_TROUBLESHOOTING.md)
2. Run the configuration validator: `cargo run --release --bin validate_setup`
3. Check your `.env` file against `.env.example`
4. Review error messages in console output
5. See the [Strategy Guide](docs/05_STRATEGY.md) to understand bot logic

---

## 10. Disclaimer

This bot is provided as-is. Trading involves financial risk. Use at your own discretion. Test thoroughly before using real funds. The author is not responsible for any losses.

☕ **Buy me a coffee (EVM):** `0xd9c5d6111983ea3692f1d29bec4ac7d6f723217a`

