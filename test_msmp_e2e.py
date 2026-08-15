import requests
import time
import sys

BASE_URL = "http://127.0.0.1:3030/api"

def print_step(step_num, desc):
    print(f"\n[Adım {step_num}] {desc}")

def run_tests():
    try:
        # 0. Başlangıç Kontrolü
        res = requests.get(f"{BASE_URL}/sysinfo")
        assert res.status_code == 200, "Sunucuya bağlanılamadı. Lütfen cargo run ile sunucuyu başlatın."

        # 1. Flow API Testi
        res = requests.post(f"{BASE_URL}/flows", json={"name": "MSMP Flow"})
        flow_id = res.json()["id"]

        # 2. Plugin Load API Testi
        requests.post(f"{BASE_URL}/plugins/load", json={"plugin_name": "plugin_binance", "flow_id": flow_id})
        requests.post(f"{BASE_URL}/plugins/load", json={"plugin_name": "plugin_ohlcv_fetcher", "flow_id": flow_id})
        requests.post(f"{BASE_URL}/plugins/load", json={"plugin_name": "plugin_msmp", "flow_id": flow_id})
        requests.post(f"{BASE_URL}/plugins/load", json={"plugin_name": "plugin_msmp_requester", "flow_id": flow_id})
        
        # 3. System Start Testi
        requests.post(f"{BASE_URL}/systems/{flow_id}_binance_01/start")
        requests.post(f"{BASE_URL}/systems/{flow_id}_plugin_ohlcv_fetcher/start")
        requests.post(f"{BASE_URL}/systems/{flow_id}_plugin_msmp/start")
        requests.post(f"{BASE_URL}/systems/{flow_id}_plugin_msmp_requester/start")
        
        time.sleep(2) # Bekle
        
        res = requests.get(f"{BASE_URL}/systems/{flow_id}_plugin_msmp/data")
        msmp_data = res.text
        print(msmp_data)

        # Check if msmp received request
        assert "MSMP analiz isteği alındı" in msmp_data, f"Data is missing request log: {msmp_data}"

        print("\n🚀 MSMP TEST BAŞARIYLA TAMAMLANDI! (PASSED)")

    except Exception as e:
        print(f"\n❌ BEKLENMEYEN HATA: {e}")
        sys.exit(1)

if __name__ == "__main__":
    run_tests()
