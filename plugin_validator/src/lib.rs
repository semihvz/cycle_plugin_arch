use orchestrator::endpoint::StandardEndpoint;
use orchestrator::system::{EndpointHandler, System, SystemContext};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use serde_json::Value;

pub struct ValidatorSystem {
    ctx: SystemContext,
    endpoints: HashMap<StandardEndpoint, EndpointHandler>,
}

#[derive(Default)]
struct ValidationStats {
    total_trades_checked: u64,
    anomalies_detected: u64,
    total_liquidations: u64,
}

impl ValidatorSystem {
    pub fn new() -> Self {
        let ctx = SystemContext::new("validator_01", "Data Cross-Validator");
        let mut endpoints = HashMap::new();
        
        let stats = Arc::new(Mutex::new(ValidationStats::default()));
        let last_log = Arc::new(Mutex::new(String::from("Bekleniyor...")));

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::Start,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                *ctx_clone.is_running.write().unwrap() = true;
                Ok(b"VALIDATOR_STARTED".to_vec())
            }) as EndpointHandler,
        );

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::Stop,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                *ctx_clone.is_running.write().unwrap() = false;
                Ok(b"VALIDATOR_STOPPED".to_vec())
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

        // We use DataValid as the payload receiver
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
                        let depth = &json["depth"];
                        
                        // "BTCUSDT": "p=123 q=1 m=false"
                        if let Some(agg_obj) = agg.as_object() {
                            for (sym, val) in agg_obj {
                                if let Some(trade_str) = val.as_str() {
                                    if let Some(p_start) = trade_str.find("p=") {
                                        let p_end = trade_str[p_start..].find(' ').unwrap_or(trade_str.len() - p_start);
                                        if let Ok(price) = trade_str[p_start+2..p_start+p_end].parse::<f64>() {
                                            
                                            if let Some(depth_str) = depth[sym].as_str() {
                                                if let (Some(b_s), Some(a_s)) = (depth_str.find("b="), depth_str.find(" a=")) {
                                                    let best_bid = depth_str[b_s+2..a_s].parse::<f64>().unwrap_or(0.0);
                                                    let best_ask = depth_str[a_s+3..].parse::<f64>().unwrap_or(0.0);
                                                    
                                                    st.total_trades_checked += 1;
                                                    
                                                    if price > 0.0 && best_bid > 0.0 && best_ask > 0.0 {
                                                        // Bazen borsa küçük gecikmelerden dolayı bid < price < ask ihlali yapabilir
                                                        if price < best_bid || price > best_ask {
                                                            st.anomalies_detected += 1;
                                                            *l = format!("ANOMALY DETECTED: {} Trade: {}, Book: b:{} a:{}", sym, price, best_bid, best_ask);
                                                        } else {
                                                            *l = format!("VERIFIED: {} Trade: {}, Book: b:{} a:{} (IN SPREAD)", sym, price, best_bid, best_ask);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(vec![1u8])
            }) as EndpointHandler,
        );

        let stats_clone = stats.clone();
        let log_clone = last_log.clone();
        endpoints.insert(
            StandardEndpoint::DataMonitor,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                let st = stats_clone.lock().unwrap();
                let l = log_clone.lock().unwrap();
                let out = format!("CROSS-VALIDATOR STATUS\n\nTotal Trades Checked: {}\nAnomalies Detected: {}\n\nLatest Check:\n{}", 
                    st.total_trades_checked, st.anomalies_detected, l);
                Ok(out.into_bytes())
            }) as EndpointHandler,
        );

        Self { ctx, endpoints }
    }
}

impl System for ValidatorSystem {
    fn id(&self) -> &str { &self.ctx.id }
    fn name(&self) -> &str { &self.ctx.name }
    fn endpoints(&self) -> &HashMap<StandardEndpoint, EndpointHandler> { &self.endpoints }
    fn context(&self) -> &SystemContext { &self.ctx }
}

#[no_mangle]
pub extern "C" fn create_plugin() -> *mut Box<dyn System> {
    let sys: Box<dyn System> = Box::new(ValidatorSystem::new());
    Box::into_raw(Box::new(sys))
}
