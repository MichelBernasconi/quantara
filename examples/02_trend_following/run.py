import yfinance as yf
import requests
import json
import uuid
import time
from datetime import datetime

# URL of the Rust backend
BASE_URL = "http://127.0.0.1:3000"

def wait_for_server():
    print("Waiting for Quantara server to be ready...")
    for _ in range(30):
        try:
            requests.get(f"{BASE_URL}/assets")
            print("Server is UP!")
            return True
        except:
            time.sleep(2)
    return False

def download_and_upload(symbol, name, asset_type):
    print(f"Downloading data for {symbol}...")
    ticker = yf.Ticker(symbol)
    df = ticker.history(start="2018-01-01", end="2024-01-01", interval="1d")
    
    asset_id = str(uuid.uuid5(uuid.NAMESPACE_DNS, symbol))
    
    # 1. Create Asset
    asset_payload = {
        "id": asset_id,
        "symbol": symbol,
        "name": name,
        "asset_type": asset_type
    }
    requests.post(f"{BASE_URL}/assets", json=asset_payload)
    
    # 2. Format and Upload Time Series
    data_points = []
    for timestamp, row in df.iterrows():
        # Ensure the timestamp is in a clean ISO format that chrono can parse
        ts_str = timestamp.strftime('%Y-%m-%dT%H:%M:%SZ')
        data_points.append({
            "timestamp": ts_str,
            "open": float(row['Open']),
            "high": float(row['High']),
            "low": float(row['Low']),
            "close": float(row['Close']),
            "volume": float(row['Volume'])
        })
    
    print(f"Uploading {len(data_points)} data points for {symbol}...")
    res = requests.post(f"{BASE_URL}/data", json={
        "asset_id": asset_id,
        "data": data_points
    })
    if res.status_code != 200:
        print(f"Upload failed: {res.status_code} - {res.text}")
        return None
    
    print(f"Upload successful!")
    return asset_id

def run_simulation():
    if not wait_for_server():
        print("Server timed out.")
        return

    # Download SPY (Stocks) and TLT (Bonds)
    spy_id = download_and_upload("SPY", "SPDR S&P 500 ETF Trust", "ETF")
    # tlt_id = download_and_upload("TLT", "iShares 20+ Year Treasury Bond ETF", "Bond")

    # Define a Strategy: Trend Following on SPY
    # Rule: Enter if Price > SMA(200), Exit if Price < SMA(200)
    strategy_payload = {
        "id": str(uuid.uuid4()),
        "name": "SPY Trend Following (SMA 200)",
        "entry_rules": [
            {
                "left": "Price",
                "operator": "GreaterThan",
                "right": {"SMA": 200}
            }
        ],
        "exit_rules": [
            {
                "left": "Price",
                "operator": "LessThan",
                "right": {"SMA": 200}
            }
        ],
        "rebalance_interval_days": None
    }
    
    print("Creating strategy...")
    res = requests.post(f"{BASE_URL}/strategies", json=strategy_payload)
    strategy = res.json()
    strategy_id = strategy['id']
    
    print(f"Running backtest for strategy {strategy_id}...")
    res = requests.post(f"{BASE_URL}/backtest/{strategy_id}")
    
    if res.status_code == 200:
        metrics = res.json()
        print("\n" + "="*40)
        print("BACKTEST RESULTS (2018-2024)")
        print("="*40)
        print(f"Strategy: {strategy['name']}")
        print(f"Total Return: {float(metrics['total_return'])*100:.2f}%")
        print(f"Max Drawdown: {float(metrics['max_drawdown'])*100:.2f}%")
        print(f"Final Value: ${float(metrics['final_value']):,.2f}")
        print("="*40)
    else:
        print(f"Error running backtest: {res.text}")

if __name__ == "__main__":
    run_simulation()
