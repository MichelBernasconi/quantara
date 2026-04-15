# Quantara: Rust Multi-Asset Rules & Backtesting Engine

Quantara is a high-performance backend built in Rust for backtesting investment strategies across multiple asset classes (stocks, bonds, ETFs, etc.) without the use of AI. It provides a transparent and repeatable environment for testing rule-based algorithms.

## Features
- **Asset Management**: Support for multiple asset types.
- **Rules Engine**: Define complex entry and exit rules based on technical indicators (e.g., SMA) and price action.
- **Backtesting API**: Execute historical simulations and retrieve key performance metrics.
- **RESTful Interface**: Easy integration for data ingestion and strategy management.
- **Precision First**: Uses `rust_decimal` to ensure financial accuracy.

## Tech Stack
- **Rust**
- **Axum** (Web Framework)
- **Tokio** (Async Runtime)
- **Serde** (Serialization)
- **Chrono** (Time Management)

## Getting Started

### Prerequisites
- Rust (Latest stable)

### Installation
1. Clone the repository.
2. Build the project:
   ```bash
   cargo build --release
   ```
3. Run the server:
   ```bash
   cargo run
   ```
The server will start on `http://127.0.0.1:3000`.

## API Usage

### Upload Data
`POST /data`
```json
{
  "asset_id": "uuid...",
  "data": [
    {"timestamp": "2023-01-01T00:00:00Z", "open": 100.0, "high": 105.0, "low": 98.0, "close": 102.0}
  ]
}
```

### Define Strategy
`POST /strategies`
```json
{
  "name": "Moving Average Crossover",
  "entry_rules": [
    {"left": "Price", "operator": "GreaterThan", "right": {"SMA": 20}}
  ],
  "exit_rules": [
    {"left": "Price", "operator": "LessThan", "right": {"SMA": 20}}
  ]
}
```

### Run Backtest
`POST /backtest/{strategy_id}`

## License
Private
