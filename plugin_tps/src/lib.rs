use orchestrator::endpoint::StandardEndpoint;
use orchestrator::system::{EndpointHandler, System, SystemContext};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use serde_json::Value;

pub struct TpsSystem {
    ctx: SystemContext,
    endpoints: HashMap<StandardEndpoint, EndpointHandler>,
}

#[derive(Clone)]
struct TpsState {
    last_count: u64,
    last_time: Instant,
    current_tps: u64,
}

impl TpsSystem { // Using ValidatorSystem struct name is fine internally but let's rename it to TpsSystem
    pub fn new() -> Self {
        let ctx = SystemContext::new("tps_01", "Trade Per Second (TPS) Monitor");
        let mut endpoints = HashMap::new();
        
        let stats = Arc::new(Mutex::new(HashMap::<String, TpsState>::new()));
        let last_log = Arc::new(Mutex::new(String::from("Bekleniyor...")));

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::Start,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                *ctx_clone.is_running.write().unwrap() = true;
                Ok(b"TPS_STARTED".to_vec())
            }) as EndpointHandler,
        );

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::Stop,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                *ctx_clone.is_running.write().unwrap() = false;
                Ok(b"TPS_STOPPED".to_vec())
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

        let stats_clone = stats.clone();
        let log_clone = last_log.clone();
        endpoints.insert(
            StandardEndpoint::DataValid,
            Arc::new(move |_ctx: &SystemContext, payload: Option<Vec<u8>>| {
                if let Some(data) = payload {
                    if let Ok(json) = serde_json::from_slice::<Value>(&data) {
                        let mut st = stats_clone.lock().unwrap();
                        let mut l = log_clone.lock().unwrap();
                        
                        let agg = &json["agg"];
                        
                        if let Some(agg_obj) = agg.as_object() {
                            let mut tps_report = String::new();
                            for (sym, val) in agg_obj {
                                if let Some(trade_str) = val.as_str() {
                                    if let Some(c_start) = trade_str.find("c=") {
                                        if let Ok(count) = trade_str[c_start+2..].parse::<u64>() {
                                            let now = Instant::now();
                                            let state = st.entry(sym.clone()).or_insert(TpsState {
                                                last_count: count,
                                                last_time: now,
                                                current_tps: 0,
                                            });
                                            
                                            let elapsed = now.duration_since(state.last_time).as_secs_f64();
                                            if elapsed >= 1.0 {
                                                let diff = count.saturating_sub(state.last_count);
                                                state.current_tps = (diff as f64 / elapsed).round() as u64;
                                                state.last_count = count;
                                                state.last_time = now;
                                            }
                                            
                                            tps_report.push_str(&format!("{}: {} TPS (Total: {})\n", sym, state.current_tps, count));
                                        }
                                    }
                                }
                            }
                            if !tps_report.is_empty() {
                                *l = tps_report;
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
                let out = format!("LIVE TPS MONITOR\n================\n\n{}", l);
                Ok(out.into_bytes())
            }) as EndpointHandler,
        );

        Self { ctx, endpoints }
    }
}

impl System for TpsSystem {
    fn id(&self) -> &str { &self.ctx.id }
    fn name(&self) -> &str { &self.ctx.name }
    fn endpoints(&self) -> &HashMap<StandardEndpoint, EndpointHandler> { &self.endpoints }
    fn context(&self) -> &SystemContext { &self.ctx }
}

#[no_mangle]
pub extern "C" fn create_plugin() -> *mut Box<dyn System> {
    let sys: Box<dyn System> = Box::new(TpsSystem::new());
    Box::into_raw(Box::new(sys))
}
