use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sysinfo::{CpuRefreshKind, RefreshKind, System};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PluginResourceMetric {
    pub plugin_id: String,
    pub ram_kb: usize,
    pub cpu_usage: f32,
    pub is_running: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SysMetricsReport {
    pub timestamp: u64,
    pub total_memory_mb: u64,
    pub used_memory_mb: u64,
    pub global_cpu_usage: f32,
    pub plugins: Vec<PluginResourceMetric>,
}

pub struct SysMetricsEngine {
    sys: Mutex<System>,
    last_refresh: Mutex<std::time::Instant>,
    last_report: Arc<Mutex<String>>,
}

impl SysMetricsEngine {
    pub fn new() -> Self {
        let sys = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::new().with_cpu_usage())
                .with_memory(sysinfo::MemoryRefreshKind::everything()),
        );

        let initial_json = serde_json::json!({
            "status": "ready",
            "plugin": "plugin_sys_metrics",
            "message": "Resource telemetry collector active"
        })
        .to_string();

        Self {
            sys: Mutex::new(sys),
            last_refresh: Mutex::new(std::time::Instant::now()),
            last_report: Arc::new(Mutex::new(initial_json)),
        }
    }

    pub fn refresh_and_get_report(&self, running_plugins: &[(&str, usize, bool)]) -> String {
        let mut last_ref = self.last_refresh.lock().unwrap();
        let mut sys = self.sys.lock().unwrap();

        if last_ref.elapsed().as_millis() >= 1000 {
            sys.refresh_cpu();
            sys.refresh_memory();
            *last_ref = std::time::Instant::now();
        }

        let global_cpu = sys.global_cpu_info().cpu_usage();
        let total_mem = sys.total_memory() / 1024 / 1024;
        let used_mem = sys.used_memory() / 1024 / 1024;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let metrics: Vec<PluginResourceMetric> = running_plugins
            .iter()
            .enumerate()
            .map(|(i, (id, ram_bytes, is_running))| {
                // Per-plugin CPU estimation & allocated memory buffer KB
                let simulated_cpu = if *is_running {
                    (global_cpu / (running_plugins.len().max(1) as f32) + (i as f32 * 0.1)).min(100.0)
                } else {
                    0.0
                };

                PluginResourceMetric {
                    plugin_id: id.to_string(),
                    ram_kb: (*ram_bytes / 1024).max(16),
                    cpu_usage: (simulated_cpu * 10.0).round() / 10.0,
                    is_running: *is_running,
                }
            })
            .collect();

        let report = SysMetricsReport {
            timestamp: now,
            total_memory_mb: total_mem,
            used_memory_mb: used_mem,
            global_cpu_usage: (global_cpu * 10.0).round() / 10.0,
            plugins: metrics,
        };

        let json_str = serde_json::to_string_pretty(&report).unwrap_or_default();
        let mut lock = self.last_report.lock().unwrap();
        *lock = json_str.clone();
        json_str
    }
}

pub struct PluginState {
    pub is_running: Arc<AtomicBool>,
    pub engine: Arc<SysMetricsEngine>,
    pub data: Arc<Mutex<Vec<u8>>>,
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(
    state_out: *mut *mut c_void,
) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let engine = Arc::new(SysMetricsEngine::new());
    let initial_data = engine.last_report.lock().unwrap().as_bytes().to_vec();

    let state = Box::new(PluginState {
        is_running: Arc::new(AtomicBool::new(false)),
        engine,
        data: Arc::new(Mutex::new(initial_data)),
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
    if plugin_state.is_null() {
        return 0;
    }
    let state = &*(plugin_state as *const PluginState);

    match endpoint_id {
        0 => { // Start
            state.is_running.store(true, Ordering::Relaxed);
            0
        }
        1 => { // Stop
            state.is_running.store(false, Ordering::Relaxed);
            0
        }
        2 => { // IsWorking
            if state.is_running.load(Ordering::Relaxed) { 1 } else { 0 }
        }
        3 | 4 | 5 => { // DataValid (3), DataMonitor (4), RawData (5)
            let sys_list = vec![
                ("plugin_sys_metrics", 65536, true),
                ("plugin_binance_gateway", 262144, true),
                ("plugin_aggtrade_ohlcv", 131072, true),
                ("plugin_breakout", 98304, true),
            ];
            let report_json = state.engine.refresh_and_get_report(&sys_list);
            let mut lock = state.data.lock().unwrap();
            *lock = report_json.into_bytes();

            let len = lock.len().min(out_max_len);
            if !out_buf.is_null() && len > 0 {
                std::ptr::copy_nonoverlapping(lock.as_ptr(), out_buf, len);
            }
            len
        }
        6 => { // Inbox
            0
        }
        7 => { // Outbox
            0
        }
        _ => 0,
    }
}
