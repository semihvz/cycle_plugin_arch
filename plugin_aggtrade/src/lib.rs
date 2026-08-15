use orchestrator::endpoint::StandardEndpoint;
use orchestrator::system::{EndpointHandler, System, SystemContext};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use futures_util::stream::StreamExt;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

struct TradeBuffer {
    queue: VecDeque<String>,
    current_bytes: usize,
    max_bytes: usize,
}

pub struct AggTradeSystem {
    ctx: SystemContext,
    endpoints: HashMap<StandardEndpoint, EndpointHandler>,
}

impl AggTradeSystem {
    pub fn new() -> Self {
        let ctx = SystemContext::new("aggtrade_01", "Binance AggTrade");
        let mut endpoints = HashMap::new();
        
        let latest_trades: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
        let outbox_queue: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        
        // 10 MB limit (10 * 1024 * 1024 = 10_485_760 bytes)
        let ring_buffer = Arc::new(Mutex::new(TradeBuffer {
            queue: VecDeque::new(),
            current_bytes: 0,
            max_bytes: 10 * 1024 * 1024, 
        }));
        
        {
            let mut p = latest_trades.lock().unwrap();
            p.insert("BTCUSDT".to_string(), "p=0 q=0 m=false".to_string());
            p.insert("ETHUSDT".to_string(), "p=0 q=0 m=false".to_string());
            p.insert("ACEUSDT".to_string(), "p=0 q=0 m=false".to_string());
        }

        let ctx_clone = ctx.clone();
        let trades_start = latest_trades.clone();
        let buffer_start = ring_buffer.clone();
        let outbox_queue_clone = outbox_queue.clone();
        
        endpoints.insert(
            StandardEndpoint::Start,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                let mut running = ctx_clone.is_running.write().unwrap();
                if !*running {
                    *running = true;
                    let is_running = ctx_clone.is_running.clone();
                    let is_data_valid = ctx_clone.is_data_valid.clone();
                    let memory = ctx_clone.memory.clone();
                    let trades = trades_start.clone();
                    let ring = buffer_start.clone();
                    let outbox = outbox_queue_clone.clone();
                    
                    std::thread::spawn(move || {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        rt.block_on(async move {
                            let connect_url = Url::parse("wss://fstream.binance.com/market/stream?streams=btcusdt@aggTrade/ethusdt@aggTrade/aceusdt@aggTrade").unwrap();
                            
                            match connect_async(connect_url).await {
                                Ok((mut ws_stream, _)) => {
                                    *is_data_valid.write().unwrap() = true;
                                    
                                    while let Some(msg) = ws_stream.next().await {
                                        if !*is_running.read().unwrap() {
                                            break;
                                        }
                                        
                                        if let Ok(Message::Text(text)) = msg {
                                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                                if let (Some(s), Some(p), Some(q), Some(m)) = (
                                                    json["data"]["s"].as_str(),
                                                    json["data"]["p"].as_str(),
                                                    json["data"]["q"].as_str(),
                                                    json["data"]["m"].as_bool()
                                                ) {
                                                    let trade_info = format!("p={} q={} m={}", p, q, m);
                                                    
                                                    // Buffer logic
                                                    let mut buf = ring.lock().unwrap();
                                                    let log_line = format!("{} | {}\n", s, trade_info);
                                                    buf.current_bytes += log_line.len();
                                                    buf.queue.push_back(log_line);
                                                    
                                                    // Sınır dolunca ilk giren veriyi sil (FIFO)
                                                    while buf.current_bytes > buf.max_bytes {
                                                        if let Some(removed) = buf.queue.pop_front() {
                                                            buf.current_bytes -= removed.len();
                                                            let msg = serde_json::json!({
                                                                "to": "timescaledb_01",
                                                                "type": "aggtrade",
                                                                "data": removed
                                                            });
                                                            outbox.lock().unwrap().push(serde_json::to_string(&msg).unwrap());
                                                        } else {
                                                            break;
                                                        }
                                                    }
                                                    
                                                    let buffer_status = format!("RAM BUFFER: {:.2} MB / 10.00 MB ({} trades)", 
                                                        buf.current_bytes as f64 / 1_048_576.0, buf.queue.len());
                                                        
                                                    // UI logic
                                                    let mut tr = trades.lock().unwrap();
                                                    
                                                    // Parse existing count if any
                                                    let mut current_count: u64 = 0;
                                                    if let Some(existing) = tr.get(s) {
                                                        if let Some(c_start) = existing.find("c=") {
                                                            if let Ok(c) = existing[c_start+2..].parse::<u64>() {
                                                                current_count = c;
                                                            }
                                                        }
                                                    }
                                                    current_count += 1;
                                                    
                                                    let trade_info = format!("p={} q={} m={} c={}", p, q, m, current_count);
                                                    
                                                    tr.insert(s.to_string(), trade_info);
                                                    
                                                    let btc = tr.get("BTCUSDT").unwrap_or(&"p=0 q=0 m=false c=0".to_string()).clone();
                                                    let eth = tr.get("ETHUSDT").unwrap_or(&"p=0 q=0 m=false c=0".to_string()).clone();
                                                    let ace = tr.get("ACEUSDT").unwrap_or(&"p=0 q=0 m=false c=0".to_string()).clone();
                                                    
                                                    let formatted = format!("{}\n\nBTC: [{}]\nETH: [{}]\nACE: [{}]", buffer_status, btc, eth, ace);
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
                Ok(b"AGGTRADE_STARTED".to_vec())
            }) as EndpointHandler,
        );

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::Stop,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                *ctx_clone.is_running.write().unwrap() = false;
                Ok(b"AGGTRADE_STOPPED".to_vec())
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

        let trades_raw = latest_trades.clone();
        endpoints.insert(
            StandardEndpoint::RawData,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                let tr = trades_raw.lock().unwrap();
                let json = serde_json::to_string(&*tr).unwrap_or_default();
                Ok(json.into_bytes())
            }) as EndpointHandler,
        );

        let outbox_queue_endpoint = outbox_queue.clone();
        endpoints.insert(
            StandardEndpoint::Outbox,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                let mut outbox = outbox_queue_endpoint.lock().unwrap();
                if outbox.is_empty() {
                    Ok(vec![])
                } else {
                    let json = serde_json::to_string(&*outbox).unwrap_or_default();
                    outbox.clear();
                    Ok(json.into_bytes())
                }
            }) as EndpointHandler,
        );

        Self { ctx, endpoints }
    }
}

impl System for AggTradeSystem {
    fn id(&self) -> &str { &self.ctx.id }
    fn name(&self) -> &str { &self.ctx.name }
    fn endpoints(&self) -> &HashMap<StandardEndpoint, EndpointHandler> { &self.endpoints }
    fn context(&self) -> &SystemContext { &self.ctx }
}

#[no_mangle]
pub extern "C" fn create_plugin() -> *mut Box<dyn System> {
    let sys: Box<dyn System> = Box::new(AggTradeSystem::new());
    Box::into_raw(Box::new(sys))
}
