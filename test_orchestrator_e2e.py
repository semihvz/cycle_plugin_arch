import requests
import time
import sys

BASE_URL = "http://127.0.0.1:3030/api"

def print_step(step_num, desc):
    print(f"\n[Adım {step_num}] {desc}")

def run_tests():
    try:
        # 0. Başlangıç Kontrolü
        print_step(0, "Sunucu ayakta mı?")
        res = requests.get(f"{BASE_URL}/sysinfo")
        assert res.status_code == 200, "Sunucuya bağlanılamadı. Lütfen cargo run ile sunucuyu başlatın."
        print("✅ Sunucu ayakta.")

        # 1. Flow API Testi
        print_step(1, "Yeni Alan (Flow) Oluşturma Testi")
        res = requests.post(f"{BASE_URL}/flows", json={"name": "Test Flow"})
        assert res.status_code == 200
        flow_data = res.json()
        flow_id = flow_data["id"]
        assert "flow_" in flow_id
        assert flow_data["name"] == "Test Flow"
        print(f"✅ Flow oluşturuldu: {flow_id}")

        res = requests.get(f"{BASE_URL}/flows")
        flows = res.json()
        assert any(f["id"] == flow_id for f in flows)
        print("✅ Flow başarıyla listelendi.")

        # 2. Plugin Load API Testi
        print_step(2, "Eklenti (Plugin) Yükleme Testi")
        res = requests.post(f"{BASE_URL}/plugins/load", json={"plugin_name": "plugin_binance", "flow_id": flow_id})
        assert res.status_code == 200
        assert res.json()["status"] == "success", f"Hata: {res.text}"
        
        res = requests.post(f"{BASE_URL}/plugins/load", json={"plugin_name": "plugin_msmp", "flow_id": flow_id})
        assert res.status_code == 200
        assert res.json()["status"] == "success"
        
        print("✅ Eklentiler başarıyla yüklendi.")

        # 3. System List & Ports Testi
        print_step(3, "Sistem Portları ve Instance ID Doğrulama")
        res = requests.get(f"{BASE_URL}/systems")
        systems = res.json()
        binance_sys = next(s for s in systems if s["id"] == f"{flow_id}_binance_01")
        msmp_sys = next(s for s in systems if s["id"] == f"{flow_id}_plugin_msmp")
        
        assert binance_sys is not None
        assert msmp_sys is not None
        assert "input_ports" in binance_sys
        assert "output_ports" in binance_sys
        print(f"✅ Sistemler port bilgileriyle listelendi. Binance: {binance_sys['id']}")

        # 4. Wiring (Links) API Testi
        print_step(4, "Port-to-Port Bağlantı (Wiring) Testi")
        link_req = {
            "source": binance_sys["id"],
            "source_port": "out_trades",
            "target": msmp_sys["id"],
            "target_port": "in_market_data"
        }
        res = requests.post(f"{BASE_URL}/links", json=link_req)
        assert res.status_code == 200
        link_id = res.json()["id"]
        assert "link_" in link_id
        
        res = requests.get(f"{BASE_URL}/links")
        links = res.json()
        assert any(l["id"] == link_id for l in links)
        print(f"✅ Link başarıyla oluşturuldu: {link_id}")

        # 5. System Start Testi
        print_step(5, "Eklentileri Başlatma Testi")
        requests.post(f"{BASE_URL}/systems/{binance_sys['id']}/start")
        requests.post(f"{BASE_URL}/systems/{msmp_sys['id']}/start")
        
        time.sleep(1) # Başlamaları için bekle
        
        res = requests.get(f"{BASE_URL}/systems")
        assert next(s for s in res.json() if s["id"] == binance_sys["id"])["running"] == True
        print("✅ Eklentiler başarıyla çalıştırıldı.")

        # 6. Delete Link & System & Flow Testi
        print_step(6, "Temizlik (Delete API) Testi")
        res = requests.delete(f"{BASE_URL}/links/{link_id}")
        assert res.status_code == 200
        assert not any(l["id"] == link_id for l in requests.get(f"{BASE_URL}/links").json())
        print("✅ Link silindi.")

        requests.delete(f"{BASE_URL}/systems/{binance_sys['id']}")
        requests.delete(f"{BASE_URL}/systems/{msmp_sys['id']}")
        assert not any(s["id"] == binance_sys["id"] for s in requests.get(f"{BASE_URL}/systems").json())
        print("✅ Eklentiler (Node'lar) silindi.")

        requests.delete(f"{BASE_URL}/flows/{flow_id}")
        assert not any(f["id"] == flow_id for f in requests.get(f"{BASE_URL}/flows").json())
        print("✅ Flow (Alan) silindi.")

        print("\n🚀 TÜM ENTEGRASYON TESTLERİ BAŞARIYLA TAMAMLANDI! (PASSED)")

    except AssertionError as e:
        print(f"\n❌ TEST BAŞARISIZ (FAILED): {e}")
        sys.exit(1)
    except Exception as e:
        print(f"\n❌ BEKLENMEYEN HATA: {e}")
        sys.exit(1)

if __name__ == "__main__":
    run_tests()
