mod tui;

use orchestrator::orchestrator::Orchestrator;
use orchestrator::endpoint::StandardEndpoint;
use orchestrator::system::{System, SystemContext, EndpointHandler};
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
    ExecutableCommand,
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::collections::HashMap;
use std::sync::Arc;

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
            available_plugins: vec![
                "plugin_binance".to_string(),
                "plugin_depth".to_string(),
                "plugin_liquidation".to_string(),
                "plugin_aggtrade".to_string(),
                "plugin_storage".to_string(),
                "plugin_timescaledb".to_string(),
                "plugin_validator".to_string(),
                "plugin_tps".to_string(),
                "plugin_ohlcv_fetcher".to_string(),
                "plugin_ohlcv_requester".to_string(),
                "plugin_alarm".to_string(),
                "plugin_msmp".to_string(),
                "plugin_msmp_requester".to_string()
            ],
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



#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let orchestrator = Arc::new(Orchestrator::new());
    let mut app = App::new(orchestrator.clone());
    
    app.log("Sistem başlatıldı. Orkestratör devrede.");
    
    app.log("Lütfen 'l' tuşuna basarak eklentileri (plugin) yükleyin.");
    
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(Clear(ClearType::All))?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    
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
                                match app.orchestrator.call_endpoint(id, StandardEndpoint::Start) {
                                    Ok(_) => app.log(&format!("{} başlatıldı", id)),
                                    Err(e) => app.log(&format!("Hata ({}): {}", id, e)),
                                }
                            }
                        }
                        
                        KeyCode::Char('x') => {
                            if let Some((id, _, _)) = systems.get(app.selected) {
                                match app.orchestrator.call_endpoint(id, StandardEndpoint::Stop) {
                                    Ok(_) => app.log(&format!("{} durduruldu", id)),
                                    Err(e) => app.log(&format!("Hata ({}): {}", id, e)),
                                }
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
                                    app.monitored_data = None; // Reset if deleted
                                }
                            }
                        }
                        
                        KeyCode::Char('l') => {
                            app.mode = ViewMode::PluginSelection;
                            app.available_plugins = vec![
                                "plugin_example".to_string(),
                                "plugin_ai".to_string(),
                                "plugin_network".to_string(),
                                "plugin_storage".to_string(),
                                "plugin_crypto".to_string(),
                                "plugin_ui_bridge".to_string(),
                                "plugin_binance".to_string(),
                                "plugin_aggtrade".to_string(),
                                "plugin_depth".to_string(),
                                "plugin_liquidation".to_string(),
                                "plugin_validator".to_string(),
                                "plugin_tps".to_string(),
                                "plugin_ohlcv_fetcher".to_string(),
                                "plugin_ohlcv_requester".to_string()
                            ];
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
                            if let Some(plugin_name) = app.available_plugins.get(app.plugin_selected) {
                                unsafe {
                                    let ext = if cfg!(target_os = "windows") { "dll" } 
                                              else if cfg!(target_os = "macos") { "dylib" } 
                                              else { "so" };
                                    let prefix = if cfg!(target_os = "windows") { "" } else { "lib" };
                                    let lib_path = format!("../{}/target/debug/{}{}.{}", plugin_name, prefix, plugin_name, ext);
                                    
                                    match libloading::Library::new(&lib_path) {
                                        Ok(lib) => {
                                            let func: Result<libloading::Symbol<unsafe extern "C" fn() -> *mut Box<dyn System>>, _> = lib.get(b"create_plugin");
                                            match func {
                                                Ok(create_plugin) => {
                                                    let ptr = create_plugin();
                                                    let sys = *Box::from_raw(ptr);
                                                    app.orchestrator.register_system(sys);
                                                    Box::leak(Box::new(lib));
                                                    app.log(&format!("{} eklentisi basariyla yuklendi.", plugin_name));
                                                }
                                                Err(_) => app.log(&format!("{} eklentisinde create_plugin fonksiyonu bulunamadi.", plugin_name)),
                                            }
                                        }
                                        Err(e) => app.log(&format!("{} eklentisi yuklenemedi (derlediginizden emin olun): {}", plugin_name, e)),
                                    }
                                }
                            }
                            app.mode = ViewMode::Main;
                        }
                        _ => {}
                    }
                }
            }
        } else {
            // Background update of monitored data to ensure real-time UI
            if app.monitored_data.is_some() {
                let systems = app.orchestrator.list_systems();
                if let Some((id, _, _)) = systems.get(app.selected) {
                    if let Ok(data) = app.orchestrator.call_endpoint(id, StandardEndpoint::DataMonitor) {
                        app.monitored_data = Some(data);
                    }
                }
            }
            
            // Message Bus Routing (Inbox/Outbox)
            let mut all_messages = Vec::new();
            for (id, _, _) in app.orchestrator.list_systems() {
                if let Ok(outbox_data) = app.orchestrator.call_endpoint(&id, StandardEndpoint::Outbox) {
                    if !outbox_data.is_empty() {
                        if let Ok(json_array) = serde_json::from_slice::<serde_json::Value>(&outbox_data) {
                            if let Some(arr) = json_array.as_array() {
                                for msg in arr {
                                    all_messages.push(msg.clone());
                                }
                            }
                        }
                    }
                }
            }
            
            for msg in all_messages {
                if let Some(target) = msg.get("to").and_then(|v| v.as_str()) {
                    let msg_bytes = serde_json::to_vec(&msg).unwrap_or_default();
                    let _ = app.orchestrator.call_endpoint_with_data(target, StandardEndpoint::Inbox, Some(msg_bytes));
                }
            }
            
            // Background validator & TPS polling
            let has_validator = app.orchestrator.get_system("validator_01").is_some();
            let has_tps = app.orchestrator.get_system("tps_01").is_some();
            
            if has_validator || has_tps {
                let agg = app.orchestrator.call_endpoint("aggtrade_01", StandardEndpoint::RawData).unwrap_or_default();
                let depth = app.orchestrator.call_endpoint("depth_01", StandardEndpoint::RawData).unwrap_or_default();
                let liq = app.orchestrator.call_endpoint("liq_01", StandardEndpoint::RawData).unwrap_or_default();
                
                if !agg.is_empty() {
                    let depth_str = if depth.is_empty() { "{}".into() } else { String::from_utf8_lossy(&depth) };
                    let liq_str = if liq.is_empty() { "{}".into() } else { String::from_utf8_lossy(&liq) };
                    
                    let combined = format!("{{\"agg\":{}, \"depth\":{}, \"liq\":{}}}", 
                        String::from_utf8_lossy(&agg), 
                        depth_str, 
                        liq_str
                    );
                    
                    if has_validator && !depth.is_empty() {
                        let _ = app.orchestrator.call_endpoint_with_data("validator_01", StandardEndpoint::DataValid, Some(combined.clone().into_bytes()));
                    }
                    if has_tps {
                        let _ = app.orchestrator.call_endpoint_with_data("tps_01", StandardEndpoint::DataValid, Some(combined.into_bytes()));
                    }
                }
            }
        }
    }
    
    disable_raw_mode()?;
    Ok(())
}

