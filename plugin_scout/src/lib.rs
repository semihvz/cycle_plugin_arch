pub mod analyzer;
pub mod client;
pub mod config;
pub mod models;
pub mod service;
pub mod utils;

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

struct PluginState {
    runtime: tokio::runtime::Runtime,
    is_running: Arc<AtomicBool>,
    data: Arc<Mutex<Vec<u8>>>,
    shutdown_tx: Mutex<Option<tokio::sync::watch::Sender<bool>>>,
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(
    state_out: *mut *mut c_void,
) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("Tokio runtime olusturulamadi");

    let state = Box::new(PluginState {
        runtime,
        is_running: Arc::new(AtomicBool::new(false)),
        data: Arc::new(Mutex::new(b"{}".to_vec())),
        shutdown_tx: Mutex::new(None),
    });

    *state_out = Box::into_raw(state) as *mut c_void;
    handle_endpoint
}

unsafe extern "C" fn handle_endpoint(
    plugin_state: *mut c_void,
    endpoint_id: u32,
    _payload: *const u8,
    _payload_len: usize,
    out_buf: *mut u8,
    out_max_len: usize,
) -> usize {
    let state = &*(plugin_state as *const PluginState);

    match endpoint_id {
        0 => { // Start
            if state.is_running.load(Ordering::Relaxed) {
                return 0;
            }

            let is_running = state.is_running.clone();
            let (tx, mut rx) = tokio::sync::watch::channel(false);

            *state.shutdown_tx.lock().unwrap() = Some(tx);
            is_running.store(true, Ordering::Relaxed);

            let data_clone = state.data.clone();
            state.runtime.spawn(async move {
                let mut service = service::ScoutService::new(data_clone);
                if let Err(err) = service.start().await {
                    tracing::error!("ScoutService baslatilamadi: {}", err);
                }

                // Wait for shutdown signal
                let _ = rx.changed().await;
                service.stop().await;
            });
            0
        }
        1 => { // Stop
            state.is_running.store(false, Ordering::Relaxed);
            if let Some(tx) = state.shutdown_tx.lock().unwrap().take() {
                let _ = tx.send(true);
            }
            0
        }
        2 => { // IsWorking
            let running = state.is_running.load(Ordering::Relaxed);
            if out_max_len >= 1 {
                *out_buf = if running { 1 } else { 0 };
                1
            } else {
                0
            }
        }
        4 | 5 => { // DataMonitor / RawData
            let guard = state.data.lock().unwrap();
            let len = guard.len().min(out_max_len);
            if len > 0 {
                std::ptr::copy_nonoverlapping(guard.as_ptr(), out_buf, len);
            }
            len
        }
        _ => 0,
    }
}
