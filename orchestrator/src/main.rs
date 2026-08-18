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
    
    let mut lib_path_buf = get_plugin_dir();
    lib_path_buf.push(format!("{}{}.{}", prefix, plugin_name, ext));
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
    if let Some(core_ids) = core_affinity::get_core_ids() {
        if let Some(core) = core_ids.first() {
            core_affinity::set_for_current(*core);
        }
        let pinned_core = core_ids.first().map(|c| c.id).unwrap_or(0);
        eprintln!("[HFT] Ana thread CPU çekirdeğine sabitlendi: Core {}", pinned_core);
    }

    let (log_tx, _log_rx) = tokio::sync::broadcast::channel::<String>(200);

    let orchestrator = Arc::new(Orchestrator::new());
    let mut app = App::new(orchestrator.clone(), log_tx.clone());

    // Spawn High-Speed Zero-Latency Telemetry Web Console on Port 8080
    let web_orc = orchestrator.clone();
    let web_log_tx = log_tx.clone();
    tokio::spawn(async move {
        cycle_finance_breakout_system::web_server::start_web_server(web_orc, web_log_tx, 8080).await;
    });
    
    // --- FLOW ENGINE & CONFIG INITIALIZATION ---
    let config_path = if std::path::Path::new("flow_config.json").exists() {
        "flow_config.json"
    } else if std::path::Path::new("../flow_config.json").exists() {
        "../flow_config.json"
    } else {
        "flow_config.json" // Default fallback
    };
    
    let flow_config = match flow_engine::FlowConfig::load(config_path) {
        Ok(c) => Some(c),
        Err(e) => {
            app.log(&format!("UYARI: flow_config.json okunamadı: {}", e));
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
    
    // Yüklenen tüm pluginleri başlat ve parametrelerini gönder
    let mut startup_buf = [0u8; 8];
    for (id, _, _) in app.orchestrator.list_systems() {
        let mut payload_bytes = Vec::new();
        if let Some(ref config) = flow_config {
            if let Some(plugin_conf) = config.iter().find(|p| p.plugin_name == id) {
                payload_bytes = serde_json::to_vec(&plugin_conf).unwrap_or_default();
            }
        }
        app.orchestrator.call_endpoint(&id, StandardEndpoint::Start, &payload_bytes, &mut startup_buf);
        app.log(&format!("Otomatik başlatıldı: {}", id));
    }
    
    app.log("Sistem başlatıldı ve eklentiler otomatik yüklendi. [HFT Modu: CPU Pinning AÇIK]");
    
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnableMouseCapture)?;
    stdout.execute(Clear(ClearType::All))?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    
    // Pre-allocated HFT buffer (sıcak yolda yeni allokasyonu önler)
    let mut hft_buf = vec![0u8; 1024 * 1024]; // 1MB
    
    let mut last_config_modified = std::fs::metadata(config_path)
        .and_then(|m| m.modified())
        .unwrap_or(std::time::SystemTime::now());
    let mut last_config_check = std::time::Instant::now();
    
    while app.running {
        terminal.draw(|f| draw_ui(f, &mut app))?;
        
        // Hot-reload check for flow_config.json
        if last_config_check.elapsed().as_secs() >= 2 {
            last_config_check = std::time::Instant::now();
            if let Ok(meta) = std::fs::metadata(config_path) {
                if let Ok(modified) = meta.modified() {
                    if modified > last_config_modified {
                        last_config_modified = modified;
                        app.log("Ayarlar degisti! flow_config.json yeniden yukleniyor...");
                        if let Ok(new_config) = flow_engine::FlowConfig::load(config_path) {
                            if let Some(ref eng) = engine_opt {
                                eng.update_config(new_config.clone());
                            }
                            
                            // Send new config to plugins
                            for (id, _, _) in app.orchestrator.list_systems() {
                                if let Some(plugin_conf) = new_config.iter().find(|p| p.plugin_name == id) {
                                    let payload = serde_json::to_vec(&plugin_conf).unwrap_or_default();
                                    app.orchestrator.call_endpoint(&id, StandardEndpoint::Start, &payload, &mut hft_buf);
                                }
                            }
                            app.log("Yeni ayarlar basariyla uygulandi.");
                        } else {
                            app.log("HATA: Yeni flow_config.json okunamadi veya parse edilemedi.");
                        }
                    }
                }
            }
        }
        
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if app.mode == ViewMode::Main {
                    let systems = app.orchestrator.list_systems();
                    
                    match key.code {
                        KeyCode::Char('q') => app.running = false,
                        KeyCode::Down => app.selected = (app.selected + 1) % systems.len().max(1),
                        KeyCode::Up => app.selected = app.selected.saturating_sub(1),
                        
                        KeyCode::Char('s') => {
                            if let Some((id, _, _)) = systems.get(app.selected) {
                                let written = app.orchestrator.call_endpoint(id, StandardEndpoint::Start, &[], &mut hft_buf);
                                if written > 0 {
                                    app.log(&format!("{} başlatıldı", id));
                                } else {
                                    app.log(&format!("{} başlatıldı (yanıt yok)", id));
                                }
                            }
                        }

                        KeyCode::PageDown => {
                            match app.active_panel {
                                ActivePanel::Systems => {
                                    app.selected = (app.selected + 5).min(systems.len().saturating_sub(1));
                                }
                                ActivePanel::Hex => {
                                    app.hex_scroll = app.hex_scroll.saturating_add(5);
                                }
                                ActivePanel::LiveFeed => {
                                    app.live_feed_scroll = app.live_feed_scroll.saturating_add(5);
                                }
                                ActivePanel::Logs => {
                                    app.logs_scroll = app.logs_scroll.saturating_sub(5);
                                }
                                _ => {}
                            }
                        }
                        KeyCode::PageUp => {
                            match app.active_panel {
                                ActivePanel::Systems => {
                                    app.selected = app.selected.saturating_sub(5);
                                }
                                ActivePanel::Hex => {
                                    app.hex_scroll = app.hex_scroll.saturating_sub(5);
                                }
                                ActivePanel::LiveFeed => {
                                    app.live_feed_scroll = app.live_feed_scroll.saturating_sub(5);
                                }
                                ActivePanel::Logs => {
                                    let max_lines = 6;
                                    let max_scroll = app.logs.len().saturating_sub(max_lines) as u16;
                                    app.logs_scroll = (app.logs_scroll + 5).min(max_scroll);
                                }
                                _ => {}
                            }
                        }
                        
                        KeyCode::Char('x') => {
                            if let Some((id, _, _)) = systems.get(app.selected) {
                                app.orchestrator.call_endpoint(id, StandardEndpoint::Stop, &[], &mut hft_buf);
                                app.log(&format!("{} durduruldu", id));
                            }
                        }
                        
                        KeyCode::Char('m') => {
                            if let Some((id, _, _)) = systems.get(app.selected) {
                                match app.orchestrator.monitor_data(id) {
                                    Ok(data) => {
                                        app.monitored_data = Some(data);
                                        app.log(&format!("{} verisi okundu (Canlı Takip Açık)", id));
                                    }
                                    Err(e) => app.log(&format!("Veri okuma hatası ({}): {}", id, e)),
                                }
                            }
                        }
                        
                        KeyCode::Char('d') => {
                            if let Some((id, _, _)) = systems.get(app.selected) {
                                if let Ok(_) = app.orchestrator.unregister_system(id) {
                                    app.log(&format!("{} sistemden silindi", id));
                                    app.selected = app.selected.saturating_sub(1);
                                    app.monitored_data = None;
                                }
                            }
                        }
                        
                        KeyCode::Char('l') => {
                            app.mode = ViewMode::PluginSelection;
                            app.available_plugins = scan_plugins();
                            app.plugin_selected = 0;
                        }
                        
                        KeyCode::Char('e') => {
                            if app.active_tab == 2 {
                                app.log("🚀 Görsel JSON Studio başlatılıyor: http://localhost:3030");
                                let _ = std::process::Command::new("xdg-open")
                                    .arg("http://localhost:3030")
                                    .spawn();
                            }
                        }

                        KeyCode::Char('t') | KeyCode::Char('c') => {
                            if app.active_tab == 2 {
                                // Ayarlar sekmesinde 't' basıldıysa terminal editörünü aç
                                if let Ok(content) = std::fs::read_to_string(config_path) {
                                    let mut textarea = tui_textarea::TextArea::default();
                                    for line in content.lines() {
                                        textarea.insert_newline();
                                        textarea.insert_str(line);
                                    }
                                    // Remove the first empty newline that is created by the above logic
                                    textarea.move_cursor(tui_textarea::CursorMove::Top);
                                    textarea.delete_line_by_end();
                                    app.textarea = Some(textarea);
                                    app.mode = ViewMode::ConfigEditor;
                                } else {
                                    app.log("HATA: flow_config.json okunamadı.");
                                }
                            }
                        }
                        
                        KeyCode::Char('i') => {
                            app.mode = ViewMode::Shell;
                        }
                        
                        _ => {}
                    }
                } else if app.mode == ViewMode::ConfigEditor {
                    // Config Editor mode
                    let mut should_exit = false;
                    let mut should_save = false;
                    
                    match key.code {
                        KeyCode::Esc => {
                            should_exit = true;
                        }
                        KeyCode::Char('s') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                            should_save = true;
                            should_exit = true;
                        }
                        _ => {
                            if let Some(ref mut ta) = app.textarea {
                                ta.input(key);
                            }
                        }
                    }
                    
                    if should_save {
                        if let Some(ref ta) = app.textarea {
                            let lines = ta.lines().join("\n");
                            if std::fs::write(config_path, lines).is_ok() {
                                app.log("flow_config.json başarıyla kaydedildi. Hot-reload tetiklenecek.");
                            } else {
                                app.log("HATA: flow_config.json kaydedilemedi.");
                            }
                        }
                    }
                    
                    if should_exit {
                        app.textarea = None;
                        app.mode = ViewMode::Main;
                    }
                } else if app.mode == ViewMode::PluginSelection {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('l') => {
                            app.mode = ViewMode::Main;
                        }
                        KeyCode::Down => {
                            app.plugin_selected = (app.plugin_selected + 1) % app.available_plugins.len().max(1);
                        }
                        KeyCode::Up => {
                            app.plugin_selected = app.plugin_selected.saturating_sub(1);
                        }
                        KeyCode::Enter => {
                            if let Some(plugin_name) = app.available_plugins.get(app.plugin_selected).cloned() {
                                unsafe { load_plugin_cabi(&mut app, &plugin_name); }
                            }
                            app.mode = ViewMode::Main;
                        }
                        _ => {}
                    }
                } else if app.mode == ViewMode::Shell {
                    let systems = app.orchestrator.list_systems();
                    let sys_id = if let Some((id, _, _)) = systems.get(app.selected) {
                        id.clone()
                    } else {
                        "".to_string()
                    };

                    match key.code {
                        KeyCode::Esc => app.mode = ViewMode::Main,
                        KeyCode::Enter => {
                            let cmd = app.input_shell.trim().to_string();
                            if !cmd.is_empty() {
                                app.shell_history.push(cmd.clone());
                                if app.shell_history.len() > 50 {
                                    app.shell_history.remove(0);
                                }
                                
                                let parts: Vec<&str> = cmd.split_whitespace().collect();
                                let mut hft_buf = vec![0u8; 1024];
                                
                                let action = parts[0].to_lowercase();
                                
                                if action == "help" {
                                    app.log("--- Shell Komutları ---");
                                    app.log("sql <QUERY> (Örn: sql SELECT * FROM mark_prices ORDER BY id DESC LIMIT 5) - Anlık SQL sorgusu çalıştırır");
                                    app.log("tables - SQLite veritabanındaki tabloları ve kayıt sayılarını listeler");
                                    app.log("schema <tablo_adı> - Tablo sütun şemasını gösterir");
                                    app.log("buy <sembol> <miktar> <fiyat|market> [kaldıraç]  (Örn: buy BTCUSDT 0.1 60000 20)");
                                    app.log("sell <sembol> <miktar> <fiyat|market> [kaldıraç] (Örn: sell ETHUSDT 1.5 market 50)");
                                    app.log("close <sembol> (Örn: close BTCUSDT) - Tüm açık pozisyonları kapatır");
                                    app.log("trigger <zaman> <limit> (Örn: trigger 15m 10) - Seçili eklentiyi tetikler");
                                    app.log("start <plugin_id|all> (Örn: start plugin_oi_fetcher) - Eklentiyi başlatır");
                                    app.log("stop <plugin_id|all> (Örn: stop all) - Eklentiyi durdurur");
                                    app.log("fetch oi <sembol> [interval] [limit] - OI verisi çeker");
                                    app.log("quit / exit - Sistemi toptan kapatır");
                                    app.log("-----------------------");
                                } else if action == "quit" || action == "exit" {
                                    app.running = false;
                                } else if action == "start" && parts.len() >= 2 {
                                    let target = parts[1];
                                    if target == "all" {
                                        for (id, _, _) in app.orchestrator.list_systems() {
                                            app.orchestrator.call_endpoint(&id, StandardEndpoint::Start, &[], &mut hft_buf);
                                        }
                                        app.log("Tüm sistemler başlatıldı.");
                                    } else {
                                        let written = app.orchestrator.call_endpoint(target, StandardEndpoint::Start, &[], &mut hft_buf);
                                        app.log(&format!("{} başlatıldı.", target));
                                    }
                                } else if action == "stop" && parts.len() >= 2 {
                                    let target = parts[1];
                                    if target == "all" {
                                        for (id, _, _) in app.orchestrator.list_systems() {
                                            app.orchestrator.call_endpoint(&id, StandardEndpoint::Stop, &[], &mut hft_buf);
                                        }
                                        app.log("Tüm sistemler durduruldu.");
                                    } else {
                                        app.orchestrator.call_endpoint(target, StandardEndpoint::Stop, &[], &mut hft_buf);
                                        app.log(&format!("{} durduruldu.", target));
                                    }
                                } else if action == "fetch" && parts.len() >= 3 && parts[1] == "oi" {
                                    let symbol = parts[2].to_uppercase();
                                    let interval = if parts.len() >= 4 { parts[3] } else { "5m" };
                                    let limit = if parts.len() >= 5 { parts[4].parse::<i64>().unwrap_or(30) } else { 30 };
                                    let req = serde_json::json!({
                                        "action": "fetch_oi",
                                        "symbol": symbol,
                                        "interval": interval,
                                        "limit": limit,
                                        "from": "admin",
                                        "context": {}
                                    });
                                    let bytes = serde_json::to_vec(&req).unwrap_or_default();
                                    app.orchestrator.call_endpoint("plugin_oi_fetcher", StandardEndpoint::Inbox, &bytes, &mut hft_buf);
                                    app.log(&format!("OI fetch isteği gönderildi: {} {} {}", symbol, interval, limit));
                                } else if action == "close" && parts.len() >= 2 {
                                    let symbol = parts[1].to_uppercase();
                                    let req = serde_json::json!({
                                        "action": "close_position",
                                        "user_id": "admin",
                                        "symbol": symbol
                                    });
                                    let bytes = serde_json::to_vec(&req).unwrap_or_default();
                                    app.orchestrator.call_endpoint("plugin_paper_exchange", StandardEndpoint::Inbox, &bytes, &mut hft_buf);
                                    app.log(&format!("Close pozisyon emri gönderildi: {}", symbol));
                                } else if (action == "buy" || action == "sell") && parts.len() >= 4 {
                                    let symbol = parts[1].to_uppercase();
                                    let amount = parts[2].parse::<f64>().unwrap_or(0.0);
                                    let price_str = parts[3].to_lowercase();
                                    
                                    let order_type = if price_str == "market" { "Market" } else { "Limit" };
                                    let price = if price_str == "market" { 0.0 } else { price_str.parse::<f64>().unwrap_or(0.0) };
                                    
                                    let leverage = if parts.len() >= 5 {
                                        parts[4].replace("x", "").parse::<f64>().unwrap_or(20.0)
                                    } else {
                                        20.0
                                    };
                                    
                                    let req = serde_json::json!({
                                        "action": "submit_order",
                                        "user_id": "admin",
                                        "data": {
                                            "id": uuid::Uuid::new_v4().to_string(),
                                            "user_id": "admin",
                                            "symbol": symbol,
                                            "side": if action == "buy" { "Buy" } else { "Sell" },
                                            "position_side": if action == "buy" { "Long" } else { "Short" },
                                            "order_type": order_type,
                                            "price": price,
                                            "stop_price": 0.0,
                                            "amount": amount,
                                            "leverage": leverage,
                                            "executed": 0.0,
                                            "timestamp": 0
                                        }
                                    });
                                    let bytes = serde_json::to_vec(&req).unwrap_or_default();
                                    app.orchestrator.call_endpoint("plugin_paper_exchange", StandardEndpoint::Inbox, &bytes, &mut hft_buf);
                                    app.log(&format!("Paper emri gönderildi: {} {} {} @ {} ({}x)", action, amount, symbol, price_str, leverage));
                                } else if action == "trigger" && parts.len() >= 3 {
                                    if sys_id.is_empty() {
                                        app.log("Lütfen listeden tetiklenecek bir sistem seçin.");
                                    } else {
                                        let interval = parts[1];
                                        let limit = parts[2].parse::<i64>().unwrap_or(5);
                                        
                                        let req = serde_json::json!({
                                            "action": "manual_trigger",
                                            "symbol": "BTCUSDT",
                                            "interval": interval,
                                            "limit": limit
                                        });
                                        let bytes = serde_json::to_vec(&req).unwrap_or_default();
                                        app.orchestrator.call_endpoint(&sys_id, StandardEndpoint::Inbox, &bytes, &mut hft_buf);
                                        app.log(&format!("Manuel tetik gönderildi: {}", sys_id));
                                    }
                                } else if (action == "sql" || action == "query") && parts.len() >= 2 {
                                    let sql_query = cmd[parts[0].len()..].trim();
                                    let req = serde_json::json!({
                                        "action": "query",
                                        "sql": sql_query
                                    });
                                    let bytes = serde_json::to_vec(&req).unwrap_or_default();
                                    let mut out_res_buf = vec![0u8; 8192];
                                    let len = app.orchestrator.call_endpoint("plugin_sqlite_query", StandardEndpoint::Inbox, &bytes, &mut out_res_buf);
                                    if len > 0 {
                                        if let Ok(res_str) = std::str::from_utf8(&out_res_buf[..len]) {
                                            for line in res_str.lines() {
                                                app.log(line);
                                            }
                                        }
                                    }
                                } else if action == "tables" {
                                    let req = serde_json::json!({ "action": "tables" });
                                    let bytes = serde_json::to_vec(&req).unwrap_or_default();
                                    let mut out_res_buf = vec![0u8; 8192];
                                    let len = app.orchestrator.call_endpoint("plugin_sqlite_query", StandardEndpoint::Inbox, &bytes, &mut out_res_buf);
                                    if len > 0 {
                                        if let Ok(res_str) = std::str::from_utf8(&out_res_buf[..len]) {
                                            for line in res_str.lines() {
                                                app.log(line);
                                            }
                                        }
                                    }
                                } else if action == "schema" && parts.len() >= 2 {
                                    let tbl = parts[1];
                                    let req = serde_json::json!({ "action": "schema", "table": tbl });
                                    let bytes = serde_json::to_vec(&req).unwrap_or_default();
                                    let mut out_res_buf = vec![0u8; 8192];
                                    let len = app.orchestrator.call_endpoint("plugin_sqlite_query", StandardEndpoint::Inbox, &bytes, &mut out_res_buf);
                                    if len > 0 {
                                        if let Ok(res_str) = std::str::from_utf8(&out_res_buf[..len]) {
                                            for line in res_str.lines() {
                                                app.log(line);
                                            }
                                        }
                                    }
                                } else {
                                    app.log("Geçersiz komut. Kullanım için 'help' yazabilirsiniz.");
                                }
                            }
                            app.input_shell.clear();
                        }
                        KeyCode::Backspace => {
                            app.input_shell.pop();
                        }
                        KeyCode::Char(c) => {
                            app.input_shell.push(c);
                        }
                        _ => {}
                    }

                }
            } else if let Event::Mouse(mouse_event) = event::read()? {
                let row = mouse_event.row;
                let col = mouse_event.column;
                let size = terminal.size()?;

                let main_layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),  // Tabs
                        Constraint::Length(3),  // Header / System Stats
                        Constraint::Min(10),    // Orta Alan (Tablo + Monitör)
                        Constraint::Length(if app.active_tab == 0 { 8 } else { 0 }),  // Loglar
                        Constraint::Length(3),  // Komutlar (Footer)
                    ])
                    .split(size);

                let middle_layout = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(app.systems_panel_width), 
                        Constraint::Percentage(100 - app.systems_panel_width),
                    ])
                    .split(main_layout[2]);

                let monitor_layout = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(33), 
                        Constraint::Percentage(33), 
                        Constraint::Percentage(34),
                    ])
                    .split(middle_layout[1]);

                let rect_contains = |rect: Rect, x: u16, y: u16| -> bool {
                    x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
                };

                if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
                    if app.mode == ViewMode::Main || app.mode == ViewMode::Shell {
                        // Footer: "Yeni Eklenti Yükle" button
                        if row >= size.height.saturating_sub(3) {
                            if col >= 3 && col <= 3 + 24 {
                                app.mode = ViewMode::PluginSelection;
                                app.available_plugins = scan_plugins();
                                app.plugin_selected = 0;
                            }
                        } else if row < 3 {
                            if col < 15 { app.active_tab = 0; }
                            else if col < 35 { app.active_tab = 1; }
                            else { app.active_tab = 2; }
                        } else if rect_contains(middle_layout[0], col, row) {
                            app.active_panel = ActivePanel::Systems;
                            app.mode = ViewMode::Main;

                            if row >= 8 {
                                let systems = app.orchestrator.list_systems();
                                let index = (row - 8) as usize;
                                if index < systems.len() {
                                    app.selected = index;
                                    let sys_id = &systems[index].0;
                                    
                                    let table_width = middle_layout[0].width;
                                    let col3_start = (table_width as f32 * 0.5) as u16;
                                    
                                    if col >= middle_layout[0].x + col3_start {
                                        let rel_col = col - (middle_layout[0].x + col3_start);
                                        if rel_col < 13 {
                                            app.orchestrator.call_endpoint(sys_id, StandardEndpoint::Start, &[], &mut hft_buf);
                                        } else if rel_col >= 14 && rel_col < 27 {
                                            app.orchestrator.call_endpoint(sys_id, StandardEndpoint::Stop, &[], &mut hft_buf);
                                        } else if rel_col >= 28 && rel_col < 39 {
                                            if let Ok(data) = app.orchestrator.monitor_data(sys_id) {
                                                app.monitored_data = Some(data);
                                            }
                                        } else if rel_col >= 40 && rel_col < 50 {
                                            let _ = app.orchestrator.unregister_system(sys_id);
                                            app.selected = app.selected.saturating_sub(1);
                                            app.monitored_data = None;
                                        }
                                    }
                                }
                            }
                        } else if rect_contains(monitor_layout[0], col, row) {
                            app.active_panel = ActivePanel::Hex;
                            app.mode = ViewMode::Main;
                        } else if rect_contains(monitor_layout[1], col, row) {
                            app.active_panel = ActivePanel::LiveFeed;
                            app.mode = ViewMode::Main;
                        } else if rect_contains(monitor_layout[2], col, row) {
                            app.active_panel = ActivePanel::Shell;
                            app.mode = ViewMode::Shell;
                        } else if app.active_tab == 0 && rect_contains(main_layout[3], col, row) {
                            app.active_panel = ActivePanel::Logs;
                            app.mode = ViewMode::Main;
                        }
                    } else if app.mode == ViewMode::PluginSelection {
                        let popup_w = (size.width as f32 * 0.4) as u16;
                        let popup_h = (size.height as f32 * 0.6) as u16;
                        let popup_x = (size.width.saturating_sub(popup_w)) / 2;
                        let popup_y = (size.height.saturating_sub(popup_h)) / 2;
                        
                        if row >= popup_y + 2 && row < popup_y + popup_h - 1 && col >= popup_x && col < popup_x + popup_w {
                            let idx = (row - (popup_y + 2)) as usize;
                            if idx < app.available_plugins.len() {
                                app.plugin_selected = idx;
                                if let Some(plugin_name) = app.available_plugins.get(app.plugin_selected).cloned() {
                                    unsafe { load_plugin_cabi(&mut app, &plugin_name); }
                                }
                                app.mode = ViewMode::Main;
                            }
                        } else {
                            app.mode = ViewMode::Main;
                        }
                    }
                } else if mouse_event.kind == MouseEventKind::ScrollUp {
                    if rect_contains(monitor_layout[0], col, row) {
                        app.hex_scroll = app.hex_scroll.saturating_sub(2);
                    } else if rect_contains(monitor_layout[1], col, row) {
                        app.live_feed_scroll = app.live_feed_scroll.saturating_sub(2);
                    } else if app.active_tab == 0 && rect_contains(main_layout[3], col, row) {
                        let max_lines = 6;
                        let max_scroll = app.logs.len().saturating_sub(max_lines) as u16;
                        app.logs_scroll = (app.logs_scroll + 2).min(max_scroll);
                    } else if rect_contains(middle_layout[0], col, row) {
                        app.selected = app.selected.saturating_sub(1);
                    }
                } else if mouse_event.kind == MouseEventKind::ScrollDown {
                    if rect_contains(monitor_layout[0], col, row) {
                        app.hex_scroll = app.hex_scroll.saturating_add(2);
                    } else if rect_contains(monitor_layout[1], col, row) {
                        app.live_feed_scroll = app.live_feed_scroll.saturating_add(2);
                    } else if app.active_tab == 0 && rect_contains(main_layout[3], col, row) {
                        app.logs_scroll = app.logs_scroll.saturating_sub(2);
                    } else if rect_contains(middle_layout[0], col, row) {
                        let systems = app.orchestrator.list_systems();
                        app.selected = (app.selected + 1) % systems.len().max(1);
                    }
                }
            }
        } else {
            // Background update of monitored data to ensure real-time UI
            if app.monitored_data.is_some() {
                let systems = app.orchestrator.list_systems();
                if let Some((id, _, _)) = systems.get(app.selected) {
                    if let Ok(data) = app.orchestrator.monitor_data(id) {
                        app.monitored_data = Some(data);
                    }
                }
            }
            
            // Message Bus Routing (Inbox/Outbox) — Zero-copy HFT
            let mut all_messages = Vec::new();
            for (id, _, _) in app.orchestrator.list_systems() {
                let written = app.orchestrator.call_endpoint(&id, StandardEndpoint::Outbox, &[], &mut hft_buf);
                if written > 0 {
                    if let Ok(json_array) = serde_json::from_slice::<serde_json::Value>(&hft_buf[..written]) {
                        if let Some(arr) = json_array.as_array() {
                            for msg in arr {
                                all_messages.push(msg.clone());
                            }
                        }
                    }
                }
            }
            
            for msg in all_messages {
                if let Some(target) = msg.get("to").and_then(|v| v.as_str()) {
                    let msg_bytes = serde_json::to_vec(&msg).unwrap_or_default();
                    app.orchestrator.call_endpoint(target, StandardEndpoint::Inbox, &msg_bytes, &mut hft_buf);
                }
            }
            
            // Background validator & TPS polling
            let has_validator = app.orchestrator.get_system("validator_01").is_some();
            let has_tps = app.orchestrator.get_system("tps_01").is_some();
            
            if has_validator || has_tps {
                let w1 = app.orchestrator.call_endpoint("aggtrade_01", StandardEndpoint::RawData, &[], &mut hft_buf);
                let agg = hft_buf[..w1].to_vec();
                let w2 = app.orchestrator.call_endpoint("depth_01", StandardEndpoint::RawData, &[], &mut hft_buf);
                let depth = hft_buf[..w2].to_vec();
                let w3 = app.orchestrator.call_endpoint("liq_01", StandardEndpoint::RawData, &[], &mut hft_buf);
                let liq = hft_buf[..w3].to_vec();
                
                if !agg.is_empty() {
                    let depth_str = if depth.is_empty() { "{}".into() } else { String::from_utf8_lossy(&depth) };
                    let liq_str = if liq.is_empty() { "{}".into() } else { String::from_utf8_lossy(&liq) };
                    
                    let combined = format!("{{\"agg\":{}, \"depth\":{}, \"liq\":{}}}", 
                        String::from_utf8_lossy(&agg), 
                        depth_str, 
                        liq_str
                    );
                    
                    if has_validator && !depth.is_empty() {
                        app.orchestrator.call_endpoint("validator_01", StandardEndpoint::DataValid, combined.as_bytes(), &mut hft_buf);
                    }
                    if has_tps {
                        app.orchestrator.call_endpoint("tps_01", StandardEndpoint::DataValid, combined.as_bytes(), &mut hft_buf);
                    }
                }
            }
        }
    }
    
    let mut stdout = io::stdout();
    stdout.execute(DisableMouseCapture)?;
    stdout.execute(crossterm::cursor::Show)?;
    disable_raw_mode()?;
    Ok(())
}
