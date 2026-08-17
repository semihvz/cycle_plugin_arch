use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

pub fn now_ts() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_secs_f64()
}

pub fn parse_json(payload: &str) -> Value {
    serde_json::from_str(payload).unwrap_or(Value::Null)
}

pub fn event_ts(data: &Value) -> f64 {
    let raw = data["T"].as_u64().or_else(|| data["E"].as_u64());
    match raw {
        Some(ts) => ts as f64 / 1000.0,
        None => now_ts(),
    }
}

pub fn chunked(items: &[String], size: usize) -> Vec<Vec<String>> {
    items.chunks(size).map(|c| c.to_vec()).collect()
}
