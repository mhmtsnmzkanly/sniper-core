use std::path::PathBuf;
use crate::config::AppConfig;

#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Scrape,
    Translate,
    Settings,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct ChromeTabInfo {
    pub id: String,
    pub title: String,
    pub url: String,
    #[serde(rename = "type")]
    pub tab_type: String,
    #[serde(rename = "webSocketDebuggerUrl")]
    pub web_socket_url: String,
}

pub struct LogEntry {
    pub message: String,
    pub level: tracing::Level,
    pub timestamp: String,
}

pub struct AppState {
    pub active_tab: Tab,
    pub config: AppConfig,
    pub session_timestamp: String,
    
    // UI State
    pub scrape_url: String,
    pub available_tabs: Vec<ChromeTabInfo>,
    pub selected_tab_id: Option<String>,
    pub is_browser_running: bool,
    pub last_tab_refresh: f64,
    
    // Logs
    pub logs: Vec<LogEntry>,
}

impl AppState {
    pub fn new(config: AppConfig, timestamp: String) -> Self {
        Self {
            active_tab: Tab::Scrape,
            config,
            session_timestamp: timestamp,
            scrape_url: String::new(),
            available_tabs: Vec::new(),
            selected_tab_id: None,
            is_browser_running: false,
            last_tab_refresh: 0.0,
            logs: Vec::new(),
        }
    }
}
