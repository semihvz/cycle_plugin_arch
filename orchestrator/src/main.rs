mod tui;

use orchestrator::orchestrator::Orchestrator;
use orchestrator::endpoint::StandardEndpoint;
use orchestrator::system::{SystemInstance, RawEndpointFn};
use crossterm::{
    event::{self, Event, KeyCode, MouseEventKind, MouseButton, EnableMouseCapture, DisableMouseCapture},
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
    ExecutableCommand,
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::sync::Arc;
use std::ffi::c_void;

#[derive(PartialEq)]
pub enum ViewMode {
    Main,
    PluginSelection,
    ConfirmDelete(String),
    ContextMenu(String, u16, u16),
}

pub struct App {
    pub orchestrator: Arc<Orchestrator>,
    pub selected: usize,
    pub logs: Vec<String>,
    pub monitored_data: Option<Vec<u8>>,
    pub running: bool,
    pub mode: ViewMode,
    pub available_plugins: Vec<String>,
    pub plugin_selected: usize,
    pub active_tab: usize,
    pub systems_panel_width: u16,
    pub is_dragging_split: bool,
    pub monitor_scroll: u16,
    pub sys: sysinfo::System,
}

impl App {
    pub fn new(orchestrator: Arc<Orchestrator>) -> Self {
        let mut sys = sysinfo::System::new_all();
        sys.refresh_all();
        Self {
            orchestrator,
            selected: 0,
            logs: Vec::new(),
            monitored_data: None,
            running: true,
            mode: ViewMode::Main,
            available_plugins: Vec::new(),
            plugin_selected: 0,
            active_tab: 0,
            systems_panel_width: 30,
            is_dragging_split: false,
            monitor_scroll: 0,
            sys,
        }
    }

    pub fn log(&mut self, msg: &str) {
        let now = chrono::Local::now();
        self.logs.push(format!("[{}] {}", now.format("%H:%M:%S"), msg));
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
    }
}

/// Eklenti yükleme yardımcı fonksiyonu (C-ABI: init_plugin)
unsafe fn load_plugin_cabi(app: &mut App, plugin_name: &str) {
    let ext = if cfg!(target_os = "windows") { "dll" } 
              else if cfg!(target_os = "macos") { "dylib" } 
              else { "so" };
    let prefix = if cfg!(target_os = "windows") { "" } else { "lib" };
    let mut lib_path_buf = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("."));
    lib_path_buf.pop();
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

/// Eklenti tarama yardımcı fonksiyonu
fn scan_plugins() -> Vec<String> {
    let mut plugins = Vec::new();
    let mut lib_dir = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("."));
    lib_dir.pop();
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

    let orchestrator = Arc::new(Orchestrator::new());
    let mut app = App::new(orchestrator.clone());
    
    app.log("Sistem başlatıldı. Orkestratör devrede. [HFT Modu: CPU Pinning AÇIK]");
    app.log("Lütfen 'l' tuşuna basarak eklentileri (plugin) yükleyin.");
    
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnableMouseCapture)?;
    stdout.execute(Clear(ClearType::All))?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    
    // Pre-allocated HFT buffer (sıcak yolda yeni allokasyonu önler)
    let mut hft_buf = vec![0u8; 1024 * 1024]; // 1MB
    
    while app.running {
        terminal.draw(|f| tui::draw_ui(f, &mut app))?;
        
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
                        
                        _ => {}
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
                }
            } else if let Event::Mouse(mouse_event) = event::read()? {
                if mouse_event.kind == MouseEventKind::Down(MouseButton::Left) {
                    let row = mouse_event.row;
                    let col = mouse_event.column;
                    
                    if app.mode == ViewMode::Main {
                        // Footer: "Yeni Eklenti Yükle" button
                        let size = terminal.size()?;
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
                        } else if app.active_tab == 0 && row >= 8 && row < size.height.saturating_sub(11) {
                            let systems = app.orchestrator.list_systems();
                            let index = (row - 8) as usize;
                            if index < systems.len() {
                                app.selected = index;
                                let sys_id = &systems[index].0;
                                
                                let table_width = (size.width as f32 * (app.systems_panel_width as f32 / 100.0)) as u16;
                                let col3_start = (table_width as f32 * 0.5) as u16;
                                
                                if col >= col3_start {
                                    let rel_col = col - col3_start;
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
                    } else if app.mode == ViewMode::PluginSelection {
                        let size = terminal.size()?;
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
    disable_raw_mode()?;
    Ok(())
}
