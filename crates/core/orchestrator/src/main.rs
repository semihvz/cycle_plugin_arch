use cycle_finance_breakout_system::orchestrator::Orchestrator;
use cycle_finance_breakout_system::endpoint::StandardEndpoint;
use cycle_finance_breakout_system::system::{SystemInstance, RawEndpointFn};
use cycle_finance_breakout_system::tui_interface::{App, ViewMode, ActivePanel, draw_ui};

use crossterm::{
    event::{self, Event, KeyCode, MouseEventKind, MouseButton, EnableMouseCapture, DisableMouseCapture},
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
    ExecutableCommand,
};
use ratatui::{backend::CrosstermBackend, Terminal};
use ratatui::layout::{Rect, Layout, Direction, Constraint};
use std::io;
use std::sync::Arc;
use std::ffi::c_void;

/// Eklenti yükleme yardımcı fonksiyonu (C-ABI: init_plugin)
unsafe fn load_plugin_cabi(app: &mut App<'_>, plugin_name: &str) {
    let ext = if cfg!(target_os = "windows") { "dll" } 
              else if cfg!(target_os = "macos") { "dylib" } 
              else { "so" };
    let prefix = if cfg!(target_os = "windows") { "" } else { "lib" };
    let clean_name = plugin_name.strip_prefix("lib").unwrap_or(plugin_name);
    
    let mut lib_path_buf = get_plugin_dir();
    lib_path_buf.push(format!("{}{}.{}", prefix, clean_name, ext));
    let lib_path = lib_path_buf.to_string_lossy().to_string();
    
    match libloading::Library::new(&lib_path) {
        Ok(lib) => {
            // Yeni HFT C-ABI: init_plugin(state_out) -> RawEndpointFn
            type PluginInit = unsafe extern "C" fn(state_out: *mut *mut c_void) -> RawEndpointFn;
            match lib.get::<PluginInit>(b"init_plugin") {
                Ok(init_fn) => {
                    let mut state_ptr: *mut c_void = std::ptr::null_mut();
                    let endpoint_fn = init_fn(&mut state_ptr);
                    let sys = SystemInstance::new(
                        plugin_name.to_string(), 
                        plugin_name.to_string(), 
                        state_ptr, 
                        endpoint_fn,
                    );
                    app.orchestrator.register_system(sys);
                    Box::leak(Box::new(lib)); // Kütüphaneyi bellekte tut
                    app.log(&format!("{} eklentisi basariyla yuklendi (HFT/C-ABI).", plugin_name));
                }
                Err(_) => app.log(&format!("{} eklentisinde init_plugin fonksiyonu bulunamadi.", plugin_name)),
            }
        }
        Err(e) => app.log(&format!("{} eklentisi yuklenemedi (derlediginizden emin olun): {}", plugin_name, e)),
    }
}

fn get_plugin_dir() -> std::path::PathBuf {
    let mut dir = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("."));
    dir.pop(); // exe_name
    if dir.ends_with("deps") {
        dir.pop(); // go up to debug/release
    }
    dir
}

/// Eklenti tarama yardımcı fonksiyonu
fn scan_plugins() -> Vec<String> {
    let mut plugins = Vec::new();
    let lib_dir = get_plugin_dir();
    let prefix = if cfg!(target_os = "windows") { "" } else { "lib" };
    if let Ok(entries) = std::fs::read_dir(&lib_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(&format!("{}plugin_", prefix)) && (name.ends_with(".so") || name.ends_with(".dll") || name.ends_with(".dylib")) {
                let ext_len = if name.ends_with(".so") { 3 } else if name.ends_with(".dll") { 4 } else { 6 };
                let plugin_name = &name[prefix.len()..name.len()-ext_len];
                plugins.push(plugin_name.to_string());
            }
        }
    }
    plugins.sort();
    plugins.dedup();
    plugins
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ═══════════════════════════════════════════════════════
    // HFT: CPU Çekirdek Sabitleme (Core Pinning)
    // Ana thread → Çekirdek 0, Router thread → Çekirdek 1
    // ═══════════════════════════════════════════════════════
    let mut pinned_core = 0;
    if let Some(core_ids) = core_affinity::get_core_ids() {
        if let Some(core) = core_ids.first() {
            core_affinity::set_for_current(*core);
        }
        pinned_core = core_ids.first().map(|c| c.id).unwrap_or(0);
    }

    let (log_tx, _log_rx) = tokio::sync::broadcast::channel::<String>(200);

    let orchestrator = Arc::new(Orchestrator::new());
    let mut app = App::new(orchestrator.clone(), log_tx.clone());
    app.log(&format!("[HFT] Ana thread CPU çekirdeğine sabitlendi: Core {}", pinned_core));
    app.log("Sadece TUI Konsolu Başlatıldı. Web Arayüzü için Ayarlar sekmesinden veya [W] tuşuna basarak başlatabilirsiniz.");
    
    // --- FLOW ENGINE & CONFIG INITIALIZATION ---
    let config_path = if std::path::Path::new("config/config.json").exists() {
        "config/config.json"
    } else if std::path::Path::new("../config/config.json").exists() {
        "../config/config.json"
    } else if std::path::Path::new("../../config/config.json").exists() {
        "../../config/config.json"
    } else if std::path::Path::new("flow_config.json").exists() {
        "flow_config.json"
    } else {
        "config/config.json" // Default fallback
    };
    
    let flow_config = match flow_engine::FlowConfig::load(config_path) {
        Ok(c) => Some(c),
        Err(e) => {
            app.log(&format!("UYARI: config.json okunamadı: {}", e));
            None
        }
    };
    
    let mut engine_opt = None;
    if let Some(ref config) = flow_config {
        let engine = std::sync::Arc::new(flow_engine::FlowEngine::new(config.clone()));
        engine_opt = Some(engine.clone());
        app.log("Flow Engine config yüklendi. Router thread başlatılıyor...");

        let orc_clone = orchestrator.clone();
        let engine_clone = engine.clone();

        std::thread::spawn(move || {
            if let Some(core_ids) = core_affinity::get_core_ids() {
                if core_ids.len() > 1 {
                    core_affinity::set_for_current(core_ids[1]); // Router thread -> Core 1
                }
            }
            
            let mut last_health_check = std::time::Instant::now();
            loop {
                engine_clone.run_loop(|plugin_name, endpoint_id, payload, out_buf| {
                    let ep = match endpoint_id {
                        5 => StandardEndpoint::RawData,
                        6 => StandardEndpoint::Inbox,
                        7 => StandardEndpoint::Outbox,
                        _ => return 0,
                    };
                    orc_clone.call_endpoint(plugin_name, ep, payload, out_buf)
                });
                
                if last_health_check.elapsed().as_secs() >= 5 {
                    let warnings = engine_clone.health_check();
                    for _warning in warnings {}
                    last_health_check = std::time::Instant::now();
                }

                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        });
    }
    
    // Tüm pluginleri otomatik tara, yükle
    app.available_plugins = scan_plugins();
    for plugin_name in app.available_plugins.clone() {
        app.log(&format!("Otomatik yükleniyor: {}", plugin_name));
        unsafe { load_plugin_cabi(&mut app, &plugin_name); }
    }
    
    // Yüklenen tüm pluginleri başlat ve parametrelerini gönder (flow_config.json içinde tanımlı ve enabled==true olanlar başlatılır)
    let mut startup_buf = [0u8; 8];
    for (id, _, _) in app.orchestrator.list_systems() {
        if let Some(ref config) = flow_config {
            if let Some(plugin_conf) = config.iter().find(|p| p.plugin_name == id) {
                if plugin_conf.enabled {
                    let payload_bytes = serde_json::to_vec(&plugin_conf).unwrap_or_default();
                    app.orchestrator.call_endpoint(&id, StandardEndpoint::Start, &payload_bytes, &mut startup_buf);
                    app.log(&format!("Otomatik başlatıldı: {}", id));
                } else {
                    app.log(&format!("Başlatılmadı (flow_config.json içinde pasif/enabled=false): {}", id));
                }
            } else {
                app.log(&format!("Başlatılmadı (flow_config.json içinde tanımlı değil): {}", id));
            }
        }
    }
    
    app.log("Sistem başlatıldı ve eklentiler otomatik yüklendi. [HFT Modu: CPU Pinning AÇIK]");

    let web_server_started = Arc::new(std::sync::atomic::AtomicBool::new(false));

    // İnteraktif Komut Shell Ekranına Doğrudan Bağlan
    cycle_finance_breakout_system::interactive_shell::run_interactive_shell_loop(
        orchestrator.clone(),
        log_tx,
        None,
        web_server_started,
    ).await;

    Ok(())
}
