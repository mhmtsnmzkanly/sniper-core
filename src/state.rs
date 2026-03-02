use std::path::PathBuf;
use crate::config::AppConfig;

#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Scrape,
    Automation,
    Network,
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

#[derive(Clone, Debug)]
pub struct NetworkRequest {
    pub request_id: String,
    pub url: String,
    pub method: String,
    pub resource_type: String,
    pub status: Option<u16>,
    pub timestamp: f64,
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
    pub mirror_mode: bool,
    pub last_tab_refresh: f64,
    
    // Automation State
    pub js_script: String,
    pub js_result: String,
    pub js_execution_active: bool,

    // Network State
    pub network_requests: Vec<NetworkRequest>,
    pub network_recording: bool,
    
    // Translate State
    pub is_translating: bool,
    
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
            mirror_mode: false,
            last_tab_refresh: 0.0,
            js_script: "document.title".to_string(),
            js_result: String::new(),
            js_execution_active: false,
            network_requests: Vec::new(),
            network_recording: false,
            is_translating: false,
            logs: Vec::new(),
        }
    }
}
