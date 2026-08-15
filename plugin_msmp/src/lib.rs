use orchestrator::endpoint::StandardEndpoint;
use orchestrator::system::{EndpointHandler, System, SystemContext};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::str::FromStr;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Kline {
    pub open_time: u64,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
    pub close_time: u64,
    pub quote_asset_volume: Decimal,
    pub number_of_trades: u64,
    pub taker_buy_base_asset_volume: Decimal,
    pub taker_buy_quote_asset_volume: Decimal,
}

pub mod session;
pub mod pivot;
pub mod trend;
pub mod levels;
pub mod liquidity;
pub mod imbalance;
pub mod narrative;

#[derive(Clone)]
struct ActiveRequest {
    requester_id: String,
    symbol: String,
    interval: String,
    limit: u64,
    klines: Option<Vec<Kline>>,
    last_price: Option<String>,
}

pub struct MsmpSystem {
    ctx: SystemContext,
    endpoints: HashMap<StandardEndpoint, EndpointHandler>,
}

impl MsmpSystem {
    pub fn new() -> Self {
        let ctx = SystemContext::new("plugin_msmp", "MSMP Analytics Engine");
        let mut endpoints = HashMap::new();

        let outbox = Arc::new(Mutex::new(Vec::<Value>::new()));
        let last_log = Arc::new(RwLock::new(String::from("Bekleniyor... Servis baslatilmadi.")));
        let active_requests: Arc<Mutex<HashMap<String, ActiveRequest>>> = Arc::new(Mutex::new(HashMap::new()));

        let ctx_clone = ctx.clone();
        let log_start = last_log.clone();
        endpoints.insert(
            StandardEndpoint::Start,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                *ctx_clone.is_running.write().unwrap() = true;
                *log_start.write().unwrap() = String::from("Servis başlatıldı. Veri bekleniyor...");
                Ok(b"MSMP_STARTED".to_vec())
            }) as EndpointHandler,
        );

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::Stop,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                *ctx_clone.is_running.write().unwrap() = false;
                Ok(b"MSMP_STOPPED".to_vec())
            }) as EndpointHandler,
        );

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::IsWorking,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                let status = *ctx_clone.is_running.read().unwrap();
                Ok(vec![if status { 1u8 } else { 0u8 }])
            }) as EndpointHandler,
        );

        let outbox_clone = outbox.clone();
        endpoints.insert(
            StandardEndpoint::Outbox,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                let mut ob = outbox_clone.lock().unwrap();
                if ob.is_empty() {
                    Ok(vec![])
                } else {
                    let json_arr = serde_json::to_string(&*ob).unwrap_or_default();
                    ob.clear();
                    Ok(json_arr.into_bytes())
                }
            }) as EndpointHandler,
        );

        let outbox_clone = outbox.clone();
        let log_clone = last_log.clone();
        let requests_clone = active_requests.clone();
        endpoints.insert(
            StandardEndpoint::Inbox,
            Arc::new(move |_ctx: &SystemContext, payload: Option<Vec<u8>>| {
                if let Some(data) = payload {
                    if let Ok(msg) = serde_json::from_slice::<Value>(&data) {
                        let msg_type = msg["type"].as_str().unwrap_or("");
                        let msg_id = msg["msg_id"].as_str().unwrap_or("").to_string();
                        let from = msg["from"].as_str().unwrap_or("").to_string();

                        if msg_type == "REQUEST" {
                            // İstek requester'dan geliyor
                            if let Some(payload_obj) = msg["payload"].as_object() {
                                let symbol = payload_obj.get("symbol").and_then(|v| v.as_str()).unwrap_or("BTCUSDT").to_string();
                                let interval = payload_obj.get("interval").and_then(|v| v.as_str()).unwrap_or("1m").to_string();
                                let limit = payload_obj.get("limit").and_then(|v| v.as_u64()).unwrap_or(100);

                                *log_clone.write().unwrap() = format!("{} adresinden {} için MSMP analiz isteği alındı.", from, symbol);

                                // Yeni bir istek kaydet
                                requests_clone.lock().unwrap().insert(msg_id.clone(), ActiveRequest {
                                    requester_id: from,
                                    symbol: symbol.clone(),
                                    interval: interval.clone(),
                                    limit,
                                    klines: None,
                                    last_price: None,
                                });

                                // OHLCV fetcher'a istek at
                                let ohlcv_req = json!({
                                    "msg_id": msg_id,
                                    "from": "plugin_msmp",
                                    "to": "plugin_ohlcv_fetcher",
                                    "type": "REQUEST",
                                    "payload": {
                                        "symbol": symbol,
                                        "interval": interval,
                                        "limit": 1500 // Maksimum gerekli olan (amp = limit * 4 veya max 1500)
                                    }
                                });
                                outbox_clone.lock().unwrap().push(ohlcv_req);

                                // Binance plugin'e son fiyat (lastprice) isteği at
                                let price_req = json!({
                                    "msg_id": msg_id,
                                    "from": "plugin_msmp",
                                    "to": "binance_01",
                                    "type": "REQUEST",
                                    "payload": {
                                        "symbol": symbol
                                    }
                                });
                                outbox_clone.lock().unwrap().push(price_req);
                            }
                        } else if msg_type == "RESPONSE" {
                            if from == "plugin_ohlcv_fetcher" {
                                if let Some(req) = requests_clone.lock().unwrap().get_mut(&msg_id) {
                                    if let Some(payload_obj) = msg.get("payload") {
                                        if let Some(arr) = payload_obj["data"].as_array() {
                                            let mut klines = Vec::new();
                                            for item in arr {
                                                if let Some(k) = item.as_array() {
                                                    let kline = Kline {
                                                        open_time: k[0].as_u64().unwrap_or(0),
                                                        open: Decimal::from_str(k[1].as_str().unwrap_or("0")).unwrap_or_default(),
                                                        high: Decimal::from_str(k[2].as_str().unwrap_or("0")).unwrap_or_default(),
                                                        low: Decimal::from_str(k[3].as_str().unwrap_or("0")).unwrap_or_default(),
                                                        close: Decimal::from_str(k[4].as_str().unwrap_or("0")).unwrap_or_default(),
                                                        volume: Decimal::from_str(k[5].as_str().unwrap_or("0")).unwrap_or_default(),
                                                        close_time: k[6].as_u64().unwrap_or(0),
                                                        quote_asset_volume: Decimal::from_str(k[7].as_str().unwrap_or("0")).unwrap_or_default(),
                                                        number_of_trades: k[8].as_u64().unwrap_or(0),
                                                        taker_buy_base_asset_volume: Decimal::from_str(k[9].as_str().unwrap_or("0")).unwrap_or_default(),
                                                        taker_buy_quote_asset_volume: Decimal::from_str(k[10].as_str().unwrap_or("0")).unwrap_or_default(),
                                                    };
                                                    klines.push(kline);
                                                }
                                            }
                                            req.klines = Some(klines);
                                            *log_clone.write().unwrap() = format!("{} için OHLCV verisi geldi.", req.symbol);
                                        }
                                    }
                                }
                            } else if from == "plugin_binance" {
                                if let Some(req) = requests_clone.lock().unwrap().get_mut(&msg_id) {
                                    if let Some(payload_obj) = msg.get("payload") {
                                        if let Some(price) = payload_obj.get("price").and_then(|v| v.as_str()) {
                                            req.last_price = Some(price.to_string());
                                            *log_clone.write().unwrap() = format!("{} için Binance LastPrice ({}) geldi.", req.symbol, price);
                                        }
                                    }
                                }
                            }

                            // Her iki veri de geldiyse raporu oluştur
                            let mut req_to_process = None;
                            {
                                let mut locks = requests_clone.lock().unwrap();
                                if let Some(req) = locks.get(&msg_id) {
                                    if req.klines.is_some() && req.last_price.is_some() {
                                        req_to_process = Some(req.clone());
                                    }
                                }
                                if req_to_process.is_some() {
                                    locks.remove(&msg_id);
                                }
                            }

                            if let Some(req) = req_to_process {
                                let all_klines = req.klines.unwrap();
                                if all_klines.is_empty() {
                                    *log_clone.write().unwrap() = format!("{} için OHLCV verisi boş!", req.symbol);
                                } else {
                                    // 3 pencereye böl
                                    let core_limit = req.limit as usize;
                                    let amp_limit = (req.limit as usize * 4).min(1500);
                                    let acute_limit = 96;

                                    let core = if all_klines.len() >= core_limit { &all_klines[all_klines.len() - core_limit..] } else { &all_klines };
                                    let amp = if all_klines.len() >= amp_limit { &all_klines[all_klines.len() - amp_limit..] } else { &all_klines };
                                    let acute = if all_klines.len() >= acute_limit { &all_klines[all_klines.len() - acute_limit..] } else { &all_klines };

                                    let mut report = narrative::generate_report(core, amp, acute);
                                    
                                    // Binance plugin'inden gelen fiyatı rapora ekle
                                    if let Some(price_str) = req.last_price {
                                        if let Ok(p) = Decimal::from_str(&price_str) {
                                            if p > Decimal::ZERO {
                                                report.current_price = p; // Binance'den gelen canlı veriyi ez
                                            }
                                        }
                                    }

                                    // Sonucu Requester'a gönder
                                    let response_msg = json!({
                                        "msg_id": msg_id,
                                        "from": "plugin_msmp",
                                        "to": req.requester_id,
                                        "type": "RESPONSE",
                                        "payload": {
                                            "status": "success",
                                            "data": report
                                        }
                                    });
                                    outbox_clone.lock().unwrap().push(response_msg);
                                    *log_clone.write().unwrap() = format!("{} için MSMP analizi tamamlandı ve rapor gönderildi.", req.symbol);
                                }
                            }
                        }
                    }
                }
                Ok(vec![1u8])
            }) as EndpointHandler,
        );

        let log_clone = last_log.clone();
        endpoints.insert(
            StandardEndpoint::DataMonitor,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                let log_msg = log_clone.read().unwrap().clone();
                let out = format!("MSMP ENGINE\n===========\nDurum: {}", log_msg);
                Ok(out.into_bytes())
            }) as EndpointHandler,
        );

        Self { ctx, endpoints }
    }
}

impl System for MsmpSystem {
    fn id(&self) -> &str { &self.ctx.id }
    fn name(&self) -> &str { &self.ctx.name }
    fn endpoints(&self) -> &HashMap<StandardEndpoint, EndpointHandler> { &self.endpoints }
    fn context(&self) -> &SystemContext { &self.ctx }
}

#[no_mangle]
pub extern "C" fn create_plugin() -> *mut Box<dyn System> {
    let sys: Box<dyn System> = Box::new(MsmpSystem::new());
    Box::into_raw(Box::new(sys))
}
