use orchestrator::endpoint::StandardEndpoint;
use orchestrator::system::{EndpointHandler, System, SystemContext};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use futures_util::stream::StreamExt;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

struct LiquidationBuffer {
    queue: VecDeque<String>,
    current_bytes: usize,
    max_bytes: usize,
}

pub struct LiquidationSystem {
    ctx: SystemContext,
    endpoints: HashMap<StandardEndpoint, EndpointHandler>,
}

impl LiquidationSystem {
    pub fn new() -> Self {
        let ctx = SystemContext::new("liq_01", "Binance Liquidations");
        let mut endpoints = HashMap::new();
        
        let latest_liquidations: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
        
        // 10 MB limit (10 * 1024 * 1024 = 10_485_760 bytes)
        let ring_buffer = Arc::new(Mutex::new(LiquidationBuffer {
            queue: VecDeque::new(),
            current_bytes: 0,
            max_bytes: 10 * 1024 * 1024, 
        }));
        
        {
            let mut p = latest_liquidations.lock().unwrap();
            p.insert("BTCUSDT".to_string(), "Henuz likidasyon yok".to_string());
            p.insert("ETHUSDT".to_string(), "Henuz likidasyon yok".to_string());
            p.insert("ACEUSDT".to_string(), "Henuz likidasyon yok".to_string());
        }

        let ctx_clone = ctx.clone();
        let liq_start = latest_liquidations.clone();
        let buffer_start = ring_buffer.clone();
        
        endpoints.insert(
            StandardEndpoint::Start,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                let mut running = ctx_clone.is_running.write().unwrap();
                if !*running {
                    *running = true;
                    let is_running = ctx_clone.is_running.clone();
                    let is_data_valid = ctx_clone.is_data_valid.clone();
                    let memory = ctx_clone.memory.clone();
                    let liqs = liq_start.clone();
                    let ring = buffer_start.clone();
                    
                    std::thread::spawn(move || {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        rt.block_on(async move {
                            let connect_url = Url::parse("wss://fstream.binance.com/stream?streams=btcusdt@forceOrder/ethusdt@forceOrder/aceusdt@forceOrder").unwrap();
                            
                            match connect_async(connect_url).await {
                                Ok((mut ws_stream, _)) => {
                                    *is_data_valid.write().unwrap() = true;
                                    
                                    // İlk UI çıktısı (Henüz likidasyon gelmeden önce)
                                    let update_ui = || {
                                        let tr = liqs.lock().unwrap();
                                        let buf = ring.lock().unwrap();
                                        let btc = tr.get("BTCUSDT").unwrap_or(&"Yok".to_string()).clone();
                                        let eth = tr.get("ETHUSDT").unwrap_or(&"Yok".to_string()).clone();
                                        let ace = tr.get("ACEUSDT").unwrap_or(&"Yok".to_string()).clone();
                                        
                                        let buffer_status = format!("RAM BUFFER: {:.2} MB / 10.00 MB ({} liquidations)", 
                                            buf.current_bytes as f64 / 1_048_576.0, buf.queue.len());
                                            
                                        let mut formatted = format!("{}\n\nSon Likidasyonlar:\nBTC: [{}]\nETH: [{}]\nACE: [{}]\n\nTum Gecmis:\n", buffer_status, btc, eth, ace);
                                        
                                        if buf.queue.is_empty() {
                                            formatted.push_str("(Likidasyon bekleniyor, piyasa su an sakin...)\n");
                                        } else {
                                            for line in buf.queue.iter().rev().take(10) {
                                                formatted.push_str(line);
                                            }
                                        }
                                        memory.write(formatted.into_bytes());
                                    };
                                    
                                    update_ui();
                                    
                                    while let Some(msg) = ws_stream.next().await {
                                        if !*is_running.read().unwrap() {
                                            break;
                                        }
                                        
                                        if let Ok(Message::Text(text)) = msg {
                                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                                if let Some(o) = json["data"]["o"].as_object() {
                                                    if let (Some(s), Some(side), Some(q), Some(p)) = (
                                                        o.get("s").and_then(|v| v.as_str()),
                                                        o.get("S").and_then(|v| v.as_str()),
                                                        o.get("q").and_then(|v| v.as_str()),
                                                        o.get("p").and_then(|v| v.as_str())
                                                    ) {
                                                        let liq_info = format!("{} {} @ {}", side, q, p);
                                                        
                                                        // Buffer logic
                                                        {
                                                            let mut buf = ring.lock().unwrap();
                                                            let log_line = format!("LIQUIDATION: {} | {}\n", s, liq_info);
                                                            buf.current_bytes += log_line.len();
                                                            buf.queue.push_back(log_line.clone());
                                                            
                                                            while buf.current_bytes > buf.max_bytes {
                                                                if let Some(removed) = buf.queue.pop_front() {
                                                                    buf.current_bytes -= removed.len();
                                                                } else {
                                                                    break;
                                                                }
                                                            }
                                                        }
                                                        
                                                        // Update internal state
                                                        {
                                                            let mut tr = liqs.lock().unwrap();
                                                            tr.insert(s.to_string(), liq_info);
                                                        }
                                                        
                                                        // Redraw UI
                                                        update_ui();
                                                    }
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
                Ok(b"LIQ_STARTED".to_vec())
            }) as EndpointHandler,
        );

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::Stop,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                *ctx_clone.is_running.write().unwrap() = false;
                Ok(b"LIQ_STOPPED".to_vec())
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

        let liq_raw = latest_liquidations.clone();
        endpoints.insert(
            StandardEndpoint::RawData,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                let tr = liq_raw.lock().unwrap();
                let json = serde_json::to_string(&*tr).unwrap_or_default();
                Ok(json.into_bytes())
            }) as EndpointHandler,
        );

        Self { ctx, endpoints }
    }
}

impl System for LiquidationSystem {
    fn id(&self) -> &str { &self.ctx.id }
    fn name(&self) -> &str { &self.ctx.name }
    fn endpoints(&self) -> &HashMap<StandardEndpoint, EndpointHandler> { &self.endpoints }
    fn context(&self) -> &SystemContext { &self.ctx }
}

#[no_mangle]
pub extern "C" fn create_plugin() -> *mut Box<dyn System> {
    let sys: Box<dyn System> = Box::new(LiquidationSystem::new());
    Box::into_raw(Box::new(sys))
}
