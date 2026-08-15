use orchestrator::endpoint::StandardEndpoint;
use orchestrator::system::{EndpointHandler, System, SystemContext};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Deserialize, Clone)]
struct AlarmConfig {
    symbol: String,
    #[serde(rename = "type")]
    alarm_type: String, // "up" or "down"
    threshold: f64,
}

#[derive(Debug, Deserialize, Clone)]
struct Config {
    alarms: Vec<AlarmConfig>,
}

fn play_beep(freq: f32, duration_ms: u32) {
    let sample_rate = 8000;
    let num_samples = (sample_rate * duration_ms) / 1000;
    let mut data = Vec::with_capacity(num_samples as usize);
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let sample = (t * freq * 2.0 * std::f32::consts::PI).sin() * 127.0 + 128.0;
        data.push(sample as u8);
    }
    
    if let Ok(mut child) = Command::new("aplay")
        .args(&["-c", "1", "-f", "U8", "-r", "8000"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn() {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(&data);
        }
        let _ = child.wait();
    }
}

pub struct AlarmSystem {
    ctx: SystemContext,
    endpoints: HashMap<StandardEndpoint, EndpointHandler>,
}

impl AlarmSystem {
    pub fn new() -> Self {
        let ctx = SystemContext::new("alarm_01", "Alarm Eklentisi");
        let mut endpoints = HashMap::new();

        // Load config
        let config_path = "../plugin_alarm/alarm_config.json"; // Relative to main executable
        let config = if let Ok(file) = File::open(config_path) {
            let reader = BufReader::new(file);
            serde_json::from_reader::<_, Config>(reader).unwrap_or(Config { alarms: vec![] })
        } else {
            Config { alarms: vec![] }
        };

        let alarms = Arc::new(config.alarms);
        let triggered_alarms = Arc::new(Mutex::new(HashSet::new()));

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::Start,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                *ctx_clone.is_running.write().unwrap() = true;
                Ok(b"ALARM_STARTED".to_vec())
            }) as EndpointHandler,
        );

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::Stop,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                *ctx_clone.is_running.write().unwrap() = false;
                Ok(b"ALARM_STOPPED".to_vec())
            }) as EndpointHandler,
        );

        let alarms_clone = alarms.clone();
        let triggered_clone = triggered_alarms.clone();
        let ctx_clone = ctx.clone();
        
        endpoints.insert(
            StandardEndpoint::Inbox,
            Arc::new(move |_ctx: &SystemContext, payload: Option<Vec<u8>>| {
                if !*ctx_clone.is_running.read().unwrap() {
                    return Ok(vec![]);
                }
                if let Some(data) = payload {
                    if let Ok(prices) = serde_json::from_slice::<HashMap<String, String>>(&data) {
                        let mut triggered = triggered_clone.lock().unwrap();
                        
                        for alarm in alarms_clone.iter() {
                            if let Some(price_str) = prices.get(&alarm.symbol) {
                                if let Ok(price) = price_str.parse::<f64>() {
                                    let key = format!("{}_{}", alarm.symbol, alarm.alarm_type);
                                    let mut should_alarm = false;
                                    
                                    if alarm.alarm_type == "up" && price >= alarm.threshold {
                                        should_alarm = true;
                                    } else if alarm.alarm_type == "down" && price <= alarm.threshold {
                                        should_alarm = true;
                                    }
                                    
                                    if should_alarm && !triggered.contains(&key) {
                                        // Play sound
                                        triggered.insert(key.clone());
                                        let freq = if alarm.alarm_type == "up" { 1000.0 } else { 300.0 };
                                        
                                        std::thread::spawn(move || {
                                            play_beep(freq, 500);
                                        });
                                        
                                        // Update memory for UI display
                                        let msg = format!("ALARM: {} {} at {}", alarm.symbol, alarm.alarm_type, price);
                                        ctx_clone.memory.write(msg.into_bytes());
                                    } else if !should_alarm {
                                        // Reset alarm trigger if price recovers
                                        triggered.remove(&key);
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(vec![])
            }) as EndpointHandler,
        );

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::DataMonitor,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                let mut data = ctx_clone.memory.read();
                if data.is_empty() {
                    data = b"BEKLEMEDE".to_vec();
                }
                Ok(data)
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

        Self { ctx, endpoints }
    }
}

impl System for AlarmSystem {
    fn id(&self) -> &str { &self.ctx.id }
    fn name(&self) -> &str { &self.ctx.name }
    fn endpoints(&self) -> &HashMap<StandardEndpoint, EndpointHandler> { &self.endpoints }
    fn context(&self) -> &SystemContext { &self.ctx }
}

#[no_mangle]
pub extern "C" fn create_plugin() -> *mut Box<dyn System> {
    let sys: Box<dyn System> = Box::new(AlarmSystem::new());
    Box::into_raw(Box::new(sys))
}
