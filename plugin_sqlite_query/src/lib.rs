pub mod storage_reader;

pub use storage_reader::*;

use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use serde_json::Value;

struct PluginState {
    is_running: Arc<AtomicBool>,
    reader: Arc<Mutex<Option<Arc<StorageReader>>>>,
    db_path: Arc<Mutex<String>>,
    outbox: Arc<Mutex<Vec<String>>>,
    last_query: Arc<Mutex<String>>,
    last_result: Arc<Mutex<String>>,
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(
    state_out: *mut *mut c_void,
) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let state = Box::new(PluginState {
        is_running: Arc::new(AtomicBool::new(false)),
        reader: Arc::new(Mutex::new(None)),
        db_path: Arc::new(Mutex::new("binance_market_data.db".to_string())),
        outbox: Arc::new(Mutex::new(Vec::new())),
        last_query: Arc::new(Mutex::new(String::new())),
        last_result: Arc::new(Mutex::new(String::new())),
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
            let mut db_path = "binance_market_data.db".to_string();
            if payload_len > 0 {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                if let Ok(config) = serde_json::from_slice::<Value>(slice) {
                    if let Some(params) = config.get("plugin_params") {
                        if let Some(path) = params.get("db_path").and_then(|p| p.as_str()) {
                            db_path = path.to_string();
                        }
                    }
                }
            }

            *state.db_path.lock().unwrap() = db_path.clone();

            match StorageReader::new(&db_path) {
                Ok(reader_inst) => {
                    *state.reader.lock().unwrap() = Some(Arc::new(reader_inst));
                    state.is_running.store(true, Ordering::Relaxed);
                    0
                }
                Err(e) => {
                    eprintln!("[plugin_sqlite_query] SQLite reader init error: {}", e);
                    0
                }
            }
        }
        1 => { // Stop
            state.is_running.store(false, Ordering::Relaxed);
            0
        }
        2 => { // IsWorking
            if state.is_running.load(Ordering::Relaxed) { 1 } else { 0 }
        }
        4 => { // DataMonitor (TUI M Key View)
            let mut report = String::new();
            report.push_str("=== SQLITE ANLIK SORGU EKLENTİSİ (QUERY ENGINE) ===\n\n");

            let running = state.is_running.load(Ordering::Relaxed);
            report.push_str(&format!("Durum: {}\n", if running { "ÇALIŞIYOR" } else { "DURDURULDU" }));

            let current_db_path = state.db_path.lock().unwrap().clone();
            report.push_str(&format!("Veritabanı Dosyası: {}\n", current_db_path));

            let reader_guard = state.reader.lock().unwrap();
            if let Some(reader) = reader_guard.as_ref() {
                let bytes = reader.get_db_file_size();
                let mb = bytes as f64 / (1024.0 * 1024.0);
                report.push_str(&format!("DB Boyutu: {:.2} MB\n\n", mb));

                report.push_str("[ Mevcut Tablolar ve Kayıt Sayıları ]\n");
                if let Ok(tables_res) = reader.list_tables() {
                    for row in &tables_res.rows {
                        if let Some(tbl_name) = row.get(0) {
                            let count_sql = format!("SELECT COUNT(*) FROM {}", tbl_name);
                            let cnt_str = reader.execute_sql(&count_sql)
                                .ok()
                                .and_then(|r| r.rows.get(0)?.get(0).cloned())
                                .unwrap_or_else(|| "0".to_string());
                            report.push_str(&format!("- {}: {} kayıt\n", tbl_name, cnt_str));
                        }
                    }
                }
                report.push_str("\n");

                let last_q = state.last_query.lock().unwrap().clone();
                let last_res = state.last_result.lock().unwrap().clone();
                if !last_q.is_empty() {
                    report.push_str(&format!("[ Son Çalıştırılan Sorgu ]\nSQL: {}\n", last_q));
                    report.push_str(&format!("Sonuç:\n{}\n", last_res));
                } else {
                    report.push_str("Henüz sorgu çalıştırılmadı. Shell üzerinden 'sql <QUERY>' yazarak sorgu atabilirsiniz.\n");
                }
            } else {
                report.push_str("\nVeritabanı henüz başlatılmadı.\n");
            }
            report.push_str("===================================================\n");

            let data = report.into_bytes();
            let len = data.len().min(out_max_len);
            if len > 0 {
                std::ptr::copy_nonoverlapping(data.as_ptr(), out_buf, len);
            }
            len
        }
        6 => { // Inbox (Incoming Query Commands)
            if payload_len > 0 {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                
                let mut sql_to_run = None;
                let mut is_tables_cmd = false;
                let mut schema_table = None;

                if let Ok(cmd_json) = serde_json::from_slice::<Value>(slice) {
                    if let Some(action) = cmd_json.get("action").and_then(|v| v.as_str()) {
                        match action {
                            "query" => {
                                if let Some(sql) = cmd_json.get("sql").and_then(|s| s.as_str()) {
                                    sql_to_run = Some(sql.to_string());
                                }
                            }
                            "tables" => {
                                is_tables_cmd = true;
                            }
                            "schema" => {
                                if let Some(tbl) = cmd_json.get("table").and_then(|t| t.as_str()) {
                                    schema_table = Some(tbl.to_string());
                                }
                            }
                            "last" => {
                                let tbl = cmd_json.get("table").and_then(|t| t.as_str()).unwrap_or("mark_prices");
                                let limit = cmd_json.get("limit").and_then(|l| l.as_i64()).unwrap_or(10);
                                sql_to_run = Some(format!("SELECT * FROM {} ORDER BY id DESC LIMIT {}", tbl, limit));
                            }
                            _ => {}
                        }
                    }
                } else if let Ok(raw_str) = std::str::from_utf8(slice) {
                    let trimmed = raw_str.trim();
                    if !trimmed.is_empty() {
                        sql_to_run = Some(trimmed.to_string());
                    }
                }

                let reader_guard = state.reader.lock().unwrap();
                if let Some(reader) = reader_guard.as_ref() {
                    let query_result = if is_tables_cmd {
                        reader.list_tables()
                    } else if let Some(tbl) = schema_table {
                        reader.get_schema(&tbl)
                    } else if let Some(sql) = sql_to_run {
                        *state.last_query.lock().unwrap() = sql.clone();
                        reader.execute_sql(&sql)
                    } else {
                        Err(rusqlite::Error::QueryReturnedNoRows)
                    };

                    let output_text = match query_result {
                        Ok(res) => res.formatted_output,
                        Err(e) => format!("Sorgu Hatası: {}\n", e),
                    };

                    *state.last_result.lock().unwrap() = output_text.clone();
                    state.outbox.lock().unwrap().push(output_text.clone());

                    // Also copy output_text to out_buf if caller provided a buffer
                    let data = output_text.as_bytes();
                    let len = data.len().min(out_max_len);
                    if len > 0 {
                        std::ptr::copy_nonoverlapping(data.as_ptr(), out_buf, len);
                        return len;
                    }
                }
            }
            0
        }
        7 => { // Outbox (Return queued responses)
            let mut out = state.outbox.lock().unwrap();
            if out.is_empty() {
                0
            } else {
                let msg = out.remove(0);
                let bytes = msg.as_bytes();
                let len = bytes.len().min(out_max_len);
                if len > 0 {
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, len);
                }
                len
            }
        }
        _ => 0,
    }
}
