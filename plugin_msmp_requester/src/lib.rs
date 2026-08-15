use orchestrator::endpoint::StandardEndpoint;
use orchestrator::system::{EndpointHandler, System, SystemContext};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::fs;
use uuid::Uuid;

pub struct MsmpRequesterSystem {
    ctx: SystemContext,
    endpoints: HashMap<StandardEndpoint, EndpointHandler>,
}

impl MsmpRequesterSystem {
    pub fn new() -> Self {
        let ctx = SystemContext::new("plugin_msmp_requester", "MSMP Requester");
        let mut endpoints = HashMap::new();

        let outbox = Arc::new(Mutex::new(Vec::<Value>::new()));
        let last_log = Arc::new(RwLock::new(String::from("Bekleniyor... Servis baslatilmadi.")));
        let received_data = Arc::new(RwLock::new(Option::<Value>::None));

        let ctx_clone = ctx.clone();
        let outbox_clone = outbox.clone();
        let log_clone_start = last_log.clone();
        let data_clone_start = received_data.clone();
        endpoints.insert(
            StandardEndpoint::Start,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                *ctx_clone.is_running.write().unwrap() = true;
                
                // Ekrandaki eski sonuçları (RAM) temizle
                *data_clone_start.write().unwrap() = None;
                
                // Konfigürasyon dosyasını oku veya oluştur
                let config_path = "msmp_req_config.json";
                let default_config = json!({
                    "symbol": "BTCUSDT",
                    "interval": "15m",
                    "limit": 500
                });
                
                let config = if let Ok(content) = fs::read_to_string(config_path) {
                    serde_json::from_str(&content).unwrap_or(default_config.clone())
                } else {
                    let _ = fs::write(config_path, serde_json::to_string_pretty(&default_config).unwrap_or_default());
                    default_config
                };

                let symbol = config["symbol"].as_str().unwrap_or("BTCUSDT");
                let interval = config["interval"].as_str().unwrap_or("15m");
                let limit = config["limit"].as_u64().unwrap_or(500);

                let msg_id = Uuid::new_v4().to_string();
                let request_msg = json!({
                    "msg_id": msg_id,
                    "from": "plugin_msmp_requester",
                    "to": "plugin_msmp",
                    "type": "REQUEST",
                    "payload": {
                        "symbol": symbol,
                        "interval": interval,
                        "limit": limit
                    }
                });
                outbox_clone.lock().unwrap().push(request_msg);
                
                *log_clone_start.write().unwrap() = format!("İstek gönderildi: {} {} {} bar (ID: {})", symbol, interval, limit, msg_id);

                Ok(b"STARTED".to_vec())
            }) as EndpointHandler,
        );

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::Stop,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                *ctx_clone.is_running.write().unwrap() = false;
                Ok(b"STOPPED".to_vec())
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
                        if msg["type"].as_str() == Some("RESPONSE") && msg["from"].as_str() == Some("plugin_msmp") {
                            if let Some(payload_obj) = msg.get("payload") {
                                if payload_obj["status"].as_str() == Some("success") {
                                    *data_clone.write().unwrap() = Some(payload_obj["data"].clone());
                                    *log_clone.write().unwrap() = String::from("MSMP Raporu alindi ve RAM'e kaydedildi.");
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
                let mut out = format!("MSMP REQUESTER\n==============\nDurum: {}\n\n", log_msg);
                
                if let Some(data) = &*data_clone.read().unwrap() {
                    out.push_str(&serde_json::to_string_pretty(data).unwrap_or_default());
                }
                
                Ok(out.into_bytes())
            }) as EndpointHandler,
        );

        Self { ctx, endpoints }
    }
}

impl System for MsmpRequesterSystem {
    fn id(&self) -> &str { &self.ctx.id }
    fn name(&self) -> &str { &self.ctx.name }
    fn endpoints(&self) -> &HashMap<StandardEndpoint, EndpointHandler> { &self.endpoints }
    fn context(&self) -> &SystemContext { &self.ctx }
}

#[no_mangle]
pub extern "C" fn create_plugin() -> *mut Box<dyn System> {
    let sys: Box<dyn System> = Box::new(MsmpRequesterSystem::new());
    Box::into_raw(Box::new(sys))
}
