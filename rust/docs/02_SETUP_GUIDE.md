# Complete Setup Guide

This guide will walk you through setting up the Polymarket Copy Trading Bot from scratch, even if you have no coding experience.

## Table of Contents

1. [Prerequisites](#1-prerequisites)
2. [Installing Rust](#2-installing-rust)
3. [Setting Up Your Wallet](#3-setting-up-your-wallet)
4. [Getting API Keys](#4-getting-api-keys)
5. [Finding a Whale Address](#5-finding-a-whale-address)
6. [Configuring the Bot](#6-configuring-the-bot)
7. [Testing Your Setup](#7-testing-your-setup)
8. [Running the Bot](#8-running-the-bot)
9. [Next Steps](#9-next-steps)
10. [Safety Checklist](#10-safety-checklist)
11. [Need Help?](#11-need-help)

---

## 1. Prerequisites

Before starting, make sure you have:

- A computer running Windows, macOS, or Linux
- An internet connection
- A text editor (Notepad, VS Code, or any text editor)
- Basic computer skills (opening files, copying text)

---

## 2. Installing Rust

### 2.1 Windows

1. Visit https://rustup.rs/
2. Download the installer (`rustup-init.exe`)
3. Run the installer
4. When prompted, press `Enter` to proceed with default installation
5. Restart your terminal/PowerShell after installation

**Verify installation:**
Open PowerShell and type:
```powershell
rustc --version
```
You should see something like `rustc 1.xx.x`.

### 2.2 macOS

Open Terminal and run:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Follow the prompts (press `Enter` for defaults).

**Verify installation:**
```bash
rustc --version
```

### 2.3 Linux

Open Terminal and run:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Follow the prompts.

**Verify installation:**
```bash
rustc --version
```

---

## 3. Setting Up Your Wallet

### 3.1 Option 1: Create a New Wallet (Recommended for Testing)

1. Install [MetaMask](https://metamask.io/) browser extension
2. Create a new wallet (or use existing)
3. **Add Polygon Network:**
   - Click the network dropdown (top left)
   - Click "Add Network"
   - Fill in:
     - Network Name: `Polygon Mainnet`
     - RPC URL: `https://polygon-rpc.com`
     - Chain ID: `137`
     - Currency Symbol: `MATIC`
     - Block Explorer: `https://polygonscan.com`

4. **Get Your Private Key:**
   - Click the account icon (top right)
   - Click "Account Details"
   - Click "Export Private Key"
   - Enter your password
   - **Copy the private key** (this is your `PRIVATE_KEY` - keep it secret!)
   - Remove the `0x` prefix if present

5. **Get Your Address:**
   - Your address is shown under your account name
   - It looks like: `0x1234...5678`
   - Copy this (this is your `FUNDER_ADDRESS`)

6. **Fund Your Wallet:**
   - You need USDC or USDC.e on Polygon
   - Bridge tokens from Ethereum to Polygon, or buy on an exchange
   - Minimum recommended: $50-100 for testing

### 3.2 Option 2: Use Existing Wallet

If you already have a wallet with funds on Polygon:
1. Export your private key (see above)
2. Get your wallet address
3. Make sure you have USDC/USDC.e for trading

---

## Getting API Keys

The bot needs a WebSocket connection to the Polygon blockchain. You'll use either Alchemy or Chainstack.

### Option 1: Alchemy (Recommended)

1. Go to https://www.alchemy.com/
2. Click "Create App" or "Sign Up"
3. Fill in:
   - App Name: `Polymarket Bot`
   - Chain: `Polygon`
   - Network: `Polygon Mainnet`
4. After creation, click on your app
5. Find "API Key" section
6. Copy the API key (this is your `ALCHEMY_API_KEY`)

**Free tier includes:** 300M compute units/month (more than enough for this bot)

### 4.2 Option 2: Chainstack (Alternative)

1. Go to https://chainstack.com/
2. Sign up for free account
3. Create a new project
4. Add Polygon Mainnet node
5. Get your WebSocket URL
6. Extract the API key from the URL (this is your `CHAINSTACK_API_KEY`)

---

## Finding a Whale Address

A "whale" is a successful trader you want to copy. Here's how to find one:

### Method 1: Polymarket Leaderboards

1. Visit https://polymarket.com/leaderboard
2. Look for traders with high win rates and profits
3. Click on a trader's profile
4. Find their wallet address (usually visible in their profile)
5. Copy the address (40 characters, remove `0x` if present)

### 5.2 Method 2: Analyze Recent Winners

1. Go to Polymarket markets
2. Check "Settled" markets
3. Find markets with large payouts
4. Click on winning positions
5. Note the wallet addresses that hold winning positions
6. Research these addresses to find consistent winners

### 5.3 Method 3: Social Media/Communities

- Check Polymarket Discord/Telegram
- Look for traders sharing their addresses
- Verify their track record before copying

**Important:** Always verify a whale's performance before copying. Past performance doesn't guarantee future results.

---

## 6. Configuring the Bot

### 6.1 Step 1: Clone/Download the Repository

If you have git:
```bash
git clone https://github.com/soulcrancerdev/polymarket-kalshi-copy-trading-arbitrage-bot
cd polymarket-kalshi-copy-trading-arbitrage-bot/rust
```

Or download and extract the ZIP file.

### 6.2 Step 2: Create Your .env File

**Windows (PowerShell):**
```powershell
Copy-Item .env.example .env
```

**macOS/Linux:**
```bash
cp .env.example .env
```

**Or manually:**
1. Copy `.env.example`
2. Rename the copy to `.env` (no extension on Linux/macOS)

### 6.3 Step 3: Edit .env File

Open `.env` in any text editor. You'll see something like:

```env
PRIVATE_KEY=your_private_key_here
FUNDER_ADDRESS=your_wallet_address_here
TARGET_WHALE_ADDRESS=target_whale_address_here
ALCHEMY_API_KEY=your_alchemy_api_key_here
```

Replace each value:

1. **PRIVATE_KEY**: Paste your wallet's private key (no `0x` prefix)
2. **FUNDER_ADDRESS**: Paste your wallet address (can have `0x` or not)
3. **TARGET_WHALE_ADDRESS**: Paste the whale address (no `0x` prefix)
4. **ALCHEMY_API_KEY**: Paste your Alchemy API key

**Example (don't use these - they're fake):**
```env
PRIVATE_KEY=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
FUNDER_ADDRESS=0x1234567890123456789012345678901234567890
TARGET_WHALE_ADDRESS=204f72f35326db932158cba6adff0b9a1da95e14
ALCHEMY_API_KEY=abc123xyz789
```

### 6.4 Step 4: Set Initial Trading Mode

For your first run, set:
```env
ENABLE_TRADING=false
MOCK_TRADING=true
```

This lets you see what the bot would do without actually trading.

---

## 7. Testing Your Setup

### 7.1 Step 1: Validate Configuration

Run the configuration checker:
```bash
cargo run --release --bin validate_setup
```

**What it checks:**
- All required values are set
- Address formats are correct
- API keys are valid format
- Private key format is correct

**Fix any errors it reports before proceeding.**

### 7.2 Step 2: Build the Bot

```bash
cargo build --release
```

This will take 5-10 minutes the first time (downloads dependencies).

### 7.3 Step 3: Test Run (Mock Mode)

Make sure your `.env` has:
```env
ENABLE_TRADING=false
MOCK_TRADING=true
```

Then run:
```bash
cargo run --release
```

**What to expect:**
- Bot connects to blockchain
- You see connection messages
- When whale trades, you see simulated trade messages
- No actual trades are placed

**If you see errors:**
- Check [Troubleshooting Guide](06_TROUBLESHOOTING.md)
- Verify your API key is correct
- Make sure your addresses are correct format

---

## 8. Running the Bot

### 8.1 Step 1: Enable Trading

Once you've tested and are confident:

1. Edit `.env`
2. Set:
   ```env
   ENABLE_TRADING=true
   MOCK_TRADING=false
   ```

3. Save the file

### 8.2 Step 2: Run the Bot

```bash
cargo run --release
```

**Windows users:** You can also use `run.bat` (double-click after setup).

### 8.3 Step 3: Monitor Output

You'll see messages like:
```
🚀 Starting trader. Trading: true, Mock: false
🔌 Connected. Subscribing...
⚡ [B:12345] BUY_FILL | $100 | 200 OK | ...
```

**What each message means:**
- `[B:12345]` = Block number where trade was detected
- `BUY_FILL` = Type of trade (BUY or SELL)
- `$100` = USD value of whale's trade
- `200 OK` = Your order was successfully placed
- Numbers after = Your fill details

### 8.4 Step 4: Check Results

- **Live:** Watch console output in real-time
- **CSV Log:** Check `matches_optimized.csv` for all trades
- **Polymarket:** Check your positions on Polymarket website

---

## 9. Next Steps

- Read [Features Guide](04_FEATURES.md) to understand what the bot does
- Adjust [Configuration](03_CONFIGURATION.md) settings as needed
- Check [Troubleshooting](06_TROUBLESHOOTING.md) if you have issues

---

## 10. Safety Checklist

Before running with real money:

- [ ] Tested in mock mode successfully
- [ ] Verified all addresses are correct
- [ ] Have sufficient funds in wallet
- [ ] Understand the risks involved
- [ ] Started with small test amounts
- [ ] Have a way to stop the bot (Ctrl+C)
- [ ] Backed up your `.env` file securely

---

## 11. Need Help?

1. Check [Troubleshooting Guide](06_TROUBLESHOOTING.md)
2. Verify your configuration with `validate_setup`
3. Review error messages carefully
4. Make sure all prerequisites are installed

