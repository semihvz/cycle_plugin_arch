use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::str::FromStr;

mod session;
mod pivot;
mod trend;
mod levels;
mod liquidity;
mod imbalance;
mod narrative;

use ohlcv_engine::Kline;

#[repr(C)]
pub struct PluginOps {
    pub name: *const std::ffi::c_char,
    pub start: unsafe extern "C" fn(*mut c_void),
    pub stop: unsafe extern "C" fn(*mut c_void),
    pub update: unsafe extern "C" fn(*mut c_void, *mut u8, usize) -> usize,
    pub call_endpoint: unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize,
}

use std::collections::HashMap;

fn parse_decimal(val: &serde_json::Value) -> rust_decimal::Decimal {
    if let Some(s) = val.as_str() {
        rust_decimal::Decimal::from_str(s).unwrap_or_default()
    } else if let Some(f) = val.as_f64() {
        rust_decimal::Decimal::from_f64_retain(f).unwrap_or_default()
    } else if let Some(i) = val.as_i64() {
        rust_decimal::Decimal::from(i)
    } else if let Some(u) = val.as_u64() {
        rust_decimal::Decimal::from(u)
    } else {
        rust_decimal::Decimal::ZERO
    }
}

struct PluginState {
    is_running: Arc<AtomicBool>,
    data: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    outbox: Arc<Mutex<Vec<serde_json::Value>>>,
    stream_configs: Arc<Mutex<HashMap<String, (String, String)>>>,
    output_streams: Arc<Mutex<Vec<String>>>,
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(state_out: *mut *mut c_void) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let state = Box::new(PluginState {
        is_running: Arc::new(AtomicBool::new(false)),
        data: Arc::new(Mutex::new(HashMap::new())),
        outbox: Arc::new(Mutex::new(Vec::new())),
        stream_configs: Arc::new(Mutex::new(HashMap::new())),
        output_streams: Arc::new(Mutex::new(Vec::new())),
    });

    unsafe {
        *state_out = Box::into_raw(state) as *mut c_void;
    }

    handle_endpoint
}

unsafe extern "C" fn handle_endpoint(
    plugin_state: *mut c_void,
    endpoint_id: u32,
    payload: *const u8,
    payload_len: usize,
    out_buf: *mut u8,
    out_max_len: usize,
) -> usize {
    let state = &*(plugin_state as *const PluginState);

    match endpoint_id {
        0 => { // Start
            if state.is_running.load(Ordering::Relaxed) {
                return 0;
            }
            state.is_running.store(true, Ordering::Relaxed);
            
            if payload_len > 0 {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                if let Ok(config) = serde_json::from_slice::<serde_json::Value>(slice) {
                    if let Some(outputs) = config.get("plugin_outputs").and_then(|o| o.as_array()) {
                        let mut outs = state.output_streams.lock().unwrap();
                        outs.clear();
                        for out in outputs {
                            if let Some(s) = out.as_str() {
                                outs.push(s.to_string());
                            }
                        }
                    }
                    if let Some(inputs) = config.get("plugin_inputs").and_then(|i| i.as_array()) {
                        let mut q = state.outbox.lock().unwrap();
                        for input in inputs {
                            if let (Some(source), Some(params), Some(stream_id)) = (
                                input.get("source").and_then(|s| s.as_str()),
                                input.get("params").and_then(|p| p.as_object()),
                                input.get("stream_id").and_then(|s| s.as_str())
                            ) {
                                let mut req = serde_json::Map::new();
                                req.insert("to".to_string(), serde_json::json!(source));
                                req.insert("stream_id".to_string(), serde_json::json!(stream_id));
                                for (k, v) in params {
                                    req.insert(k.clone(), v.clone());
                                }
                                
                                if let (Some(sym), Some(inv)) = (
                                    req.get("symbol").and_then(|v| v.as_str()),
                                    req.get("interval").and_then(|v| v.as_str())
                                ) {
                                    let mut configs = state.stream_configs.lock().unwrap();
                                    configs.insert(stream_id.to_string(), (sym.to_string(), inv.to_string()));
                                }
                                
                                q.push(serde_json::Value::Object(req));
                            }
                        }
                    }
                }
            }
            
            0
        }
        1 => { // Stop
            state.is_running.store(false, Ordering::Relaxed);
            0
        }
        2 => { // IsWorking
            if state.is_running.load(Ordering::Relaxed) { 1 } else { 0 }
        }
        3 => { // DataValid
            1
        }
        4 | 5 => { // DataMonitor & RawData
            let guard = state.data.lock().unwrap();
            if let Ok(bytes) = serde_json::to_vec(&*guard) {
                let len = bytes.len().min(out_max_len);
                if len > 0 {
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, len);
                }
                len
            } else {
                0
            }
        }
        6 => { // Inbox
            if payload_len > 32 {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                // FlowEngine prepends a 32-byte header with the local input name (e.g., "ohlcv")
                let header = &slice[0..32];
                let data_slice = &slice[32..];
                
                let stream_id = std::str::from_utf8(header)
                    .unwrap_or("")
                    .trim_matches(char::from(0))
                    .to_string();
                    
                let (symbol, interval) = {
                    let configs = state.stream_configs.lock().unwrap();
                    configs.get(&stream_id)
                        .cloned()
                        .unwrap_or_else(|| ("Bilinmiyor".to_string(), "Bilinmiyor".to_string()))
                };
                
                // Read the JSON array of klines
                if let Ok(data_array) = serde_json::from_slice::<serde_json::Value>(data_slice) {
                    if let Some(arr) = data_array.as_array() {
                        let mut klines = Vec::new();
                        for row in arr {
                            if let Some(row_arr) = row.as_array() {
                                if row_arr.len() >= 11 {
                                    let open_time = row_arr[0].as_u64().unwrap_or(0);
                                    let open = parse_decimal(&row_arr[1]);
                                    let high = parse_decimal(&row_arr[2]);
                                    let low = parse_decimal(&row_arr[3]);
                                    let close = parse_decimal(&row_arr[4]);
                                    let volume = parse_decimal(&row_arr[5]);
                                    let close_time = row_arr[6].as_u64().unwrap_or(0);
                                    let taker_buy_base = parse_decimal(&row_arr[9]);
                                    
                                    klines.push(Kline {
                                        open_time, open, high, low, close, volume, close_time,
                                        taker_buy_base_asset_volume: taker_buy_base,
                                    });
                                }
                            }
                        }
                        
                        if !klines.is_empty() {
                            let len = klines.len();
                            let core_limit = 100.min(len);
                            let amp_limit = 400.min(len);
                            let acute_limit = 96.min(len);
                            
                            let core_klines = &klines[len.saturating_sub(core_limit)..];
                            let amp_klines = &klines[len.saturating_sub(amp_limit)..];
                            let acute_klines = &klines[len.saturating_sub(acute_limit)..];
                            
                            let report = narrative::generate_report(core_klines, amp_klines, acute_klines);
                            let formatted_table = report.format_table(&symbol, &interval, &stream_id, len);
                            
                            let mut report_json = serde_json::to_value(&report).unwrap_or_default();
                            if let Some(obj) = report_json.as_object_mut() {
                                obj.insert("symbol".to_string(), serde_json::json!(symbol));
                                obj.insert("interval".to_string(), serde_json::json!(interval));
                                obj.insert("analyzed_bars".to_string(), serde_json::json!(len));
                                obj.insert("stream_id".to_string(), serde_json::json!(stream_id));
                                obj.insert("formatted_table".to_string(), serde_json::json!(formatted_table));
                            }
                            
                            // Write to RAM/screen buffer map
                            let mut guard = state.data.lock().unwrap();
                            guard.insert(stream_id.clone(), report_json.clone());
                            
                            let outs = state.output_streams.lock().unwrap();
                            for out_stream in outs.iter() {
                                if (stream_id.contains("btc") && out_stream.contains("btc"))
                                    || (stream_id.contains("eth") && out_stream.contains("eth"))
                                    || (stream_id.contains("tac") && out_stream.contains("tac"))
                                    || (symbol.to_lowercase().contains("btc") && out_stream.contains("btc"))
                                    || (symbol.to_lowercase().contains("eth") && out_stream.contains("eth"))
                                    || (symbol.to_lowercase().contains("tac") && out_stream.contains("tac")) {
                                    guard.insert(out_stream.clone(), report_json.clone());
                                }
                            }
                            if outs.len() == 1 {
                                guard.insert(outs[0].clone(), report_json.clone());
                            }
                        }
                    }
                }
            }
            0
        }
        7 => { // Outbox
            let mut q = state.outbox.lock().unwrap();
            if !q.is_empty() {
                let json_array = serde_json::Value::Array(q.clone());
                q.clear();
                let bytes = serde_json::to_vec(&json_array).unwrap_or_default();
                let len = bytes.len().min(out_max_len);
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, len);
                len
            } else {
                0
            }
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ms_analyzer_lifecycle_and_endpoints() {
        let mut state_ptr: *mut c_void = std::ptr::null_mut();
        unsafe {
            let endpoint_fn = init_plugin(&mut state_ptr);

            // 1. Start with plugin config
            let start_config = serde_json::json!({
                "plugin_name": "plugin_ms_analyzer",
                "plugin_outputs": ["ms_signals_btc"],
                "plugin_inputs": [
                    {
                        "source": "plugin_ohlcv_fetcher",
                        "stream_id": "btc_15m",
                        "params": {
                            "symbol": "BTCUSDT",
                            "interval": "15m"
                        }
                    }
                ]
            });
            let config_bytes = serde_json::to_vec(&start_config).unwrap();
            let mut dummy_buf = [0u8; 16];
            endpoint_fn(state_ptr, 0, config_bytes.as_ptr(), config_bytes.len(), dummy_buf.as_mut_ptr(), dummy_buf.len());

            // 2. Check working status
            let is_working = endpoint_fn(state_ptr, 2, std::ptr::null(), 0, dummy_buf.as_mut_ptr(), dummy_buf.len());
            assert_eq!(is_working, 1);

            // 3. Send mock klines via Inbox (Endpoint 6)
            let mut klines_json = Vec::new();
            for i in 0..15 {
                let open_time = 1600000000000u64 + (i as u64) * 900000u64;
                let close_time = open_time + 899999;
                let open_val = 60000.0 + (i as f64) * 10.0;
                let high_val = open_val + 50.0;
                let low_val = open_val - 20.0;
                let close_val = open_val + 30.0;
                let vol_val = 100.5;
                let taker_vol = 50.25;

                // Test mixing numbers and string representations
                let row = if i % 2 == 0 {
                    serde_json::json!([open_time, open_val, high_val, low_val, close_val, vol_val, close_time, "0", 0, taker_vol, "0", "0"])
                } else {
                    serde_json::json!([open_time, open_val.to_string(), high_val.to_string(), low_val.to_string(), close_val.to_string(), vol_val.to_string(), close_time, "0", 0, taker_vol.to_string(), "0", "0"])
                };
                klines_json.push(row);
            }
            let data_bytes = serde_json::to_vec(&serde_json::Value::Array(klines_json)).unwrap();

            // Construct 32-byte header + data_bytes
            let mut combined = Vec::with_capacity(32 + data_bytes.len());
            let mut header = [0u8; 32];
            let stream_id_bytes = b"btc_15m";
            header[..stream_id_bytes.len()].copy_from_slice(stream_id_bytes);
            combined.extend_from_slice(&header);
            combined.extend_from_slice(&data_bytes);

            endpoint_fn(state_ptr, 6, combined.as_ptr(), combined.len(), dummy_buf.as_mut_ptr(), dummy_buf.len());

            // 4. Test RawData / DataMonitor (Endpoint 5 and 4)
            let mut raw_buf = vec![0u8; 65536];
            let written_5 = endpoint_fn(state_ptr, 5, std::ptr::null(), 0, raw_buf.as_mut_ptr(), raw_buf.len());
            assert!(written_5 > 0, "Endpoint 5 (RawData) should return > 0 bytes");

            let written_4 = endpoint_fn(state_ptr, 4, std::ptr::null(), 0, raw_buf.as_mut_ptr(), raw_buf.len());
            assert!(written_4 > 0, "Endpoint 4 (DataMonitor) should return > 0 bytes");

            let parsed: serde_json::Value = serde_json::from_slice(&raw_buf[..written_5]).unwrap();
            assert!(parsed.get("btc_15m").is_some() || parsed.get("ms_signals_btc").is_some());
        }
    }
}


