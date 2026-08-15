import requests
import time
import sys

BASE_URL = "http://127.0.0.1:3030/api"

def run_tests():
    res = requests.post(f"{BASE_URL}/flows", json={"name": "MSMP Flow"})
    flow_id = res.json()["id"]

    requests.post(f"{BASE_URL}/plugins/load", json={"plugin_name": "plugin_binance", "flow_id": flow_id})
    requests.post(f"{BASE_URL}/plugins/load", json={"plugin_name": "plugin_ohlcv_fetcher", "flow_id": flow_id})
    requests.post(f"{BASE_URL}/plugins/load", json={"plugin_name": "plugin_msmp", "flow_id": flow_id})
    requests.post(f"{BASE_URL}/plugins/load", json={"plugin_name": "plugin_msmp_requester", "flow_id": flow_id})
    
    requests.post(f"{BASE_URL}/systems/{flow_id}_binance_01/start")
    requests.post(f"{BASE_URL}/systems/{flow_id}_plugin_ohlcv_fetcher/start")
    requests.post(f"{BASE_URL}/systems/{flow_id}_plugin_msmp/start")
    requests.post(f"{BASE_URL}/systems/{flow_id}_plugin_msmp_requester/start")
    
    time.sleep(3) # Bekle
    
    res = requests.get(f"{BASE_URL}/systems/{flow_id}_plugin_msmp_requester/data")
    req_data = res.text
    print(f"REQUESTER DATA: {req_data}")

if __name__ == "__main__":
    run_tests()
