# Polymarket Copy Trading Bot

> Automated copy trading bots for Polymarket that mirror trades from successful traders (whales) in real-time.

[![GitHub](https://img.shields.io/badge/GitHub-Repository-blue)](https://github.com/terauss/Polymarket-Copy-Trading-Bot)
[![License: ISC](https://img.shields.io/badge/License-ISC-blue.svg)](LICENSE)

## 📋 Overview

This repository contains **two implementations** of a Polymarket copy trading bot:

1. **Rust Bot** (`rust/`) - High-performance, production-ready implementation
2. **Python Bot** (`python/`) - Feature-rich implementation with extensive tooling

Both bots automatically monitor and copy trades from successful Polymarket traders, executing scaled positions based on your capital and risk preferences.

## ⚠️ Important Update Notice

**Update Status:** The bot has been updated with Rust and Python implementations. There is no need to update if you already have a working version.

**Performance:** The bot now makes transactions within 1 block with insider traders.

**Latest Version:** From today, if users want to get the latest version, please contact me via Telegram: [@terauss](https://t.me/terauss)

## 🚀 Quick Start

### Choose Your Bot

**Use the Rust Bot if you want:**
- ⚡ Maximum performance and speed
- 🎯 Focused, streamlined trading functionality
- 🔒 Production-grade reliability
- 📦 Single binary deployment

**Use the Python Bot if you want:**
- 🛠️ Extensive tooling and research scripts
- 📊 Advanced simulation and backtesting
- 🔍 Trader research and analysis tools
- 💼 Position management utilities
- 📈 Comprehensive logging and monitoring

### Rust Bot Quick Start

```bash
cd rust
# Follow the setup guide in rust/README.md
cargo run --release --bin validate_setup
cargo run --release
```

📖 **Full Documentation:** [rust/README.md](rust/README.md)

### Python Bot Quick Start

```bash
cd python
pip install -r requirements.txt
python -m src.scripts.setup.setup
python -m src.main
```

📖 **Full Documentation:** [python/README.md](python/README.md)

## 📁 Repository Structure

```
Polymarket-Copy-Trading-Bot/
│
├── rust/                    # Rust implementation
│   ├── src/                # Source code
│   ├── docs/               # Documentation
│   ├── README.md           # Rust bot documentation
│   └── Cargo.toml          # Rust dependencies
│
├── python/                  # Python implementation
│   ├── src/                # Source code
│   ├── docs/               # Documentation
│   ├── scripts/            # Utility scripts
│   ├── README.md           # Python bot documentation
│   └── requirements.txt    # Python dependencies
│
└── README.md               # This file
```

## 🔄 Key Differences

| Feature | Rust Bot | Python Bot |
|---------|----------|------------|
| **Performance** | ⚡ Extremely fast | 🐍 Moderate |
| **Setup Complexity** | 🟢 Simple | 🟡 Moderate |
| **Trading Features** | ✅ Core trading | ✅ Core + Advanced |
| **Research Tools** | ❌ No | ✅ Extensive |
| **Simulation** | ❌ No | ✅ Full backtesting |
| **Position Management** | ❌ Basic | ✅ Advanced |
| **MongoDB Integration** | ❌ No | ✅ Yes |
| **Multi-Trader Support** | ✅ Single trader | ✅ Multiple traders |
| **Documentation** | 📚 Comprehensive | 📚 Extensive |

## 🎯 How It Works

Both bots follow a similar core workflow:

1. **Monitor** - Watch for trades from target traders (whales)
2. **Analyze** - Evaluate trade size, price, and market conditions
3. **Calculate** - Determine scaled position size based on your capital
4. **Execute** - Place matching orders on Polymarket
5. **Track** - Log all trades for analysis

### Rust Bot Strategy

- Real-time blockchain monitoring via WebSocket
- 2% default position scaling (configurable)
- Tiered execution based on trade size
- Multi-layer risk management
- Sport-specific market adjustments

### Python Bot Strategy

- API polling or WebSocket monitoring
- Proportional position sizing based on capital ratios
- Trade aggregation for efficiency
- MongoDB persistence for trade history
- Advanced risk management and circuit breakers

## 📚 Documentation

### Rust Bot Documentation

- **[Quick Start Guide](rust/docs/01_QUICK_START.md)** - 5-minute setup
- **[Complete Setup Guide](rust/docs/02_SETUP_GUIDE.md)** - Detailed instructions
- **[Configuration Guide](rust/docs/03_CONFIGURATION.md)** - All settings explained
- **[Features Overview](rust/docs/04_FEATURES.md)** - Feature details
- **[Trading Strategy](rust/docs/05_STRATEGY.md)** - Strategy logic
- **[Troubleshooting](rust/docs/06_TROUBLESHOOTING.md)** - Common issues

### Python Bot Documentation

- **[Getting Started Guide](python/docs/GETTING_STARTED.md)** - Complete setup
- **[Strategy Guide](python/docs/STRATEGY.md)** - Copy trading strategy
- **[Command Reference](python/docs/COMMAND_REFERENCE.md)** - All commands
- **[Usage Examples](python/docs/EXAMPLES.md)** - Practical examples

## ⚙️ Requirements

### Common Requirements

- **Polymarket Account** - Sign up at [polymarket.com](https://polymarket.com)
- **Web3 Wallet** - MetaMask recommended
- **USDC/USDC.e** - On Polygon network for trading
- **RPC Provider** - Alchemy, Chainstack, or Infura API key
- **Whale Address** - The trader address you want to copy

### Rust Bot Specific

- Rust 1.70+ ([Install Rust](https://rustup.rs/))

### Python Bot Specific

- Python 3.10+
- MongoDB database ([MongoDB Atlas](https://www.mongodb.com/cloud/atlas/register) free tier works)

## 🔒 Security Notes

⚠️ **IMPORTANT:**

- Never share your `PRIVATE_KEY` with anyone
- Never commit your `.env` file to git
- Start with small amounts to test
- Use test/mock mode first to verify everything works
- Use a dedicated wallet separate from your main funds

## 🛠️ Features

### Rust Bot Features

- ✅ Real-time trade copying via WebSocket
- ✅ Intelligent position sizing (2% default)
- ✅ Circuit breakers for risk management
- ✅ Automatic order resubmission
- ✅ Market cache system
- ✅ CSV logging
- ✅ Live market detection
- ✅ Tiered execution

### Python Bot Features

- ✅ Multi-trader support
- ✅ Smart position sizing
- ✅ Trade aggregation
- ✅ Real-time execution
- ✅ MongoDB integration
- ✅ Price protection
- ✅ Extensive research tools
- ✅ Simulation & backtesting
- ✅ Position management utilities
- ✅ Wallet management scripts

## 📊 Output Files

### Rust Bot
- `matches_optimized.csv` - All detected and executed trades
- `.clob_creds.json` - Auto-generated API credentials
- `.clob_market_cache.json` - Market data cache

### Python Bot
- `logs/bot-YYYY-MM-DD.log` - Daily log files
- `trader_data_cache/` - Cached trader data
- `simulation_results/` - Simulation results
- MongoDB collections - Trade history and positions

## 🆘 Getting Help

1. Check the bot-specific troubleshooting guides:
   - [Rust Bot Troubleshooting](rust/docs/06_TROUBLESHOOTING.md)
   - Python Bot: Run `python -m src.scripts.setup.system_status`
2. Validate your configuration:
   - Rust: `cargo run --release --bin validate_setup`
   - Python: `python -m src.scripts.setup.system_status`
3. Review your `.env` file against the examples
4. Check console output for error messages

## 📄 Contact

For questions or issues, contact via Telegram: [@terauss](https://t.me/terauss)

## ⚖️ Disclaimer

This software is provided as-is. Trading involves financial risk. Use at your own discretion. Test thoroughly before using real funds. The authors are not responsible for any losses.

## 📝 License

ISC License - See [LICENSE](LICENSE) file for details.

## 🌟 Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 🔗 Links

- **GitHub Repository:** [https://github.com/terauss/Polymarket-Copy-Trading-Bot](https://github.com/terauss/Polymarket-Copy-Trading-Bot)
- **Polymarket:** [https://polymarket.com](https://polymarket.com)
- **Polymarket Leaderboard:** [https://polymarket.com/leaderboard](https://polymarket.com/leaderboard)
- **Predictfolio:** [https://predictfolio.com](https://predictfolio.com)

---

**Made with ❤️ for the Polymarket community**

