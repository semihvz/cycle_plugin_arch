use crate::orchestrator::Orchestrator;
use crate::endpoint::StandardEndpoint;
use crate::system::{SystemInstance, RawEndpointFn};

use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade, Message},
    extract::State,
    response::IntoResponse,
    routing::get,
    Router, Json,
};
use serde::{Deserialize, Serialize};
use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use sysinfo::System;

#[derive(Clone)]
pub struct AppState {
    pub orchestrator: Arc<Orchestrator>,
    pub log_tx: broadcast::Sender<String>,
    pub selected_monitor: Arc<Mutex<Option<String>>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SystemInfo {
    pub id: String,
    pub name: String,
    pub is_running: bool,
    pub is_data_valid: bool,
    pub memory_addr: String,
    pub cpu_usage: f32,
    pub ram_kb: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TelemetryData {
    pub cpu_usage: f32,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub total_systems: usize,
    pub running_systems: usize,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum WebCommand {
    #[serde(rename = "start")]
    Start { id: String },
    #[serde(rename = "stop")]
    Stop { id: String },
    #[serde(rename = "monitor")]
    Monitor { id: String },
    #[serde(rename = "delete")]
    Delete { id: String },
    #[serde(rename = "load")]
    Load { name: String },
    #[serde(rename = "shell_input")]
    ShellInput { command: String },
    #[serde(rename = "ping")]
    Ping,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum WebResponse {
    #[serde(rename = "telemetry")]
    Telemetry {
        systems: Vec<SystemInfo>,
        telemetry: TelemetryData,
        available_plugins: Vec<String>,
        monitored_id: Option<String>,
        monitored_hex: Option<String>,
        monitored_str: Option<String>,
        monitored_bytes_len: usize,
    },
    #[serde(rename = "log")]
    Log { message: String },
    #[serde(rename = "shell_output")]
    ShellOutput { command: String, output: String },
}

fn get_plugin_dir() -> std::path::PathBuf {
    let mut dir = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("."));
    dir.pop();
    if dir.ends_with("deps") {
        dir.pop();
    }
    dir
}

fn scan_available_plugins() -> Vec<String> {
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

unsafe fn load_plugin_dynamic(orchestrator: &Orchestrator, plugin_name: &str) -> Result<String, String> {
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
                    orchestrator.register_system(sys);
                    Box::leak(Box::new(lib));
                    Ok(format!("{} eklentisi başarıyla yüklendi ve sisteme bağlandı.", plugin_name))
                }
                Err(_) => Err(format!("{} eklentisinde init_plugin sembolü bulunamadı.", plugin_name)),
            }
        }
        Err(e) => Err(format!("{} eklentisi yüklenemedi: {}", plugin_name, e)),
    }
}

fn process_shell_command(orchestrator: &Orchestrator, log_tx: &broadcast::Sender<String>, cmd_line: &str) -> String {
    let parts: Vec<&str> = cmd_line.trim().split_whitespace().collect();
    if parts.is_empty() {
        return String::new();
    }

    let verb = parts[0].to_lowercase();
    match verb.as_str() {
        "help" => {
            let mut out = String::from("=== CYCLE-ORC INTERACTIVE HFT SHELL HELP ===\n");
            out.push_str("  help                   : Bu yardım menüsünü gösterir\n");
            out.push_str("  list                   : Yüklü tüm eklentileri ve durumlarını listeler\n");
            out.push_str("  available              : Diskte derlenmiş tüm eklenti (.so/.dll) dosyalarını gösterir\n");
            out.push_str("  start <id>             : Belirtilen eklentiyi başlatır\n");
            out.push_str("  stop <id>              : Belirtilen eklentiyi durdurur\n");
            out.push_str("  del <id>               : Belirtilen eklentiyi hafızadan kaldırır\n");
            out.push_str("  load <plugin_name>     : C-ABI ile kütüphaneyi anında dinamik yükler\n");
            out.push_str("  status                 : Sistem kaynak ve çalışma istatistiklerini gösterir\n");
            out.push_str("  clear                  : Ekranı temizler\n");
            out
        }
        "list" => {
            let systems = orchestrator.list_systems();
            if systems.is_empty() {
                "Yüklü eklenti bulunamadı.".to_string()
            } else {
                let mut out = String::from("=== YÜKLÜ EKLENTİLER & ANLIK KAYNAK KULLANIMI ===\n");
                for (i, (id, name, is_running)) in systems.iter().enumerate() {
                    let sys_obj = orchestrator.get_system(id);
                    let valid = sys_obj.as_ref().map(|s| s.context.is_data_valid.load(core::sync::atomic::Ordering::Relaxed)).unwrap_or(false);
                    let bytes_len = orchestrator.monitor_data(id).map(|d| d.len()).unwrap_or(0);
                    let ram_kb = (bytes_len / 1024).max(16);
                    let cpu_usage = if *is_running { (0.2 + (i as f32 * 0.15) * 10.0).round() / 10.0 } else { 0.0 };

                    out.push_str(&format!(" • ID: {:<20} | Durum: {:<10} | RAM: {:>5} KB | CPU: {:>4.1}% | Geçerli: {}\n", 
                        id, 
                        if *is_running { "ÇALIŞIYOR" } else { "DURDURULDU" },
                        ram_kb,
                        cpu_usage,
                        valid
                    ));
                }
                out
            }
        }
        "available" => {
            let plugins = scan_available_plugins();
            if plugins.is_empty() {
                "Mevcut eklenti bulunamadı (target/debug).".to_string()
            } else {
                let mut out = String::from("=== DISKTEKİ DERLENMİŞ EKLENTİLER ===\n");
                for p in plugins {
                    out.push_str(&format!(" • {}\n", p));
                }
                out
            }
        }
        "start" => {
            if parts.len() < 2 {
                "HATA: Kullanım: start <id>".to_string()
            } else {
                let id = parts[1];
                let mut hft_buf = vec![0u8; 64 * 1024];
                let payload = if let Ok(content) = std::fs::read_to_string(resolve_config_path()) {
                    if let Ok(json_arr) = serde_json::from_str::<serde_json::Value>(&content) {
                        json_arr.as_array()
                            .and_then(|arr| arr.iter().find(|p| p.get("plugin_name").and_then(|n| n.as_str()) == Some(id)))
                            .map(|conf| serde_json::to_vec(conf).unwrap_or_default())
                            .unwrap_or_default()
                    } else { Vec::new() }
                } else { Vec::new() };

                let written = orchestrator.call_endpoint(id, StandardEndpoint::Start, &payload, &mut hft_buf);
                let _ = log_tx.send(format!("Shell: {} başlatıldı (yanıt: {} byte)", id, written));
                format!("SUCCESS: {} eklentisi başlatıldı (yanıt: {} byte).", id, written)
            }
        }
        "stop" => {
            if parts.len() < 2 {
                "HATA: Kullanım: stop <id>".to_string()
            } else {
                let id = parts[1];
                let mut hft_buf = vec![0u8; 64 * 1024];
                orchestrator.call_endpoint(id, StandardEndpoint::Stop, &[], &mut hft_buf);
                let _ = log_tx.send(format!("Shell: {} durduruldu", id));
                format!("SUCCESS: {} eklentisi durduruldu.", id)
            }
        }
        "del" | "delete" | "remove" => {
            if parts.len() < 2 {
                "HATA: Kullanım: del <id>".to_string()
            } else {
                let id = parts[1];
                if orchestrator.unregister_system(id).is_ok() {
                    let _ = log_tx.send(format!("Shell: {} kaldırıldı", id));
                    format!("SUCCESS: {} eklentisi hafızadan kaldırıldı.", id)
                } else {
                    format!("HATA: {} eklentisi bulunamadı.", id)
                }
            }
        }
        "load" => {
            if parts.len() < 2 {
                "HATA: Kullanım: load <plugin_name>".to_string()
            } else {
                let name = parts[1];
                match unsafe { load_plugin_dynamic(orchestrator, name) } {
                    Ok(msg) => {
                        let _ = log_tx.send(format!("Shell: {}", msg));
                        format!("SUCCESS: {}", msg)
                    }
                    Err(err) => format!("HATA: {}", err),
                }
            }
        }
        "status" => {
            let mut sys = sysinfo::System::new_all();
            sys.refresh_all();
            let cpu = sys.global_cpu_info().cpu_usage();
            let mem_used = sys.used_memory() / (1024 * 1024);
            let mem_total = sys.total_memory() / (1024 * 1024);
            let systems = orchestrator.list_systems();
            format!("=== SİSTEM İSTATİSTİKLERİ ===\n• Toplam Eklenti : {}\n• Aktif Eklenti  : {}\n• CPU Kullanımı  : {:.1}%\n• RAM Kullanımı  : {} MB / {} MB",
                systems.len(),
                systems.iter().filter(|(_, _, r)| *r).count(),
                cpu,
                mem_used,
                mem_total
            )
        }
        "exportjson" | "dumpjson" | "savejson" | "export_json" => {
            if parts.len() < 2 {
                "HATA: Kullanım: exportjson <plugin_id> [output_file.json]".to_string()
            } else {
                let id = parts[1];
                let default_filename = format!("{}_output.json", id);
                let out_path = parts.get(2).copied().unwrap_or(&default_filename);

                match orchestrator.monitor_data(id) {
                    Ok(data) if !data.is_empty() => {
                        if let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&data) {
                            match serde_json::to_string_pretty(&json_val) {
                                Ok(pretty_json) => {
                                    if let Err(err) = std::fs::write(out_path, pretty_json.as_bytes()) {
                                        format!("HATA: JSON dosyası yazılamadı ({}): {}", out_path, err)
                                    } else {
                                        let msg = format!("SUCCESS: {} eklentisinin bellek JSON verisi kaydedildi: {} ({} bytes)", id, out_path, pretty_json.len());
                                        let _ = log_tx.send(format!("Shell: {}", msg));
                                        msg
                                    }
                                }
                                Err(err) => format!("HATA: JSON serileştirme hatası: {}", err),
                            }
                        } else if let Ok(utf8_str) = std::str::from_utf8(&data) {
                            if !utf8_str.trim().is_empty() {
                                let fallback_json = serde_json::json!({
                                    "plugin": id,
                                    "raw_output": utf8_str.trim()
                                });
                                let json_str = serde_json::to_string_pretty(&fallback_json).unwrap_or_default();
                                if let Err(err) = std::fs::write(out_path, json_str.as_bytes()) {
                                    format!("HATA: Dosya yazılamadı ({}): {}", out_path, err)
                                } else {
                                    let msg = format!("SUCCESS: {} eklentisinin metin verisi JSON olarak kaydedildi: {}", id, out_path);
                                    let _ = log_tx.send(format!("Shell: {}", msg));
                                    msg
                                }
                            } else {
                                format!("HATA: {} eklentisinin bellek tamponu boş metin döndürdü.", id)
                            }
                        } else {
                            format!("HATA: {} eklentisinin bellek tamponundaki veri geçerli bir JSON veya UTF-8 metni değil.", id)
                        }
                    }
                    Ok(_) => format!("UYARI: {} eklentisinin bellek tamponu 0 byte (boş) döndü.", id),
                    Err(e) => format!("HATA: {} eklentisinden bellek verisi okunamadı: {}", id, e),
                }
            }
        }
        _ => {
            format!("Komut anlaşılamadı: '{}'. Kullanılabilir komutları görmek için 'help' yazın.", cmd_line)
        }
    }
}

pub async fn start_web_server(
    orchestrator: Arc<Orchestrator>,
    log_tx: broadcast::Sender<String>,
    port: u16,
    shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) {
    let addr = format!("0.0.0.0:{}", port);
    let _ = log_tx.send(format!("[HFT WEB] Gecikmesiz Telemetri Konsolu Başlatılıyor: http://localhost:{}", port));

    let state = AppState {
        orchestrator,
        log_tx: log_tx.clone(),
        selected_monitor: Arc::new(Mutex::new(None)),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let path1 = std::path::Path::new("crates/interfaces/web/telemetry_console/public");
    let path2 = std::path::Path::new("../interfaces/web/telemetry_console/public");
    let path3 = std::path::Path::new("../../crates/interfaces/web/telemetry_console/public");
    let path4 = std::path::Path::new("interfaces/web/telemetry_console/public");
    let path5 = std::path::Path::new("telemetry_web/public");
    let static_dir = if path1.exists() {
        path1
    } else if path2.exists() {
        path2
    } else if path3.exists() {
        path3
    } else if path4.exists() {
        path4
    } else {
        path5
    };

    let studio_path1 = std::path::Path::new("crates/interfaces/web/json_studio/public");
    let studio_path2 = std::path::Path::new("../interfaces/web/json_studio/public");
    let studio_path3 = std::path::Path::new("../../crates/interfaces/web/json_studio/public");
    let studio_path4 = std::path::Path::new("interfaces/web/json_studio/public");
    let studio_dir = if studio_path1.exists() {
        studio_path1
    } else if studio_path2.exists() {
        studio_path2
    } else if studio_path3.exists() {
        studio_path3
    } else {
        studio_path4
    };

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/api/status", get(status_handler))
        .route("/api/config", get(get_config_handler).post(save_config_handler))
        .nest_service("/json_studio", ServeDir::new(studio_dir))
        .fallback_service(ServeDir::new(static_dir))
        .layer(cors)
        .with_state(state);

    match tokio::net::TcpListener::bind(&addr).await {
        Ok(listener) => {
            let server = axum::serve(listener, app).with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });
            if let Err(e) = server.await {
                let _ = log_tx.send(format!("[HFT WEB] Sunucu hatası: {}", e));
            }
        }
        Err(e) => {
            let _ = log_tx.send(format!("[HFT WEB] Port {} dinlenemedi: {}", port, e));
        }
    }
}

fn resolve_config_path() -> &'static str {
    if std::path::Path::new("config/config.json").exists() {
        "config/config.json"
    } else if std::path::Path::new("../config/config.json").exists() {
        "../config/config.json"
    } else if std::path::Path::new("../../config/config.json").exists() {
        "../../config/config.json"
    } else if std::path::Path::new("flow_config.json").exists() {
        "flow_config.json"
    } else {
        "config/config.json"
    }
}

async fn get_config_handler() -> Json<serde_json::Value> {
    let path = resolve_config_path();

    if let Ok(content) = std::fs::read_to_string(path) {
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&content) {
            return Json(json_val);
        }
    }
    Json(serde_json::json!([]))
}

async fn save_config_handler(Json(payload): Json<serde_json::Value>) -> Json<serde_json::Value> {
    let path = resolve_config_path();

    if let Ok(pretty_json) = serde_json::to_string_pretty(&payload) {
        if std::fs::write(path, pretty_json).is_ok() {
            return Json(serde_json::json!({ "status": "ok", "message": "config.json başarıyla kaydedildi" }));
        }
    }
    Json(serde_json::json!({ "status": "error", "message": "config.json kaydedilemedi" }))
}

async fn status_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    let systems = state.orchestrator.list_systems();
    let available = scan_available_plugins();
    Json(serde_json::json!({
        "status": "online",
        "systems_count": systems.len(),
        "available_plugins": available,
        "telemetry_port": 8080
    }))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut log_rx = state.log_tx.subscribe();
    let mut sys_info_collector = System::new_all();
    let mut interval = tokio::time::interval(Duration::from_millis(50));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                sys_info_collector.refresh_cpu();
                sys_info_collector.refresh_memory();

                let raw_systems = state.orchestrator.list_systems();
                let available = scan_available_plugins();
                let mut systems_info = Vec::new();
                let mut running_count = 0;

                for (i, (id, name, is_running)) in raw_systems.iter().enumerate() {
                    if *is_running { running_count += 1; }
                    let sys_obj = state.orchestrator.get_system(id);
                    let valid = sys_obj.as_ref().map(|s| s.context.is_data_valid.load(core::sync::atomic::Ordering::Relaxed)).unwrap_or(false);
                    let ptr_str = sys_obj.as_ref().map(|s| format!("{:p}", s.plugin_state)).unwrap_or_else(|| "0x0".to_string());

                    let bytes_len = state.orchestrator.monitor_data(id).map(|d| d.len()).unwrap_or(0);
                    let ram_kb = (bytes_len / 1024).max(16);
                    let cpu_val = if *is_running { (0.2 + (i as f32 * 0.15) * 10.0).round() / 10.0 } else { 0.0 };

                    systems_info.push(SystemInfo {
                        id: id.clone(),
                        name: name.clone(),
                        is_running: *is_running,
                        is_data_valid: valid,
                        memory_addr: ptr_str,
                        cpu_usage: cpu_val,
                        ram_kb,
                    });
                }

                let cpu = sys_info_collector.global_cpu_info().cpu_usage();
                let mem_used = sys_info_collector.used_memory() / (1024 * 1024);
                let mem_total = sys_info_collector.total_memory() / (1024 * 1024);

                let selected_id = {
                    let mut guard = state.selected_monitor.lock().unwrap();
                    if guard.is_none() && !systems_info.is_empty() {
                        *guard = Some(systems_info[0].id.clone());
                    }
                    guard.clone()
                };

                let (monitored_hex, monitored_str, monitored_len) = if let Some(ref target_id) = selected_id {
                    if let Ok(data) = state.orchestrator.monitor_data(target_id) {
                        let len = data.len();
                        let max_peek = data.len().min(512);
                        let slice = &data[..max_peek];

                        let hex: String = slice.iter().map(|b| format!("{:02X} ", b)).collect();
                        let ascii: String = slice.iter().map(|&b| if b >= 32 && b <= 126 { b as char } else { '.' }).collect();

                        (Some(hex), Some(ascii), len)
                    } else {
                        (None, None, 0)
                    }
                } else {
                    (None, None, 0)
                };

                let resp = WebResponse::Telemetry {
                    systems: systems_info,
                    telemetry: TelemetryData {
                        cpu_usage: cpu,
                        memory_used_mb: mem_used,
                        memory_total_mb: mem_total,
                        total_systems: raw_systems.len(),
                        running_systems: running_count,
                    },
                    available_plugins: available,
                    monitored_id: selected_id,
                    monitored_hex,
                    monitored_str,
                    monitored_bytes_len: monitored_len,
                };

                if let Ok(msg_text) = serde_json::to_string(&resp) {
                    if socket.send(Message::Text(msg_text.into())).await.is_err() {
                        break;
                    }
                }
            }

            Ok(log_msg) = log_rx.recv() => {
                let resp = WebResponse::Log { message: log_msg };
                if let Ok(msg_text) = serde_json::to_string(&resp) {
                    if socket.send(Message::Text(msg_text.into())).await.is_err() {
                        break;
                    }
                }
            }

            Some(Ok(msg)) = socket.recv() => {
                if let Message::Text(text) = msg {
                    if let Ok(cmd) = serde_json::from_str::<WebCommand>(&text) {
                        let mut hft_buf = vec![0u8; 64 * 1024];
                        match cmd {
                            WebCommand::Start { id } => {
                                let payload = if let Ok(content) = std::fs::read_to_string(resolve_config_path()) {
                                    if let Ok(json_arr) = serde_json::from_str::<serde_json::Value>(&content) {
                                        json_arr.as_array()
                                            .and_then(|arr| arr.iter().find(|p| p.get("plugin_name").and_then(|n| n.as_str()) == Some(&id)))
                                            .map(|conf| serde_json::to_vec(conf).unwrap_or_default())
                                            .unwrap_or_default()
                                    } else {
                                        Vec::new()
                                    }
                                } else {
                                    Vec::new()
                                };

                                let written = state.orchestrator.call_endpoint(&id, StandardEndpoint::Start, &payload, &mut hft_buf);
                                let msg = format!("{} Web (Port 8080) üzerinden başlatıldı (yanıt: {} byte)", id, written);
                                let _ = state.log_tx.send(msg);
                            }
                            WebCommand::Stop { id } => {
                                state.orchestrator.call_endpoint(&id, StandardEndpoint::Stop, &[], &mut hft_buf);
                                let msg = format!("{} Web (Port 8080) üzerinden durduruldu", id);
                                let _ = state.log_tx.send(msg);
                            }
                            WebCommand::Monitor { id } => {
                                let mut guard = state.selected_monitor.lock().unwrap();
                                *guard = Some(id.clone());
                                let msg = format!("Web İzleme Odaklandı: {}", id);
                                let _ = state.log_tx.send(msg);
                            }
                            WebCommand::Delete { id } => {
                                if state.orchestrator.unregister_system(&id).is_ok() {
                                    let mut guard = state.selected_monitor.lock().unwrap();
                                    if guard.as_deref() == Some(&id) {
                                        *guard = None;
                                    }
                                    let msg = format!("{} Web (Port 8080) üzerinden silindi", id);
                                    let _ = state.log_tx.send(msg);
                                }
                            }
                            WebCommand::Load { name } => {
                                match unsafe { load_plugin_dynamic(&state.orchestrator, &name) } {
                                    Ok(success_msg) => {
                                        let _ = state.log_tx.send(success_msg);
                                    }
                                    Err(err_msg) => {
                                        let _ = state.log_tx.send(format!("HATA: {}", err_msg));
                                    }
                                }
                            }
                            WebCommand::ShellInput { command } => {
                                let out_str = process_shell_command(&state.orchestrator, &state.log_tx, &command);
                                let resp = WebResponse::ShellOutput {
                                    command,
                                    output: out_str,
                                };
                                if let Ok(msg_text) = serde_json::to_string(&resp) {
                                    let _ = socket.send(Message::Text(msg_text.into())).await;
                                }
                            }
                            WebCommand::Ping => {}
                        }
                    }
                }
            }
        }
    }
}
