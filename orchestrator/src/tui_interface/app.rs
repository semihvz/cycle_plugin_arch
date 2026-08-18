use crate::orchestrator::Orchestrator;
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(PartialEq)]
pub enum ViewMode {
    Main,
    PluginSelection,
    ConfirmDelete(String),
    ContextMenu(String, u16, u16),
    Shell,
    ConfigEditor,
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum ActivePanel {
    Systems,
    Hex,
    LiveFeed,
    Shell,
    Logs,
}

pub struct App<'a> {
    pub orchestrator: Arc<Orchestrator>,
    pub log_tx: broadcast::Sender<String>,
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
    pub input_shell: String,
    pub shell_history: Vec<String>,
    pub textarea: Option<tui_textarea::TextArea<'a>>,
    pub active_panel: ActivePanel,
    pub hex_scroll: u16,
    pub live_feed_scroll: u16,
    pub logs_scroll: u16,
    pub web_server_started: bool,
    pub web_shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    pub last_sys_refresh: std::time::Instant,
}

impl<'a> App<'a> {
    pub fn new(orchestrator: Arc<Orchestrator>, log_tx: broadcast::Sender<String>) -> Self {
        let mut sys = sysinfo::System::new_all();
        sys.refresh_processes();
        Self {
            orchestrator,
            log_tx,
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
            input_shell: String::new(),
            shell_history: Vec::new(),
            textarea: None,
            active_panel: ActivePanel::Systems,
            hex_scroll: 0,
            live_feed_scroll: 0,
            logs_scroll: 0,
            web_server_started: false,
            web_shutdown_tx: None,
            last_sys_refresh: std::time::Instant::now(),
        }
    }

    pub fn refresh_sys_if_needed(&mut self) {
        if self.last_sys_refresh.elapsed().as_secs() >= 2 {
            self.sys.refresh_processes();
            self.last_sys_refresh = std::time::Instant::now();
        }
    }

    pub fn toggle_web_server(&mut self) {
        if !self.web_server_started || self.web_shutdown_tx.is_none() {
            let (tx, rx) = tokio::sync::oneshot::channel();
            self.web_shutdown_tx = Some(tx);
            self.web_server_started = true;

            let orchestrator = self.orchestrator.clone();
            let log_tx = self.log_tx.clone();
            tokio::spawn(async move {
                crate::web_server::start_web_server(orchestrator, log_tx, 8080, rx).await;
            });
            self.log("🚀 Web Arayüz Sunucusu Başlatıldı (Port 8080): http://localhost:8080");
        } else {
            if let Some(tx) = self.web_shutdown_tx.take() {
                let _ = tx.send(());
            }
            self.web_server_started = false;
            self.log("⏹️ Web Arayüz Sunucusu Durduruldu (Port 8080 Kapalı).");
        }
    }

    pub fn log(&mut self, msg: &str) {
        let now = chrono::Local::now();
        let formatted = format!("[{}] {}", now.format("%H:%M:%S.%6f"), msg);
        self.logs.push(formatted.clone());
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
        let _ = self.log_tx.send(formatted);
    }
}
