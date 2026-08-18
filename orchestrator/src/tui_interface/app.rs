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
}

impl<'a> App<'a> {
    pub fn new(orchestrator: Arc<Orchestrator>, log_tx: broadcast::Sender<String>) -> Self {
        let mut sys = sysinfo::System::new_all();
        sys.refresh_all();
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
        }
    }

    pub fn log(&mut self, msg: &str) {
        let now = chrono::Local::now();
        let formatted = format!("[{}] {}", now.format("%H:%M:%S"), msg);
        self.logs.push(formatted.clone());
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
        let _ = self.log_tx.send(formatted);
    }
}
