# Quantara: Rust Multi-Asset Rules & Backtesting Engine

[![Buy Me A Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-ffdd00?style=for-the-badge&logo=buy-me-a-coffee&logoColor=black)](https://www.buymeacoffee.com/MichelBernasconi)

Quantara is a high-performance backend built in Rust for backtesting investment strategies across multiple asset classes (stocks, bonds, ETFs, etc.) without the use of AI. It provides a transparent and repeatable environment for testing rule-based algorithms.

## ✨ Features
- **Multi-Asset Engine**: Handle Stocks, Bonds, ETFs, and more.
- **Rules Engine**: Define complex entry/exit conditions using technical indicators.
- **Indicators**: Support for SMA, RSI, and Constant values.
- **Rebalancing**: Support for periodic portfolio rebalancing (e.g., monthly).
- **Precision**: Financial-grade precision using `rust_decimal`.
- **REST API**: Fully managed via a modern Axum-based web interface.

## 🚀 Getting Started

### Prerequisites
- Rust (Latest stable)
- Python (for running examples)

### Installation
```bash
cargo build --release
cargo run
```
The server will start on `http://127.0.0.1:3000`.

## 📂 Examples & Demos
We have provided several examples in the `examples/` directory to help you get started:
- `01_simple_mock`: A basic connectivity test.
- `02_trend_following`: Real-world SPY data with an SMA 200 trend-following strategy.
- `03_multi_asset_rotation`: Diversified portfolio with RSI signals and monthly rebalancing.

Run them using:
```bash
pip install yfinance requests
python examples/03_multi_asset_rotation/run.py
```

## 📜 License
This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
