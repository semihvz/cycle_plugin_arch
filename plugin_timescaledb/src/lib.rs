use orchestrator::endpoint::StandardEndpoint;
use orchestrator::system::{EndpointHandler, System, SystemContext};
use std::collections::HashMap;
use std::sync::Arc;
use tokio_postgres::NoTls;
use serde_json::Value;
use tokio::sync::mpsc;

pub struct TimescaleDBSystem {
    ctx: SystemContext,
    endpoints: HashMap<StandardEndpoint, EndpointHandler>,
}

impl TimescaleDBSystem {
    pub fn new() -> Self {
        let ctx = SystemContext::new("timescaledb_01", "TimescaleDB Storage");
        let mut endpoints = HashMap::new();

        let ctx_clone = ctx.clone();
        
        let (tx, rx) = mpsc::unbounded_channel::<(String, String)>();
        let tx = Arc::new(tx);
        let rx_mutex = Arc::new(std::sync::Mutex::new(Some(rx)));

        endpoints.insert(
            StandardEndpoint::Start,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                let mut running = ctx_clone.is_running.write().unwrap();
                if !*running {
                    *running = true;
                    let is_running = ctx_clone.is_running.clone();
                    let memory = ctx_clone.memory.clone();
                    let rx_mutex = rx_mutex.clone();
                    
                    std::thread::spawn(move || {
                        let mut rx_opt = rx_mutex.lock().unwrap();
                        if let Some(mut rx) = rx_opt.take() {
                            let rt = tokio::runtime::Runtime::new().unwrap();
                            rt.block_on(async move {
                                memory.write(b"Veritabanina baglaniliyor...".to_vec());
                            
                            let db_url = std::env::var("DATABASE_URL")
                                .unwrap_or_else(|_| "host=localhost user=postgres password=postgres dbname=cycle_orc".to_string());
                            
                            match tokio_postgres::connect(&db_url, NoTls).await {
                                Ok((client, connection)) => {
                                    tokio::spawn(async move {
                                        if let Err(e) = connection.await {
                                            eprintln!("PostgreSQL connection error: {}", e);
                                        }
                                    });
                                    
                                    let _ = client.execute("CREATE EXTENSION IF NOT EXISTS timescaledb;", &[]).await;
                                    
                                    let _ = client.execute("CREATE TABLE IF NOT EXISTS market_data_history (
                                        time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                                        data_type TEXT NOT NULL,
                                        data JSONB NOT NULL
                                    );", &[]).await;
                                    
                                    let _ = client.execute("SELECT create_hypertable('market_data_history', 'time', if_not_exists => TRUE);", &[]).await;
                                    
                                    memory.write(b"TimescaleDB Baglantisi Basarili (Hypertable Hazir)".to_vec());
                                    
                                    while *is_running.read().unwrap() {
                                        tokio::select! {
                                            Some((data_type, payload_str)) = rx.recv() => {
                                                if let Ok(json_val) = serde_json::from_str::<Value>(&payload_str) {
                                                    let _ = client.execute(
                                                        "INSERT INTO market_data_history (data_type, data) VALUES ($1, $2)",
                                                        &[&data_type, &json_val]
                                                    ).await;
                                                } else {
                                                    // If it's not JSON, just wrap it in JSON
                                                    let json_val = serde_json::json!({"text": payload_str});
                                                    let _ = client.execute(
                                                        "INSERT INTO market_data_history (data_type, data) VALUES ($1, $2)",
                                                        &[&data_type, &json_val]
                                                    ).await;
                                                }
                                            }
                                            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {}
                                        }
                                    }
                                }
                                Err(e) => {
                                    memory.write(format!("DB Baglanti Hatasi: {}", e).into_bytes());
                                }
                            }
                        });
                        }
                    });
                }
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

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::DataMonitor,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                let mut data = ctx_clone.memory.read();
                if data.is_empty() {
                    data = b"BEKLEMEDE...".to_vec();
                }
                Ok(data)
            }) as EndpointHandler,
        );

        let tx_inbox = tx.clone();
        endpoints.insert(
            StandardEndpoint::Inbox,
            Arc::new(move |_ctx: &SystemContext, payload: Option<Vec<u8>>| {
                if let Some(data) = payload {
                    if let Ok(json) = serde_json::from_slice::<Value>(&data) {
                        if let (Some(data_type), Some(payload_str)) = (
                            json.get("type").and_then(|v| v.as_str()),
                            json.get("data").and_then(|v| v.as_str())
                        ) {
                            let _ = tx_inbox.send((data_type.to_string(), payload_str.to_string()));
                        }
                    }
                }
                Ok(vec![])
            }) as EndpointHandler,
        );

        Self { ctx, endpoints }
    }
}

impl System for TimescaleDBSystem {
    fn id(&self) -> &str { &self.ctx.id }
    fn name(&self) -> &str { &self.ctx.name }
    fn endpoints(&self) -> &HashMap<StandardEndpoint, EndpointHandler> { &self.endpoints }
    fn context(&self) -> &SystemContext { &self.ctx }
}

#[no_mangle]
pub extern "C" fn create_plugin() -> *mut Box<dyn System> {
    let sys: Box<dyn System> = Box::new(TimescaleDBSystem::new());
    Box::into_raw(Box::new(sys))
}
