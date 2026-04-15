import yfinance as yf
import requests
import json
import uuid
import time
from datetime import datetime

BASE_URL = "http://127.0.0.1:3000"

def upload_asset(symbol, name, asset_type):
    print(f"Loading {symbol}...")
    ticker = yf.Ticker(symbol)
    df = ticker.history(start="2020-01-01", end="2024-01-01", interval="1d")
    
    asset_id = str(uuid.uuid5(uuid.NAMESPACE_DNS, symbol))
    requests.post(f"{BASE_URL}/assets", json={
        "id": asset_id, "symbol": symbol, "name": name, "asset_type": asset_type
    })
    
    data_points = []
    for timestamp, row in df.iterrows():
        data_points.append({
            "timestamp": timestamp.strftime('%Y-%m-%dT%H:%M:%SZ'),
            "open": float(row['Open']), "high": float(row['High']), "low": float(row['Low']), "close": float(row['Close']),
            "volume": float(row['Volume'])
        })
    
    requests.post(f"{BASE_URL}/data", json={"asset_id": asset_id, "data": data_points})
    return asset_id

def run_complex_demo():
    print("Starting Complex Multi-Asset Demo (RSI Mean Reversion + Monthly Rebalancing)...")
    
    # 1. Assets
    # SPY (Stocks), TLT (Bonds), GLD (Gold)
    spy_id = upload_asset("SPY", "S&P 500 ETF", "ETF")
    tlt_id = upload_asset("TLT", "Treasury Bond ETF", "Bond")
    gld_id = upload_asset("GLD", "Gold Shares", "Commodity")

    # 2. Strategy: RSI Mean Reversion + Monthly Rebalancing
    # Entry: RSI < 30
    # Exit: RSI > 70
    strategy_payload = {
        "id": str(uuid.uuid4()),
        "name": "Multi-Asset RSI Rebalance",
        "entry_rules": [
            {
                "left": {"RSI": 14},
                "operator": "LessThan",
                "right": {"Value": 35.0} # A bit higher than 30 for more signals
            }
        ],
        "exit_rules": [
            {
                "left": {"RSI": 14},
                "operator": "GreaterThan",
                "right": {"Value": 65.0}
            }
        ],
        "rebalance_interval_days": 30
    }
    
    print("Creating Strategy...")
    res = requests.post(f"{BASE_URL}/strategies", json=strategy_payload)
    strategy = res.json()
    
    print(f"Running backtest for strategy {strategy['id']}...")
    res = requests.post(f"{BASE_URL}/backtest/{strategy['id']}")
    
    if res.status_code == 200:
        metrics = res.json()
        print("\n" + "="*50)
        print("COMPLEX BACKTEST RESULTS (2020-2024)")
        print("="*50)
        print(f"Strategy: {strategy['name']}")
        print(f"Total Return: {float(metrics['total_return'])*100:.2f}%")
        print(f"Max Drawdown: {float(metrics['max_drawdown'])*100:.2f}%")
        print(f"Final Value: ${float(metrics['final_value']):,.2f}")
        print("="*50)
    else:
        print(f"Error: {res.text}")

if __name__ == "__main__":
    run_complex_demo()
