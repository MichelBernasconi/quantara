import requests
import json
import uuid
from datetime import datetime
import random

# Mock data generator for Quantara Backtesting Engine
def generate_mock_data(symbol, days=100):
    asset_id = str(uuid.uuid4())
    data = []
    current_price = 150.0
    
    start_date = 1672531200 # 2023-01-01
    
    for i in range(days):
        timestamp = datetime.fromtimestamp(start_date + i * 86400).isoformat() + "Z"
        change = random.uniform(-0.02, 0.02)
        open_p = current_price
        close_p = open_p * (1 + change)
        high_p = max(open_p, close_p) * (1 + random.uniform(0, 0.01))
        low_p = min(open_p, close_p) * (1 - random.uniform(0, 0.01))
        
        data.append({
            "timestamp": timestamp,
            "open": round(open_p, 2),
            "high": round(high_p, 2),
            "low": round(low_p, 2),
            "close": round(close_p, 2),
            "volume": str(random.randint(1000, 1000000))
        })
        current_price = close_p
        
    return {
        "asset_id": asset_id,
        "symbol": symbol,
        "data": data
    }

def test_api():
    base_url = "http://127.0.0.1:3000"
    
    # 1. Create Asset
    mock_asset = generate_mock_data("AAPL", 100)
    asset_payload = {
        "id": mock_asset["asset_id"],
        "symbol": mock_asset["symbol"],
        "name": "Apple Inc.",
        "asset_type": "Stock"
    }
    
    print(f"Creating asset {mock_asset['symbol']}...")
    res = requests.post(f"{base_url}/assets", json=asset_payload)
    print(res.text)

    # 2. Upload Data
    print("Uploading time series data...")
    res = requests.post(f"{base_url}/data", json={"asset_id": mock_asset["asset_id"], "data": mock_asset["data"]})
    print(res.text)

    # 3. Create Strategy
    strategy_payload = {
        "id": str(uuid.uuid4()),
        "name": "SMA 20 Crossover",
        "entry_rules": [
            {
                "left": "Price",
                "operator": "GreaterThan",
                "right": {"SMA": 20}
            }
        ],
        "exit_rules": [
            {
                "left": "Price",
                "operator": "LessThan",
                "right": {"SMA": 20}
            }
        ],
        "rebalance_interval_days": None
    }
    print("Creating strategy...")
    res = requests.post(f"{base_url}/strategies", json=strategy_payload)
    strategy = res.json()
    print(f"Strategy created with ID: {strategy['id']}")

    # 4. Run Backtest
    print("Running backtest...")
    res = requests.post(f"{base_url}/backtest/{strategy['id']}")
    print("Metrics:")
    print(json.dumps(res.json(), indent=2))

if __name__ == "__main__":
    try:
        test_api()
    except Exception as e:
        print(f"Error: {e}. Make sure the Rust server is running on http://127.0.0.1:3000")
