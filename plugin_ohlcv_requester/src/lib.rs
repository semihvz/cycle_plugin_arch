use orchestrator::endpoint::StandardEndpoint;
use orchestrator::system::{EndpointHandler, System, SystemContext};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use uuid::Uuid;

pub struct OhlcvRequesterSystem {
    ctx: SystemContext,
    endpoints: HashMap<StandardEndpoint, EndpointHandler>,
}

impl OhlcvRequesterSystem {
    pub fn new() -> Self {
        let ctx = SystemContext::new("plugin_ohlcv_requester", "OHLCV Requester (Service)");
        let mut endpoints = HashMap::new();

        let outbox = Arc::new(Mutex::new(Vec::<Value>::new()));
        let last_log = Arc::new(RwLock::new(String::from("Bekleniyor... Servis baslatilmadi.")));
        let received_data = Arc::new(RwLock::new(Option::<Value>::None));

        // Start endpoint: Servis başladığında isteği gönderir
        let ctx_clone = ctx.clone();
        let outbox_clone = outbox.clone();
        let log_clone = last_log.clone();
        endpoints.insert(
            StandardEndpoint::Start,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                *ctx_clone.is_running.write().unwrap() = true;

                // Send request
                let msg_id = Uuid::new_v4().to_string();
                let request_msg = json!({
                    "msg_id": msg_id,
                    "from": "plugin_ohlcv_requester",
                    "to": "plugin_ohlcv_fetcher",
                    "type": "REQUEST",
                    "payload": {
                        "symbol": "ACEUSDT",
                        "interval": "1m",
                        "limit": 100
                    }
                });

                outbox_clone.lock().unwrap().push(request_msg);
                
                *log_clone.write().unwrap() = String::from("Servis calistirildi. ACEUSDT 1m 100 bar icin istek gonderildi.");

                Ok(b"REQUESTER_STARTED".to_vec())
            }) as EndpointHandler,
        );

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::Stop,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                *ctx_clone.is_running.write().unwrap() = false;
                Ok(b"REQUESTER_STOPPED".to_vec())
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

        let log_clone = last_log.clone();
        let data_clone = received_data.clone();
        endpoints.insert(
            StandardEndpoint::Inbox,
            Arc::new(move |_ctx: &SystemContext, payload: Option<Vec<u8>>| {
                if let Some(data) = payload {
                    if let Ok(msg) = serde_json::from_slice::<Value>(&data) {
                        if msg["type"].as_str() == Some("RESPONSE") && msg["from"].as_str() == Some("plugin_ohlcv_fetcher") {
                            if let Some(payload_obj) = msg.get("payload") {
                                if payload_obj["status"].as_str() == Some("success") {
                                    *data_clone.write().unwrap() = Some(payload_obj["data"].clone());
                                    
                                    if let Some(arr) = payload_obj["data"].as_array() {
                                        *log_clone.write().unwrap() = format!("Cevap alindi. {} adet OHLCV mumu RAM'e kaydedildi.", arr.len());
                                    } else {
                                        *log_clone.write().unwrap() = format!("Cevap alindi, ancak beklenen data array formatinda degil.");
                                    }
                                } else {
                                    *log_clone.write().unwrap() = format!("Cevap alindi ancak status basarisiz.");
                                }
                            }
                        }
                    }
                }
                Ok(vec![1u8])
            }) as EndpointHandler,
        );

        let log_clone = last_log.clone();
        let data_clone = received_data.clone();
        endpoints.insert(
            StandardEndpoint::DataMonitor,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                let log_msg = log_clone.read().unwrap().clone();
                let mut out = format!("OHLCV REQUESTER (SERVICE)\n=========================\n\nDurum: {}\n\n", log_msg);
                
                if let Some(data) = &*data_clone.read().unwrap() {
                    if let Some(arr) = data.as_array() {
                        if let Some(first) = arr.first() {
                            out.push_str(&format!("Ilk Mum Ornegi: {}\n", serde_json::to_string_pretty(first).unwrap_or_default()));
                        }
                    }
                }
                
                Ok(out.into_bytes())
            }) as EndpointHandler,
        );

        Self { ctx, endpoints }
    }
}

impl System for OhlcvRequesterSystem {
    fn id(&self) -> &str { &self.ctx.id }
    fn name(&self) -> &str { &self.ctx.name }
    fn endpoints(&self) -> &HashMap<StandardEndpoint, EndpointHandler> { &self.endpoints }
    fn context(&self) -> &SystemContext { &self.ctx }
}

#[no_mangle]
pub extern "C" fn create_plugin() -> *mut Box<dyn System> {
    let sys: Box<dyn System> = Box::new(OhlcvRequesterSystem::new());
    Box::into_raw(Box::new(sys))
}
