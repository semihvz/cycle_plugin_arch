use crate::orchestrator::Orchestrator;
use crate::endpoint::StandardEndpoint;

use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade, Message},
    extract::State,
    response::IntoResponse,
    routing::get,
    Router, Json,
};
use serde::{Deserialize, Serialize};
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
        monitored_id: Option<String>,
        monitored_hex: Option<String>,
        monitored_str: Option<String>,
        monitored_bytes_len: usize,
    },
    #[serde(rename = "log")]
    Log { message: String },
}

pub async fn start_web_server(
    orchestrator: Arc<Orchestrator>,
    log_tx: broadcast::Sender<String>,
    port: u16,
) {
    let state = AppState {
        orchestrator,
        log_tx,
        selected_monitor: Arc::new(Mutex::new(None)),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let path1 = std::path::Path::new("web_interfaces/telemetry_console/public");
    let path2 = std::path::Path::new("../web_interfaces/telemetry_console/public");
    let path3 = std::path::Path::new("telemetry_web/public");
    let static_dir = if path1.exists() {
        path1
    } else if path2.exists() {
        path2
    } else {
        path3
    };

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/api/status", get(status_handler))
        .fallback_service(ServeDir::new(static_dir))
        .layer(cors)
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    eprintln!("[HFT WEB] Gecikmesiz Telemetri Konsolu Başlatılıyor: http://localhost:{}", port);

    match tokio::net::TcpListener::bind(&addr).await {
        Ok(listener) => {
            if let Err(e) = axum::serve(listener, app).await {
                eprintln!("[HFT WEB] Sunucu hatası: {}", e);
            }
        }
        Err(e) => {
            eprintln!("[HFT WEB] Port {} dinlenemedi: {}", port, e);
        }
    }
}

async fn status_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    let systems = state.orchestrator.list_systems();
    Json(serde_json::json!({
        "status": "online",
        "systems_count": systems.len(),
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
                let mut systems_info = Vec::new();
                let mut running_count = 0;

                for (id, name, is_running) in &raw_systems {
                    if *is_running { running_count += 1; }
                    let sys_obj = state.orchestrator.get_system(id);
                    let valid = sys_obj.as_ref().map(|s| s.context.is_data_valid.load(core::sync::atomic::Ordering::Relaxed)).unwrap_or(false);
                    let ptr_str = sys_obj.as_ref().map(|s| format!("{:p}", s.plugin_state)).unwrap_or_else(|| "0x0".to_string());

                    systems_info.push(SystemInfo {
                        id: id.clone(),
                        name: name.clone(),
                        is_running: *is_running,
                        is_data_valid: valid,
                        memory_addr: ptr_str,
                    });
                }

                let cpu = sys_info_collector.global_cpu_info().cpu_usage();
                let mem_used = sys_info_collector.used_memory() / (1024 * 1024);
                let mem_total = sys_info_collector.total_memory() / (1024 * 1024);

                let selected_id = {
                    let guard = state.selected_monitor.lock().unwrap();
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
                                let payload = if let Ok(content) = std::fs::read_to_string("flow_config.json") {
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
                            WebCommand::Ping => {}
                        }
                    }
                }
            }
        }
    }
}
