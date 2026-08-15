use orchestrator::endpoint::StandardEndpoint;
use orchestrator::system::{EndpointHandler, System, SystemContext};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct OhlcvFetcherSystem {
    ctx: SystemContext,
    endpoints: HashMap<StandardEndpoint, EndpointHandler>,
}

impl OhlcvFetcherSystem {
    pub fn new() -> Self {
        let ctx = SystemContext::new("plugin_ohlcv_fetcher", "OHLCV Fetcher (Provider)");
        let mut endpoints = HashMap::new();

        let outbox = Arc::new(Mutex::new(Vec::<Value>::new()));
        let last_log = Arc::new(Mutex::new(String::from("Bekleniyor... İstek gelmedi.")));

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::Start,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                *ctx_clone.is_running.write().unwrap() = true;
                Ok(b"FETCHER_STARTED".to_vec())
            }) as EndpointHandler,
        );

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::Stop,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                *ctx_clone.is_running.write().unwrap() = false;
                Ok(b"FETCHER_STOPPED".to_vec())
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
        endpoints.insert(
            StandardEndpoint::Inbox,
            Arc::new(move |_ctx: &SystemContext, payload: Option<Vec<u8>>| {
                if let Some(data) = payload {
                    if let Ok(msg) = serde_json::from_slice::<Value>(&data) {
                        if msg["type"].as_str() == Some("REQUEST") {
                            let msg_id = msg["msg_id"].as_str().unwrap_or("").to_string();
                            let from = msg["from"].as_str().unwrap_or("").to_string();
                            
                            if let Some(payload) = msg["payload"].as_object() {
                                let symbol = payload.get("symbol").and_then(|v| v.as_str()).unwrap_or("BTCUSDT").to_string();
                                let interval = payload.get("interval").and_then(|v| v.as_str()).unwrap_or("1m").to_string();
                                let limit = payload.get("limit").and_then(|v| v.as_u64()).unwrap_or(100);

                                let ob = outbox_clone.clone();
                                let log_c = log_clone.clone();
                                
                                std::thread::spawn(move || {
                                    let url = format!("https://fapi.binance.com/fapi/v1/klines?symbol={}&interval={}&limit={}", symbol, interval, limit);
                                    let client = reqwest::blocking::Client::new();
                                    
                                    *log_c.lock().unwrap() = format!("İstek alındı. {} {} {} bar çekiliyor...", symbol, interval, limit);
                                    
                                    match client.get(&url).send() {
                                        Ok(resp) => {
                                            if let Ok(klines) = resp.json::<Value>() {
                                                *log_c.lock().unwrap() = format!("Başarılı! {} {} {} bar çekildi.", symbol, interval, limit);
                                                
                                                let response_msg = json!({
                                                    "msg_id": msg_id,
                                                    "from": "plugin_ohlcv_fetcher",
                                                    "to": from,
                                                    "type": "RESPONSE",
                                                    "payload": {
                                                        "status": "success",
                                                        "data": klines
                                                    }
                                                });
                                                ob.lock().unwrap().push(response_msg);
                                            } else {
                                                *log_c.lock().unwrap() = format!("Hata: JSON ayrıştırılamadı ({})", symbol);
                                            }
                                        }
                                        Err(e) => {
                                            *log_c.lock().unwrap() = format!("Hata: Bağlantı sorunu ({})", e);
                                        }
                                    }
                                });
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
                let l = log_clone.lock().unwrap();
                let out = format!("OHLCV FETCHER (PROVIDER)\n========================\n\n{}", l);
                Ok(out.into_bytes())
            }) as EndpointHandler,
        );

        Self { ctx, endpoints }
    }
}

impl System for OhlcvFetcherSystem {
    fn id(&self) -> &str { &self.ctx.id }
    fn name(&self) -> &str { &self.ctx.name }
    fn endpoints(&self) -> &HashMap<StandardEndpoint, EndpointHandler> { &self.endpoints }
    fn context(&self) -> &SystemContext { &self.ctx }
}

#[no_mangle]
pub extern "C" fn create_plugin() -> *mut Box<dyn System> {
    let sys: Box<dyn System> = Box::new(OhlcvFetcherSystem::new());
    Box::into_raw(Box::new(sys))
}
