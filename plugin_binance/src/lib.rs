use orchestrator::endpoint::StandardEndpoint;
use orchestrator::system::{EndpointHandler, System, SystemContext};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use futures_util::stream::StreamExt;
use futures_util::SinkExt;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;
use serde_json::{json, Value};

pub struct BinanceSystem {
    ctx: SystemContext,
    endpoints: HashMap<StandardEndpoint, EndpointHandler>,
}

impl BinanceSystem {
    pub fn new() -> Self {
        let ctx = SystemContext::new("binance_01", "Binance Futures Live");
        let mut endpoints = HashMap::new();
        
        let latest_prices: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
        let outbox = Arc::new(Mutex::new(Vec::<Value>::new()));
        
        {
            let mut p = latest_prices.lock().unwrap();
            p.insert("BTCUSDT".to_string(), "0.0".to_string());
            p.insert("ETHUSDT".to_string(), "0.0".to_string());
            p.insert("ACEUSDT".to_string(), "0.0".to_string());
        }

        let ctx_clone = ctx.clone();
        let prices_start = latest_prices.clone();
        endpoints.insert(
            StandardEndpoint::Start,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                let mut running = ctx_clone.is_running.write().unwrap();
                if !*running {
                    *running = true;
                    let is_running = ctx_clone.is_running.clone();
                    let is_data_valid = ctx_clone.is_data_valid.clone();
                    let memory = ctx_clone.memory.clone();
                    let prices = prices_start.clone();
                    
                    std::thread::spawn(move || {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        rt.block_on(async move {
                            let connect_url = Url::parse("wss://fstream.binance.com/public/stream?streams=btcusdt@bookTicker/ethusdt@bookTicker/aceusdt@bookTicker").unwrap();
                            
                            match connect_async(connect_url).await {
                                Ok((mut ws_stream, _)) => {
                                    *is_data_valid.write().unwrap() = true;
                                    
                                    while let Some(msg) = ws_stream.next().await {
                                        if !*is_running.read().unwrap() {
                                            break;
                                        }
                                        
                                        if let Ok(Message::Text(text)) = msg {
                                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                                if let (Some(s), Some(b)) = (json["data"]["s"].as_str(), json["data"]["b"].as_str()) {
                                                    let mut p = prices.lock().unwrap();
                                                    p.insert(s.to_string(), b.to_string());
                                                    
                                                    let btc = p.get("BTCUSDT").unwrap_or(&"0.0".to_string()).clone();
                                                    let eth = p.get("ETHUSDT").unwrap_or(&"0.0".to_string()).clone();
                                                    let ace = p.get("ACEUSDT").unwrap_or(&"0.0".to_string()).clone();
                                                    
                                                    let formatted = format!("BTC: {} | ETH: {} | ACE: {}", btc, eth, ace);
                                                    memory.write(formatted.into_bytes());
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    let err_msg = format!("Websocket hatasi: {}", e);
                                    memory.write(err_msg.into_bytes());
                                }
                            }
                            
                            let mut valid = is_data_valid.write().unwrap();
                            *valid = false;
                        });
                    });
                }
                Ok(b"BINANCE_STARTED".to_vec())
            }) as EndpointHandler,
        );

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::Stop,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                *ctx_clone.is_running.write().unwrap() = false;
                Ok(b"BINANCE_STOPPED".to_vec())
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

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::DataValid,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                let valid = *ctx_clone.is_data_valid.read().unwrap();
                Ok(vec![if valid { 1u8 } else { 0u8 }])
            }) as EndpointHandler,
        );

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::DataMonitor,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                let mut data = ctx_clone.memory.read();
                if data.is_empty() {
                    data = b"BAGLANILIYOR...".to_vec();
                }
                Ok(data)
            }) as EndpointHandler,
        );

        let prices_raw = latest_prices.clone();
        endpoints.insert(
            StandardEndpoint::RawData,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                let p = prices_raw.lock().unwrap();
                let json_str = serde_json::to_string(&*p).unwrap_or_else(|_| "{}".to_string());
                Ok(json_str.into_bytes())
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
        let prices_inbox = latest_prices.clone();
        endpoints.insert(
            StandardEndpoint::Inbox,
            Arc::new(move |_ctx: &SystemContext, payload: Option<Vec<u8>>| {
                if let Some(data) = payload {
                    if let Ok(msg) = serde_json::from_slice::<Value>(&data) {
                        if msg["type"].as_str() == Some("REQUEST") {
                            let msg_id = msg["msg_id"].as_str().unwrap_or("").to_string();
                            let from = msg["from"].as_str().unwrap_or("").to_string();
                            
                            if let Some(payload_obj) = msg["payload"].as_object() {
                                let symbol = payload_obj.get("symbol").and_then(|v| v.as_str()).unwrap_or("BTCUSDT").to_string();
                                
                                let p = prices_inbox.lock().unwrap();
                                let price_str = p.get(&symbol).cloned().unwrap_or_else(|| "0.0".to_string());
                                
                                let response_msg = json!({
                                    "msg_id": msg_id,
                                    "from": "plugin_binance",
                                    "to": from,
                                    "type": "RESPONSE",
                                    "payload": {
                                        "status": "success",
                                        "symbol": symbol,
                                        "price": price_str
                                    }
                                });
                                
                                outbox_clone.lock().unwrap().push(response_msg);
                            }
                        }
                    }
                }
                Ok(vec![1u8])
            }) as EndpointHandler,
        );

        Self { ctx, endpoints }
    }
}

impl System for BinanceSystem {
    fn id(&self) -> &str { &self.ctx.id }
    fn name(&self) -> &str { &self.ctx.name }
    fn endpoints(&self) -> &HashMap<StandardEndpoint, EndpointHandler> { &self.endpoints }
    fn context(&self) -> &SystemContext { &self.ctx }
}

#[no_mangle]
pub extern "C" fn create_plugin() -> *mut Box<dyn System> {
    let sys: Box<dyn System> = Box::new(BinanceSystem::new());
    Box::into_raw(Box::new(sys))
}
