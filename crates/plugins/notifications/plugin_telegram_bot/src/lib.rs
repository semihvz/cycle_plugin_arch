use std::ffi::c_void;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TelegramBotConfig {
    pub api_mode: String,               // "telegram_api", "webhook"
    pub bot_token: String,              // Telegram Bot Token from @BotFather
    pub chat_id: String,                // Default target recipient Chat ID or Channel Username
    pub admin_chat_ids: Vec<String>,    // List of authorized admin chat IDs
    pub webhook_port: u16,              // Port for receiving incoming Telegram webhooks
    pub webhook_verify_token: String,    // Optional webhook verification token
    pub auto_reply_enabled: bool,       // Automatically reply to commands via Telegram
}

impl Default for TelegramBotConfig {
    fn default() -> Self {
        Self {
            api_mode: "telegram_api".to_string(),
            bot_token: "123456789:YOUR_TELEGRAM_BOT_TOKEN_HERE".to_string(),
            chat_id: "@your_channel_or_chat_id".to_string(),
            admin_chat_ids: vec!["123456789".to_string()],
            webhook_port: 8085,
            webhook_verify_token: "CYCLE_ORC_TELEGRAM_VERIFY".to_string(),
            auto_reply_enabled: true,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TelegramMessageRecord {
    pub direction: String, // "inbound" or "outbound"
    pub sender: String,
    pub recipient: String,
    pub content: String,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TelegramBotStats {
    pub sent_count: usize,
    pub received_count: usize,
    pub failed_count: usize,
    pub last_incoming_cmd: String,
    pub last_outgoing_msg: String,
    pub server_status: String,
    pub active_mode: String,
    pub webhook_port: u16,
}

struct PluginState {
    runtime: tokio::runtime::Runtime,
    is_running: Arc<AtomicBool>,
    config: Arc<Mutex<TelegramBotConfig>>,
    stats: Arc<Mutex<TelegramBotStats>>,
    recent_messages: Arc<Mutex<Vec<TelegramMessageRecord>>>,
    outbox: Arc<Mutex<Vec<serde_json::Value>>>,
    data: Arc<Mutex<Vec<u8>>>,
}

fn get_config_path() -> String {
    let paths = [
        "config/telegram_bot.cfg",
        "../config/telegram_bot.cfg",
        "../../config/telegram_bot.cfg",
        "telegram_bot.cfg",
    ];
    for p in &paths {
        if Path::new(p).exists() {
            return p.to_string();
        }
    }
    "config/telegram_bot.cfg".to_string()
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(
    state_out: *mut *mut c_void,
) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("Failed to initialize Tokio runtime for plugin_telegram_bot");

    let cfg_path = get_config_path();
    let config = if let Ok(content) = fs::read_to_string(&cfg_path) {
        serde_json::from_str::<TelegramBotConfig>(&content).unwrap_or_default()
    } else {
        let _ = fs::create_dir_all("config");
        let default_cfg = TelegramBotConfig::default();
        let _ = fs::write(
            &cfg_path,
            serde_json::to_string_pretty(&default_cfg).unwrap_or_default(),
        );
        default_cfg
    };

    let initial_stats = TelegramBotStats {
        sent_count: 0,
        received_count: 0,
        failed_count: 0,
        last_incoming_cmd: "None".to_string(),
        last_outgoing_msg: "None".to_string(),
        server_status: "Initialized".to_string(),
        active_mode: config.api_mode.clone(),
        webhook_port: config.webhook_port,
    };

    let initial_monitor = json!({
        "plugin": "plugin_telegram_bot",
        "status": "ready",
        "config": config,
        "stats": initial_stats,
        "recent_messages": []
    })
    .to_string();

    let state = Box::new(PluginState {
        runtime,
        is_running: Arc::new(AtomicBool::new(false)),
        config: Arc::new(Mutex::new(config)),
        stats: Arc::new(Mutex::new(initial_stats)),
        recent_messages: Arc::new(Mutex::new(Vec::new())),
        outbox: Arc::new(Mutex::new(Vec::new())),
        data: Arc::new(Mutex::new(initial_monitor.into_bytes())),
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
        0 => {
            // Start
            if state.is_running.load(Ordering::Relaxed) {
                return 0;
            }
            state.is_running.store(true, Ordering::Relaxed);

            // Re-read configuration if payload passed
            if payload_len > 0 {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                if let Ok(new_cfg) = serde_json::from_slice::<TelegramBotConfig>(slice) {
                    let mut cfg_guard = state.config.lock().unwrap();
                    *cfg_guard = new_cfg;
                }
            }

            let is_running = state.is_running.clone();
            let config = state.config.clone();
            let stats = state.stats.clone();
            let recent = state.recent_messages.clone();
            let outbox = state.outbox.clone();
            let data = state.data.clone();

            state.runtime.spawn(async move {
                let (port, token) = {
                    let c = config.lock().unwrap();
                    (c.webhook_port, c.bot_token.clone())
                };

                {
                    let mut s = stats.lock().unwrap();
                    s.server_status = format!("Active on port {}", port);
                }

                // Start Webhook listener HTTP server
                let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await;
                match listener {
                    Ok(listener) => {
                        while is_running.load(Ordering::Relaxed) {
                            tokio::select! {
                                accept_res = listener.accept() => {
                                    if let Ok((mut stream, _addr)) = accept_res {
                                        let config_cloned = config.clone();
                                        let stats_cloned = stats.clone();
                                        let recent_cloned = recent.clone();
                                        let outbox_cloned = outbox.clone();
                                        let data_cloned = data.clone();
                                        let token_cloned = token.clone();

                                        tokio::spawn(async move {
                                            let mut buf = vec![0u8; 8192];
                                            use tokio::io::{AsyncReadExt, AsyncWriteExt};
                                            if let Ok(n) = stream.read(&mut buf).await {
                                                if n > 0 {
                                                    let req_str = String::from_utf8_lossy(&buf[..n]);
                                                    
                                                    // Process incoming HTTP / Webhook request
                                                    let (reply_body, status_code) = handle_http_request(
                                                        &req_str,
                                                        &config_cloned,
                                                        &stats_cloned,
                                                        &recent_cloned,
                                                        &outbox_cloned,
                                                        &token_cloned
                                                    ).await;

                                                    let response = format!(
                                                        "HTTP/1.1 {} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                                        status_code,
                                                        reply_body.len(),
                                                        reply_body
                                                    );
                                                    let _ = stream.write_all(response.as_bytes()).await;

                                                    // Update DataMonitor payload
                                                    update_monitor_data(&config_cloned, &stats_cloned, &recent_cloned, &data_cloned);
                                                }
                                            }
                                        });
                                    }
                                }
                                _ = tokio::time::sleep(tokio::time::Duration::from_millis(250)) => {}
                            }
                        }
                    }
                    Err(e) => {
                        let mut s = stats.lock().unwrap();
                        s.server_status = format!("Listener Error: {}", e);
                    }
                }
            });

            0
        }
        1 => {
            // Stop
            state.is_running.store(false, Ordering::Relaxed);
            let mut s = state.stats.lock().unwrap();
            s.server_status = "Stopped".to_string();
            0
        }
        2 => {
            // IsWorking
            if state.is_running.load(Ordering::Relaxed) { 1 } else { 0 }
        }
        3 => {
            // DataValid
            1
        }
        4 | 5 => {
            // DataMonitor / RawData
            let guard = state.data.lock().unwrap();
            let len = guard.len().min(out_max_len);
            if len > 0 {
                std::ptr::copy_nonoverlapping(guard.as_ptr(), out_buf, len);
            }
            len
        }
        6 => {
            // Inbox - Outbound Telegram dispatch or external commands
            if payload_len > 0 {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                if let Ok(msg) = serde_json::from_slice::<serde_json::Value>(slice) {
                    let action = msg["action"].as_str().unwrap_or("");
                    let config_cloned = state.config.clone();
                    let stats_cloned = state.stats.clone();
                    let recent_cloned = state.recent_messages.clone();
                    let data_cloned = state.data.clone();

                    if action == "send_message" || action == "notify" || action == "breakout_result" {
                        let target_chat = msg["chat_id"]
                            .as_str()
                            .or_else(|| msg["target"].as_str())
                            .unwrap_or("")
                            .to_string();

                        let text_content = if action == "breakout_result" {
                            let dir = msg["data"]["direction"].as_str().unwrap_or("NONE");
                            let lvl = msg["data"]["broken_level"].as_f64().unwrap_or(0.0);
                            let q = msg["data"]["breakout_quality"].as_f64().unwrap_or(0.0);
                            let c = msg["data"]["certainty_percentage"].as_f64().unwrap_or(0.0);
                            let f = msg["data"]["fake_percentage"].as_f64().unwrap_or(0.0);

                            format!(
                                "<b>🔥 KIRILIM UYARISI 🔥</b>\n\n\
                                <b>Yön:</b> {}\n\
                                <b>Seviye:</b> {:.2}\n\
                                <b>Kalite Skoru:</b> %{:.2}\n\
                                <b>Kesinlik Skoru:</b> %{:.2}\n\
                                <b>Sahte İhtimali:</b> %{:.2}",
                                if dir == "UP" { "🚀 YUKARI" } else if dir == "DOWN" { "💥 AŞAĞI" } else { "Beklemede" },
                                lvl, q, c, f
                            )
                        } else {
                            msg["text"].as_str().or_else(|| msg["content"].as_str()).unwrap_or("Empty message").to_string()
                        };

                        state.runtime.spawn(async move {
                            let (token, default_chat) = {
                                let c = config_cloned.lock().unwrap();
                                (c.bot_token.clone(), c.chat_id.clone())
                            };
                            let dest = if !target_chat.is_empty() { target_chat.clone() } else { default_chat };

                            let res = send_telegram_message(&token, &dest, &text_content).await;

                            let mut s = stats_cloned.lock().unwrap();
                            if res {
                                s.sent_count += 1;
                                s.last_outgoing_msg = text_content.clone();
                                let mut r = recent_cloned.lock().unwrap();
                                r.push(TelegramMessageRecord {
                                    direction: "outbound".to_string(),
                                    sender: "plugin_telegram_bot".to_string(),
                                    recipient: dest,
                                    content: text_content,
                                    timestamp: current_timestamp(),
                                });
                                if r.len() > 50 { r.remove(0); }
                            } else {
                                s.failed_count += 1;
                            }
                            update_monitor_data(&config_cloned, &stats_cloned, &recent_cloned, &data_cloned);
                        });
                    }
                }
            }
            0
        }
        7 => {
            // Outbox Check
            let mut out = state.outbox.lock().unwrap();
            if out.is_empty() {
                0
            } else {
                let msg = out.remove(0);
                if let Ok(json_str) = serde_json::to_string(&msg) {
                    let bytes = json_str.as_bytes();
                    let len = bytes.len().min(out_max_len);
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, len);
                    len
                } else {
                    0
                }
            }
        }
        _ => 0,
    }
}

async fn send_telegram_message(bot_token: &str, chat_id: &str, text: &str) -> bool {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
    let client = reqwest::Client::new();
    let payload = json!({
        "chat_id": chat_id,
        "text": text,
        "parse_mode": "HTML"
    });

    match client.post(&url).json(&payload).send().await {
        Ok(res) => res.status().is_success(),
        Err(_) => false,
    }
}

async fn handle_http_request(
    raw_req: &str,
    config: &Arc<Mutex<TelegramBotConfig>>,
    stats: &Arc<Mutex<TelegramBotStats>>,
    recent: &Arc<Mutex<Vec<TelegramMessageRecord>>>,
    outbox: &Arc<Mutex<Vec<serde_json::Value>>>,
    bot_token: &str,
) -> (String, u16) {
    let body = if let Some(idx) = raw_req.find("\r\n\r\n") {
        &raw_req[idx + 4..]
    } else {
        ""
    };

    if body.is_empty() {
        return (json!({"status": "telegram_bot_active"}).to_string(), 200);
    }

    if let Ok(update) = serde_json::from_str::<serde_json::Value>(body) {
        // Parse Telegram Update object
        let message = &update["message"];
        let text = message["text"].as_str().unwrap_or("");
        let chat_id = message["chat"]["id"].as_i64().map(|id| id.to_string())
            .or_else(|| message["chat"]["username"].as_str().map(|u| format!("@{}", u)))
            .unwrap_or_default();
        let from_user = message["from"]["username"].as_str()
            .or_else(|| message["from"]["first_name"].as_str())
            .unwrap_or("Unknown");

        if !text.is_empty() {
            // Update stats
            {
                let mut s = stats.lock().unwrap();
                s.received_count += 1;
                s.last_incoming_cmd = text.to_string();

                let mut r = recent.lock().unwrap();
                r.push(TelegramMessageRecord {
                    direction: "inbound".to_string(),
                    sender: from_user.to_string(),
                    recipient: chat_id.clone(),
                    content: text.to_string(),
                    timestamp: current_timestamp(),
                });
                if r.len() > 50 { r.remove(0); }
            }

            // Command processing
            let reply_text = match text.trim() {
                "/start" | "/help" => {
                    "<b>🤖 CYCLE-ORC TELEGRAM BOT</b>\n\n\
                    Mevcut Komutlar:\n\
                    • <b>/status</b> - Sistem durumunu gösterir\n\
                    • <b>/metrics</b> - Eklenti metriklerini listeler\n\
                    • <b>/help</b> - Yardım menüsü".to_string()
                }
                "/status" => {
                    "<b>✅ Sistem Durumu:</b> AKTİF\n\
                    <b>Eklenti:</b> plugin_telegram_bot\n\
                    <b>Engine:</b> Cycle-Orc Microstructure Architecture".to_string()
                }
                "/metrics" => {
                    let s = stats.lock().unwrap();
                    format!(
                        "<b>📊 Bot İstatistikleri:</b>\n\
                        • Gönderilen: {}\n\
                        • Alınan: {}\n\
                        • Hatalı: {}",
                        s.sent_count, s.received_count, s.failed_count
                    )
                }
                _ => {
                    if text.starts_with('/') {
                        "⚠️ Bilinmeyen komut. /help yazarak komut listesini alabilirsiniz.".to_string()
                    } else {
                        "📩 Mesajınız alındı.".to_string()
                    }
                }
            };

            // Auto reply via Telegram API if enabled
            let auto_reply = config.lock().unwrap().auto_reply_enabled;
            if auto_reply && !chat_id.is_empty() {
                let token = bot_token.to_string();
                let cid = chat_id.clone();
                let rtext = reply_text.clone();
                tokio::spawn(async move {
                    send_telegram_message(&token, &cid, &rtext).await;
                });
            }

            // Broadcast outbox signal
            let mut out = outbox.lock().unwrap();
            out.push(json!({
                "from": "plugin_telegram_bot",
                "action": "telegram_command",
                "chat_id": chat_id,
                "command": text,
                "sender": from_user
            }));

            return (json!({"status": "ok", "processed": true}).to_string(), 200);
        }
    }

    (json!({"status": "telegram_bot_active"}).to_string(), 200)
}

fn update_monitor_data(
    config: &Arc<Mutex<TelegramBotConfig>>,
    stats: &Arc<Mutex<TelegramBotStats>>,
    recent: &Arc<Mutex<Vec<TelegramMessageRecord>>>,
    data: &Arc<Mutex<Vec<u8>>>,
) {
    let cfg = config.lock().unwrap().clone();
    let st = stats.lock().unwrap().clone();
    let rec = recent.lock().unwrap().clone();

    let json_val = json!({
        "plugin": "plugin_telegram_bot",
        "status": if st.server_status.starts_with("Active") { "active" } else { "inactive" },
        "config": cfg,
        "stats": st,
        "recent_messages": rec
    });

    if let Ok(bytes) = serde_json::to_vec(&json_val) {
        let mut d = data.lock().unwrap();
        *d = bytes;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telegram_config_default() {
        let cfg = TelegramBotConfig::default();
        assert_eq!(cfg.api_mode, "telegram_api");
        assert_eq!(cfg.webhook_port, 8085);
        assert!(cfg.auto_reply_enabled);
    }
}
