use orchestrator::endpoint::StandardEndpoint;
use orchestrator::system::{EndpointHandler, System, SystemContext};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use futures_util::stream::StreamExt;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;
use ordered_float::OrderedFloat;
use serde_json::Value;

#[derive(Clone, Debug)]
struct DepthUpdate {
    u: u64,
    pu: u64,
    u_first: u64,
    bids: Vec<(f64, f64)>,
    asks: Vec<(f64, f64)>,
}

struct LocalOrderBook {
    last_update_id: u64,
    is_synced: bool,
    buffer: VecDeque<DepthUpdate>,
    bids: BTreeMap<OrderedFloat<f64>, f64>,
    asks: BTreeMap<OrderedFloat<f64>, f64>,
}

impl LocalOrderBook {
    fn new() -> Self {
        Self {
            last_update_id: 0,
            is_synced: false,
            buffer: VecDeque::new(),
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    fn apply_event(&mut self, ev: &DepthUpdate) {
        for &(p, q) in &ev.bids {
            if q == 0.0 {
                self.bids.remove(&OrderedFloat(p));
            } else {
                self.bids.insert(OrderedFloat(p), q);
            }
        }
        for &(p, q) in &ev.asks {
            if q == 0.0 {
                self.asks.remove(&OrderedFloat(p));
            } else {
                self.asks.insert(OrderedFloat(p), q);
            }
        }
    }

    fn process_buffer(&mut self) {
        let mut to_process = Vec::new();
        while let Some(ev) = self.buffer.pop_front() {
            if ev.u <= self.last_update_id {
                continue; // drop
            }
            to_process.push(ev);
        }
        for ev in to_process {
            self.apply_event(&ev);
            self.last_update_id = ev.u;
        }
    }
}

struct DepthBuffer {
    queue: VecDeque<String>,
    current_bytes: usize,
    max_bytes: usize,
}

pub struct DepthSystem {
    ctx: SystemContext,
    endpoints: HashMap<StandardEndpoint, EndpointHandler>,
}

async fn fetch_snapshot(symbol: String, books: Arc<Mutex<HashMap<String, LocalOrderBook>>>) {
    let url = format!("https://fapi.binance.com/fapi/v1/depth?symbol={}&limit=1000", symbol);
    if let Ok(resp) = reqwest::get(&url).await {
        if let Ok(json) = resp.json::<Value>().await {
            let last_update_id = json["lastUpdateId"].as_u64().unwrap_or(0);
            let mut new_bids = BTreeMap::new();
            let mut new_asks = BTreeMap::new();
            
            if let Some(bids) = json["bids"].as_array() {
                for b in bids {
                    let price: f64 = b[0].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                    let qty: f64 = b[1].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                    new_bids.insert(OrderedFloat(price), qty);
                }
            }
            if let Some(asks) = json["asks"].as_array() {
                for a in asks {
                    let price: f64 = a[0].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                    let qty: f64 = a[1].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                    new_asks.insert(OrderedFloat(price), qty);
                }
            }
            
            let mut b_guard = books.lock().unwrap();
            let book = b_guard.entry(symbol).or_insert(LocalOrderBook::new());
            book.bids = new_bids;
            book.asks = new_asks;
            book.last_update_id = last_update_id;
            book.is_synced = true;
            book.process_buffer();
        }
    }
}

impl DepthSystem {
    pub fn new() -> Self {
        let ctx = SystemContext::new("depth_01", "Binance LOB 1000 + 10MB Buffer");
        let mut endpoints = HashMap::new();
        
        let order_books: Arc<Mutex<HashMap<String, LocalOrderBook>>> = Arc::new(Mutex::new(HashMap::new()));
        let outbox_queue: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        
        // 10 MB limit (10 * 1024 * 1024 = 10_485_760 bytes)
        let ring_buffer = Arc::new(Mutex::new(DepthBuffer {
            queue: VecDeque::new(),
            current_bytes: 0,
            max_bytes: 10 * 1024 * 1024, 
        }));
        
        for s in &["BTCUSDT", "ETHUSDT", "ACEUSDT"] {
            order_books.lock().unwrap().insert(s.to_string(), LocalOrderBook::new());
        }

        let ctx_clone = ctx.clone();
        let books_start = order_books.clone();
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
                    let books = books_start.clone();
                    let ring = buffer_start.clone();
                    let outbox = outbox_queue_clone.clone();
                    
                    std::thread::spawn(move || {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        rt.block_on(async move {
                            // Canlı akışı başlat (Diff stream)
                            let connect_url = Url::parse("wss://fstream.binance.com/stream?streams=btcusdt@depth/ethusdt@depth/aceusdt@depth").unwrap();
                            
                            // Snapshot çekme görevlerini başlat
                            for s in &["BTCUSDT", "ETHUSDT", "ACEUSDT"] {
                                tokio::spawn(fetch_snapshot(s.to_string(), books.clone()));
                            }
                            
                            match connect_async(connect_url).await {
                                Ok((mut ws_stream, _)) => {
                                    *is_data_valid.write().unwrap() = true;
                                    
                                    while let Some(msg) = ws_stream.next().await {
                                        if !*is_running.read().unwrap() {
                                            break;
                                        }
                                        
                                        if let Ok(Message::Text(text)) = msg {
                                            if let Ok(json) = serde_json::from_str::<Value>(&text) {
                                                let data = &json["data"];
                                                if let (Some(s), Some(u), Some(pu), Some(big_u)) = (
                                                    data["s"].as_str(),
                                                    data["u"].as_u64(),
                                                    data["pu"].as_u64(),
                                                    data["U"].as_u64(),
                                                ) {
                                                    let mut ev = DepthUpdate {
                                                        u, pu, u_first: big_u,
                                                        bids: Vec::new(),
                                                        asks: Vec::new(),
                                                    };
                                                    
                                                    if let Some(bids) = data["b"].as_array() {
                                                        for b in bids {
                                                            let p: f64 = b[0].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                                                            let q: f64 = b[1].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                                                            ev.bids.push((p, q));
                                                        }
                                                    }
                                                    if let Some(asks) = data["a"].as_array() {
                                                        for a in asks {
                                                            let p: f64 = a[0].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                                                            let q: f64 = a[1].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                                                            ev.asks.push((p, q));
                                                        }
                                                    }
                                                    
                                                    // 10MB Buffer Logic
                                                    let log_line = format!("{} | u:{} pu:{} bids:{} asks:{}\n", s, ev.u, ev.pu, ev.bids.len(), ev.asks.len());
                                                    let buffer_status = {
                                                        let mut buf = ring.lock().unwrap();
                                                        buf.current_bytes += log_line.len();
                                                        buf.queue.push_back(log_line);
                                                        
                                                        while buf.current_bytes > buf.max_bytes {
                                                            if let Some(removed) = buf.queue.pop_front() {
                                                                buf.current_bytes -= removed.len();
                                                                let msg = serde_json::json!({
                                                                    "to": "timescaledb_01",
                                                                    "type": "depth",
                                                                    "data": removed
                                                                });
                                                                outbox.lock().unwrap().push(serde_json::to_string(&msg).unwrap());
                                                            } else {
                                                                break;
                                                            }
                                                        }
                                                        format!("RAM BUFFER: {:.2} MB / 10.00 MB ({} diff updates)", 
                                                            buf.current_bytes as f64 / 1_048_576.0, buf.queue.len())
                                                    };
                                                    
                                                    // LOB Logic
                                                    let mut b_guard = books.lock().unwrap();
                                                    let book = b_guard.entry(s.to_string()).or_insert(LocalOrderBook::new());
                                                    
                                                    if book.is_synced {
                                                        book.apply_event(&ev);
                                                        book.last_update_id = ev.u;
                                                    } else {
                                                        book.buffer.push_back(ev);
                                                    }
                                                    
                                                    // UI Güncelleme Metni
                                                    let mut summary = format!("{}\n\n1000-Level LOB Status:\n", buffer_status);
                                                    for (sym, b) in b_guard.iter() {
                                                        let best_bid = b.bids.iter().next_back().map(|(p, _)| p.into_inner()).unwrap_or(0.0);
                                                        let best_ask = b.asks.iter().next().map(|(p, _)| p.into_inner()).unwrap_or(0.0);
                                                        let synced_str = if b.is_synced { "SYNCED" } else { "BUFFERING" };
                                                        
                                                        summary.push_str(&format!(
                                                            "{}: [{}] Bids: {} levels, Asks: {} levels | b={} a={}\n",
                                                            sym, synced_str, b.bids.len(), b.asks.len(), best_bid, best_ask
                                                        ));
                                                    }
                                                    
                                                    memory.write(summary.into_bytes());
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
                Ok(b"DEPTH_STARTED".to_vec())
            }) as EndpointHandler,
        );

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::Stop,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                *ctx_clone.is_running.write().unwrap() = false;
                Ok(b"DEPTH_STOPPED".to_vec())
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

        let books_raw = order_books.clone();
        endpoints.insert(
            StandardEndpoint::RawData,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                let guard = books_raw.lock().unwrap();
                let mut summary = HashMap::new();
                for (sym, b) in guard.iter() {
                    let best_bid = b.bids.iter().next_back().map(|(p, _)| p.into_inner()).unwrap_or(0.0);
                    let best_ask = b.asks.iter().next().map(|(p, _)| p.into_inner()).unwrap_or(0.0);
                    summary.insert(sym.clone(), format!("b={} a={}", best_bid, best_ask));
                }
                let json = serde_json::to_string(&summary).unwrap_or_default();
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

impl System for DepthSystem {
    fn id(&self) -> &str { &self.ctx.id }
    fn name(&self) -> &str { &self.ctx.name }
    fn endpoints(&self) -> &HashMap<StandardEndpoint, EndpointHandler> { &self.endpoints }
    fn context(&self) -> &SystemContext { &self.ctx }
}

#[no_mangle]
pub extern "C" fn create_plugin() -> *mut Box<dyn System> {
    let sys: Box<dyn System> = Box::new(DepthSystem::new());
    Box::into_raw(Box::new(sys))
}
