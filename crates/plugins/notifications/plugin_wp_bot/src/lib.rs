use std::ffi::c_void;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WpBotConfig {
    pub api_mode: String,           // "webhook", "bridge_http", "meta_api", "twilio"
    pub target_phone: String,       // Default target recipient phone number
    pub admin_numbers: Vec<String>, // List of authorized admin numbers
    pub webhook_port: u16,          // Port for receiving incoming WhatsApp webhooks
    pub gateway_url: String,        // Outbound HTTP Gateway endpoint (e.g. Baileys / Node bridge)
    pub api_token: String,          // API key or Bearer token
    pub webhook_verify_token: String, // Webhook verification token (Meta API)
    pub auto_reply_enabled: bool,   // Automatically reply to commands via WhatsApp
}

impl Default for WpBotConfig {
    fn default() -> Self {
        Self {
            api_mode: "bridge_http".to_string(),
            target_phone: "+905000000000".to_string(),
            admin_numbers: vec!["+905000000000".to_string()],
            webhook_port: 8085,
            gateway_url: "http://127.0.0.1:3000/api/send-message".to_string(),
            api_token: "SECRET_BOT_TOKEN_123".to_string(),
            webhook_verify_token: "CYCLE_ORC_WP_VERIFY".to_string(),
            auto_reply_enabled: true,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WpMessageRecord {
    pub direction: String, // "inbound" or "outbound"
    pub sender: String,
    pub recipient: String,
    pub content: String,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WpBotStats {
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
    config: Arc<Mutex<WpBotConfig>>,
    stats: Arc<Mutex<WpBotStats>>,
    recent_messages: Arc<Mutex<Vec<WpMessageRecord>>>,
    outbox: Arc<Mutex<Vec<serde_json::Value>>>,
    data: Arc<Mutex<Vec<u8>>>,
}

fn get_config_path() -> String {
    let paths = [
        "config/wp_bot.cfg",
        "../config/wp_bot.cfg",
        "../../config/wp_bot.cfg",
        "wp_bot.cfg",
    ];
    for p in &paths {
        if Path::new(p).exists() {
            return p.to_string();
        }
    }
    "config/wp_bot.cfg".to_string()
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn init_plugin(
    state_out: *mut *mut c_void,
) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("Failed to initialize Tokio runtime for plugin_wp_bot");

    let cfg_path = get_config_path();
    let config = if let Ok(content) = fs::read_to_string(&cfg_path) {
        serde_json::from_str::<WpBotConfig>(&content).unwrap_or_default()
    } else {
        let _ = fs::create_dir_all("config");
        let default_cfg = WpBotConfig::default();
        let _ = fs::write(
            &cfg_path,
            serde_json::to_string_pretty(&default_cfg).unwrap_or_default(),
        );
        default_cfg
    };

    let initial_stats = WpBotStats {
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
        "plugin": "plugin_wp_bot",
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
    if plugin_state.is_null() {
        return 0;
    }
    let state = &*(plugin_state as *const PluginState);

    match endpoint_id {
        0 => {
            // Start
            if !state.is_running.load(Ordering::Relaxed) {
                state.is_running.store(true, Ordering::Relaxed);
                let is_running = state.is_running.clone();
                let config = state.config.clone();
                let stats = state.stats.clone();
                let recent = state.recent_messages.clone();
                let outbox = state.outbox.clone();
                let data = state.data.clone();

                {
                    let mut st = stats.lock().unwrap();
                    st.server_status = "Listening".to_string();
                }

                // Spawn Tokio Webhook Server
                state.runtime.spawn(async move {
                    let port = { config.lock().unwrap().webhook_port };
                    let addr = format!("0.0.0.0:{}", port);
                    if let Ok(listener) = tokio::net::TcpListener::bind(&addr).await {
                        while is_running.load(Ordering::Relaxed) {
                            tokio::select! {
                                res = listener.accept() => {
                                    if let Ok((mut stream, _)) = res {
                                        let config_cloned = config.clone();
                                        let stats_cloned = stats.clone();
                                        let recent_cloned = recent.clone();
                                        let outbox_cloned = outbox.clone();
                                        let data_cloned = data.clone();

                                        tokio::spawn(async move {
                                            let mut buf = [0u8; 4096];
                                            use tokio::io::{AsyncReadExt, AsyncWriteExt};
                                            if let Ok(n) = stream.read(&mut buf).await {
                                                if n > 0 {
                                                    let req_str = String::from_utf8_lossy(&buf[..n]);
                                                    let (resp, incoming_msg) = process_http_request(
                                                        &req_str,
                                                        &config_cloned,
                                                        &stats_cloned,
                                                        &recent_cloned,
                                                        &outbox_cloned,
                                                    ).await;

                                                    let _ = stream.write_all(resp.as_bytes()).await;

                                                    if let Some(msg_text) = incoming_msg {
                                                        update_monitor_data(&data_cloned, &config_cloned, &stats_cloned, &recent_cloned);
                                                        let _ = msg_text;
                                                    }
                                                }
                                            }
                                        });
                                    }
                                }
                                _ = tokio::time::sleep(tokio::time::Duration::from_millis(250)) => {}
                            }
                        }
                    } else {
                        let mut st = stats.lock().unwrap();
                        st.server_status = format!("Port {} busy", port);
                    }
                });
            }
            0
        }
        1 => {
            // Stop
            state.is_running.store(false, Ordering::Relaxed);
            let mut st = state.stats.lock().unwrap();
            st.server_status = "Stopped".to_string();
            0
        }
        2 => {
            // IsWorking
            if state.is_running.load(Ordering::Relaxed) {
                1
            } else {
                0
            }
        }
        3 => {
            // DataValid
            1
        }
        4 | 5 => {
            // DataMonitor (4) or RawData (5)
            update_monitor_data(&state.data, &state.config, &state.stats, &state.recent_messages);
            let guard = state.data.lock().unwrap();
            let len = guard.len().min(out_max_len);
            if !out_buf.is_null() && len > 0 {
                unsafe {
                    std::ptr::copy_nonoverlapping(guard.as_ptr(), out_buf, len);
                }
            }
            len
        }
        6 => {
            // Inbox: Incoming commands from orchestrator / plugins to WP Bot
            if payload_len > 0 {
                let slice = unsafe { std::slice::from_raw_parts(payload, payload_len) };
                if let Ok(msg) = serde_json::from_slice::<serde_json::Value>(slice) {
                    let action = msg["action"].as_str().unwrap_or("");
                    let recipient = msg["recipient"]
                        .as_str()
                        .or_else(|| msg["to"].as_str())
                        .unwrap_or("");
                    let message = msg["message"]
                        .as_str()
                        .or_else(|| msg["content"].as_str())
                        .unwrap_or("");

                    match action {
                        "send_whatsapp" | "send_alert" | "notify" => {
                            let cfg = state.config.lock().unwrap().clone();
                            let target = if recipient.is_empty() {
                                cfg.target_phone.clone()
                            } else {
                                recipient.to_string()
                            };

                            let stats = state.stats.clone();
                            let recent = state.recent_messages.clone();
                            let data = state.data.clone();
                            let text = message.to_string();

                            state.runtime.spawn(async move {
                                let success = dispatch_whatsapp_message(&cfg, &target, &text).await;

                                {
                                    let mut st = stats.lock().unwrap();
                                    if success {
                                        st.sent_count += 1;
                                        st.last_outgoing_msg = text.clone();
                                    } else {
                                        st.failed_count += 1;
                                    }
                                }

                                {
                                    let mut recs = recent.lock().unwrap();
                                    recs.push(WpMessageRecord {
                                        direction: "outbound".to_string(),
                                        sender: "plugin_wp_bot".to_string(),
                                        recipient: target,
                                        content: text,
                                        timestamp: current_timestamp(),
                                    });
                                    if recs.len() > 50 {
                                        recs.remove(0);
                                    }
                                }

                                update_monitor_data(&data, &state.config, &stats, &recent);
                            });
                        }
                        "broadcast" => {
                            let cfg = state.config.lock().unwrap().clone();
                            let stats = state.stats.clone();
                            let recent = state.recent_messages.clone();
                            let data = state.data.clone();
                            let text = message.to_string();

                            state.runtime.spawn(async move {
                                for target in &cfg.admin_numbers {
                                    let success = dispatch_whatsapp_message(&cfg, target, &text).await;
                                    let mut st = stats.lock().unwrap();
                                    if success {
                                        st.sent_count += 1;
                                    } else {
                                        st.failed_count += 1;
                                    }
                                }

                                update_monitor_data(&data, &state.config, &stats, &recent);
                            });
                        }
                        "update_config" => {
                            if let Ok(new_cfg) = serde_json::from_value::<WpBotConfig>(msg["config"].clone()) {
                                *state.config.lock().unwrap() = new_cfg.clone();
                                let cfg_path = get_config_path();
                                let _ = fs::write(
                                    cfg_path,
                                    serde_json::to_string_pretty(&new_cfg).unwrap_or_default(),
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }
            0
        }
        7 => {
            // Outbox: Drain incoming WhatsApp commands for downstream plugins
            let mut q = state.outbox.lock().unwrap();
            if !q.is_empty() {
                let json_array = serde_json::Value::Array(q.clone());
                q.clear();
                let bytes = serde_json::to_vec(&json_array).unwrap_or_default();
                let len = bytes.len().min(out_max_len);
                if !out_buf.is_null() && len > 0 {
                    unsafe {
                        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, len);
                    }
                }
                len
            } else {
                0
            }
        }
        _ => 0,
    }
}

async fn process_http_request(
    raw_req: &str,
    config_arc: &Arc<Mutex<WpBotConfig>>,
    stats_arc: &Arc<Mutex<WpBotStats>>,
    recent_arc: &Arc<Mutex<Vec<WpMessageRecord>>>,
    outbox_arc: &Arc<Mutex<Vec<serde_json::Value>>>,
) -> (String, Option<String>) {
    let lines: Vec<&str> = raw_req.lines().collect();
    if lines.is_empty() {
        return (
            "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n".to_string(),
            None,
        );
    }

    let first_line = lines[0];

    // Meta Webhook Verification (GET)
    if first_line.starts_with("GET") {
        let verify_token = { config_arc.lock().unwrap().webhook_verify_token.clone() };
        if first_line.contains("hub.mode=subscribe") && first_line.contains(&verify_token) {
            if let Some(challenge_idx) = first_line.find("hub.challenge=") {
                let challenge = &first_line[challenge_idx + 14..]
                    .split('&')
                    .next()
                    .unwrap_or("")
                    .split(' ')
                    .next()
                    .unwrap_or("");

                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                    challenge.len(),
                    challenge
                );
                return (resp, None);
            }
        }
        return (
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 26\r\n\r\n{\"status\":\"wp_bot_active\"}".to_string(),
            None,
        );
    }

    // Parse JSON Body for POST
    if first_line.starts_with("POST") {
        if let Some(body_idx) = raw_req.find("\r\n\r\n") {
            let body = &raw_req[body_idx + 4..];
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(body) {
                // Extract sender and message text from various JSON structures (Bridge, Meta, Twilio)
                let (sender, message_text) = extract_incoming_message(&val);

                if !message_text.is_empty() {
                    let timestamp = current_timestamp();

                    // Update Stats & History
                    {
                        let mut st = stats_arc.lock().unwrap();
                        st.received_count += 1;
                        st.last_incoming_cmd = message_text.clone();
                    }

                    {
                        let mut recs = recent_arc.lock().unwrap();
                        recs.push(WpMessageRecord {
                            direction: "inbound".to_string(),
                            sender: sender.clone(),
                            recipient: "plugin_wp_bot".to_string(),
                            content: message_text.clone(),
                            timestamp,
                        });
                        if recs.len() > 50 {
                            recs.remove(0);
                        }
                    }

                    // Process Command
                    let (cmd_event, reply_opt) = parse_and_execute_command(&sender, &message_text);

                    if let Some(evt) = cmd_event {
                        let mut q = outbox_arc.lock().unwrap();
                        q.push(evt);
                    }

                    // Auto Reply if enabled
                    let (auto_reply_enabled, cfg) = {
                        let c = config_arc.lock().unwrap();
                        (c.auto_reply_enabled, c.clone())
                    };

                    if auto_reply_enabled {
                        if let Some(reply_text) = reply_opt {
                            let target = if sender.is_empty() {
                                cfg.target_phone.clone()
                            } else {
                                sender.clone()
                            };

                            let stats_c = stats_arc.clone();
                            let recent_c = recent_arc.clone();

                            tokio::spawn(async move {
                                let success = dispatch_whatsapp_message(&cfg, &target, &reply_text).await;
                                {
                                    let mut st = stats_c.lock().unwrap();
                                    if success {
                                        st.sent_count += 1;
                                        st.last_outgoing_msg = reply_text.clone();
                                    } else {
                                        st.failed_count += 1;
                                    }
                                }

                                {
                                    let mut recs = recent_c.lock().unwrap();
                                    recs.push(WpMessageRecord {
                                        direction: "outbound".to_string(),
                                        sender: "plugin_wp_bot".to_string(),
                                        recipient: target,
                                        content: reply_text,
                                        timestamp: current_timestamp(),
                                    });
                                    if recs.len() > 50 {
                                        recs.remove(0);
                                    }
                                }
                            });
                        }
                    }

                    let resp_body = "{\"status\":\"success\",\"received\":true}";
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                        resp_body.len(),
                        resp_body
                    );
                    return (resp, Some(message_text));
                }
            }
        }
    }

    let default_body = "{\"status\":\"ok\"}";
    (
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            default_body.len(),
            default_body
        ),
        None,
    )
}

fn extract_incoming_message(val: &serde_json::Value) -> (String, String) {
    // 1. Generic Bridge format: {"sender": "+905...", "message": "/status"} or {"from": "+905...", "text": "..."}
    if let Some(msg) = val["message"].as_str() {
        let sender = val["sender"]
            .as_str()
            .or_else(|| val["from"].as_str())
            .unwrap_or("unknown")
            .to_string();
        return (sender, msg.to_string());
    }
    if let Some(text) = val["text"].as_str() {
        let sender = val["from"]
            .as_str()
            .or_else(|| val["sender"].as_str())
            .unwrap_or("unknown")
            .to_string();
        return (sender, text.to_string());
    }

    // 2. Meta WhatsApp Cloud API format: entry[0].changes[0].value.messages[0].text.body
    if let Some(messages) = val["entry"][0]["changes"][0]["value"]["messages"].as_array() {
        if let Some(first_msg) = messages.get(0) {
            let sender = first_msg["from"].as_str().unwrap_or("unknown").to_string();
            let body = first_msg["text"]["body"].as_str().unwrap_or("").to_string();
            return (sender, body);
        }
    }

    // 3. Twilio format: {"From": "whatsapp:+905...", "Body": "..."}
    if let Some(body) = val["Body"].as_str() {
        let from = val["From"].as_str().unwrap_or("unknown").to_string();
        return (from, body.to_string());
    }

    ("unknown".to_string(), "".to_string())
}

pub fn parse_and_execute_command(
    sender: &str,
    text: &str,
) -> (Option<serde_json::Value>, Option<String>) {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') {
        return (
            None,
            Some("🤖 *Cycle Orchestrator Bot*\nKomut listesini görmek için `/help` yazabilirsiniz.".to_string()),
        );
    }

    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    let cmd = parts[0].to_lowercase();

    match cmd.as_str() {
        "/help" => {
            let menu = "🤖 *Cycle Orchestrator WhatsApp Bot*\n\n\
                *Kullanılabilir Komutlar:*\n\
                🔹 `/status` - Sistem & eklenti durumunu gösterir\n\
                🔹 `/balance` - Borsa bakiyesini sorgular\n\
                🔹 `/metrics` - Sistem kaynak kullanımını (CPU/RAM) gösterir\n\
                🔹 `/buy <sembol> <miktar>` - Alış emri gönderir (ör: `/buy BTCUSDT 0.01`)\n\
                🔹 `/sell <sembol> <miktar>` - Satış emri gönderir (ör: `/sell BTCUSDT 0.01`)\n\
                🔹 `/ping` - Bot bağlantı testini doğrular"
                .to_string();
            (None, Some(menu))
        }
        "/ping" => (None, Some("🏓 *PONG!* Cycle Orchestrator WhatsApp Bot aktif ve çalışıyor.".to_string())),
        "/status" => {
            let event = json!({
                "from": "plugin_wp_bot",
                "to": "orchestrator",
                "action": "get_system_status",
                "requested_by": sender
            });
            let reply = format!("⏳ *Sistem Durumu:* `/status` sorgusu işleniyor... (Gönderen: {})", sender);
            (Some(event), Some(reply))
        }
        "/balance" => {
            let event = json!({
                "from": "plugin_wp_bot",
                "to": "plugin_binance_trader",
                "action": "get_balance",
                "requested_by": sender
            });
            let reply = "💳 *Bakiye Sorgusu:* Binance Futures bakiye sorgusu iletildi...".to_string();
            (Some(event), Some(reply))
        }
        "/metrics" => {
            let event = json!({
                "from": "plugin_wp_bot",
                "to": "plugin_sys_metrics",
                "action": "get_metrics",
                "requested_by": sender
            });
            let reply = "📊 *Sistem Metrikleri:* CPU ve RAM telemetrileri alınıyor...".to_string();
            (Some(event), Some(reply))
        }
        "/buy" => {
            if parts.len() < 3 {
                return (
                    None,
                    Some("⚠️ *Hatalı Kullanım:* `/buy <SEMBOL> <MİKTAR>` şeklinde giriniz.\nÖrnek: `/buy BTCUSDT 0.001`".to_string()),
                );
            }
            let symbol = parts[1].to_uppercase();
            let qty: f64 = parts[2].parse().unwrap_or(0.0);
            if qty <= 0.0 {
                return (None, Some("⚠️ *Hata:* Geçersiz miktar girildi.".to_string()));
            }

            let event = json!({
                "from": "plugin_wp_bot",
                "to": "plugin_binance_trader",
                "action": "place_order",
                "symbol": symbol,
                "side": "BUY",
                "positionSide": "LONG",
                "type": "MARKET",
                "quantity": qty,
                "requested_by": sender
            });
            let reply = format!("🟢 *Alış Emri İletildi:*\nSembol: `{}`\nMiktar: `{}`\nYön: `LONG / BUY`", symbol, qty);
            (Some(event), Some(reply))
        }
        "/sell" => {
            if parts.len() < 3 {
                return (
                    None,
                    Some("⚠️ *Hatalı Kullanım:* `/sell <SEMBOL> <MİKTAR>` şeklinde giriniz.\nÖrnek: `/sell BTCUSDT 0.001`".to_string()),
                );
            }
            let symbol = parts[1].to_uppercase();
            let qty: f64 = parts[2].parse().unwrap_or(0.0);
            if qty <= 0.0 {
                return (None, Some("⚠️ *Hata:* Geçersiz miktar girildi.".to_string()));
            }

            let event = json!({
                "from": "plugin_wp_bot",
                "to": "plugin_binance_trader",
                "action": "place_order",
                "symbol": symbol,
                "side": "SELL",
                "positionSide": "SHORT",
                "type": "MARKET",
                "quantity": qty,
                "requested_by": sender
            });
            let reply = format!("🔴 *Satış Emri İletildi:*\nSembol: `{}`\nMiktar: `{}`\nYön: `SHORT / SELL`", symbol, qty);
            (Some(event), Some(reply))
        }
        _ => (
            None,
            Some(format!("❓ *Bilinmeyen Komut:* `{}`\nKomut listesi için `/help` yazabilirsiniz.", cmd)),
        ),
    }
}

async fn dispatch_whatsapp_message(cfg: &WpBotConfig, recipient: &str, text: &str) -> bool {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build();

    let client = match client {
        Ok(c) => c,
        Err(_) => return false,
    };

    match cfg.api_mode.as_str() {
        "bridge_http" | "webhook" => {
            let payload = json!({
                "recipient": recipient,
                "message": text,
                "token": cfg.api_token
            });
            if let Ok(res) = client
                .post(&cfg.gateway_url)
                .json(&payload)
                .send()
                .await
            {
                return res.status().is_success();
            }
        }
        "meta_api" => {
            let url = format!(
                "https://graph.facebook.com/v18.0/me/messages?access_token={}",
                cfg.api_token
            );
            let payload = json!({
                "messaging_product": "whatsapp",
                "to": recipient,
                "type": "text",
                "text": { "body": text }
            });
            if let Ok(res) = client.post(&url).json(&payload).send().await {
                return res.status().is_success();
            }
        }
        "twilio" => {
            let payload = json!({
                "From": "whatsapp:+14155238886",
                "To": format!("whatsapp:{}", recipient),
                "Body": text
            });
            if let Ok(res) = client
                .post(&cfg.gateway_url)
                .json(&payload)
                .send()
                .await
            {
                return res.status().is_success();
            }
        }
        _ => {}
    }
    false
}

fn update_monitor_data(
    data_arc: &Arc<Mutex<Vec<u8>>>,
    config_arc: &Arc<Mutex<WpBotConfig>>,
    stats_arc: &Arc<Mutex<WpBotStats>>,
    recent_arc: &Arc<Mutex<Vec<WpMessageRecord>>>,
) {
    let cfg = config_arc.lock().unwrap().clone();
    let stats = stats_arc.lock().unwrap().clone();
    let recents = recent_arc.lock().unwrap().clone();

    let json_val = json!({
        "plugin": "plugin_wp_bot",
        "status": if stats.server_status == "Listening" { "running" } else { "idle" },
        "config": cfg,
        "stats": stats,
        "recent_messages": recents
    });

    let json_bytes = serde_json::to_vec_pretty(&json_val).unwrap_or_default();
    let mut lock = data_arc.lock().unwrap();
    *lock = json_bytes;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = WpBotConfig::default();
        assert_eq!(cfg.webhook_port, 8085);
        assert_eq!(cfg.api_mode, "bridge_http");
        assert!(cfg.auto_reply_enabled);
    }

    #[test]
    fn test_command_parser_help() {
        let (event, reply) = parse_and_execute_command("+905000000000", "/help");
        assert!(event.is_none());
        assert!(reply.unwrap().contains("Kullanılabilir Komutlar"));
    }

    #[test]
    fn test_command_parser_ping() {
        let (event, reply) = parse_and_execute_command("+905000000000", "/ping");
        assert!(event.is_none());
        assert!(reply.unwrap().contains("PONG"));
    }

    #[test]
    fn test_command_parser_buy() {
        let (event, reply) = parse_and_execute_command("+905000000000", "/buy BTCUSDT 0.05");
        assert!(event.is_some());
        let evt = event.unwrap();
        assert_eq!(evt["action"], "place_order");
        assert_eq!(evt["symbol"], "BTCUSDT");
        assert_eq!(evt["quantity"], 0.05);
        assert!(reply.unwrap().contains("Alış Emri İletildi"));
    }

    #[test]
    fn test_extract_incoming_message_bridge() {
        let sample_json = json!({
            "sender": "+905551234567",
            "message": "/status"
        });
        let (sender, msg) = extract_incoming_message(&sample_json);
        assert_eq!(sender, "+905551234567");
        assert_eq!(msg, "/status");
    }
}
