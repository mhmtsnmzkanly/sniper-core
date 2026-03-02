use crate::config::AppConfig;
use serde::{Serialize, Deserialize};

#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Scrape,
    Automation,
    Network,
    Storage,
    Translate,
    Settings,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum AutomationStep {
    Navigate(String),
    Click(String),
    Wait(u64),
    WaitSelector(String),
    ScrollBottom,
    ExtractText(String),
    InjectJS(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum AutomationStatus {
    Idle,
    Running(usize),
    Finished,
    Error(String),
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

#[derive(Clone, Debug, serde::Deserialize, Serialize)]
pub struct ChromeCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub expires: f64,
    pub secure: bool,
    pub http_only: bool,
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
    pub auto_steps: Vec<AutomationStep>,
    pub auto_status: AutomationStatus,

    // Network State
    pub network_requests: Vec<NetworkRequest>,
    pub network_recording: bool,

    // Storage State
    pub cookies: Vec<ChromeCookie>,
    
    // Emulation Settings
    pub user_agent_override: String,
    pub latitude: f64,
    pub longitude: f64,
    
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
            auto_steps: Vec::new(),
            auto_status: AutomationStatus::Idle,
            network_requests: Vec::new(),
            network_recording: false,
            cookies: Vec::new(),
            user_agent_override: String::new(),
            latitude: 41.0082, // Default Istanbul
            longitude: 28.9784,
            is_translating: false,
            logs: Vec::new(),
        }
    }
}
