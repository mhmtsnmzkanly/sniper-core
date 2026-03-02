use std::path::PathBuf;
use std::collections::{HashSet, HashMap};
use crate::config::AppConfig;
use serde::{Serialize, Deserialize};

#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Scrape,
    Automation,
    Translate,
    Settings,
}

#[derive(Clone, Debug)]
pub struct MediaAsset {
    pub name: String,
    pub url: String,
    pub mime_type: String,
    pub size_bytes: usize,
    pub data: Option<Vec<u8>>,
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

#[derive(Clone, Debug, serde::Deserialize, Serialize, Default)]
pub struct ChromeCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub expires: f64,
    pub secure: bool,
    pub http_only: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct NetworkRequest {
    pub request_id: String,
    pub url: String,
    pub method: String,
    pub resource_type: String,
    pub status: Option<u16>,
    pub request_body: Option<String>,
    pub response_body: Option<String>,
}

pub struct TabWorkspace {
    pub tab_id: String,
    pub title: String,
    
    // Window Visibility Flags
    pub show_network: bool,
    pub show_media: bool,
    pub show_storage: bool,
    
    // Data
    pub network_requests: Vec<NetworkRequest>,
    pub media_assets: Vec<MediaAsset>,
    pub selected_media_urls: HashSet<String>,
    pub console_logs: Vec<String>,
    pub cookies: Vec<ChromeCookie>,
    pub cookie_edit_buffer: ChromeCookie,
    pub show_cookie_modal: bool,
    
    // Automation
    pub auto_steps: Vec<AutomationStep>,
    pub auto_status: AutomationStatus,
    pub js_script: String,
    pub js_result: String,
    
    // Local UI State
    pub network_search: String,
    pub media_search: String,
    pub sniffer_active: bool,
    pub auto_reload_triggered: bool,
    pub open_time: f64,
}

impl TabWorkspace {
    pub fn new(id: String, title: String) -> Self {
        Self {
            tab_id: id,
            title,
            show_network: false,
            show_media: false,
            show_storage: false,
            network_requests: Vec::new(),
            media_assets: Vec::new(),
            selected_media_urls: HashSet::new(),
            console_logs: Vec::new(),
            cookies: Vec::new(),
            cookie_edit_buffer: ChromeCookie::default(),
            show_cookie_modal: false,
            auto_steps: Vec::new(),
            auto_status: AutomationStatus::Idle,
            js_script: String::new(),
            js_result: String::new(),
            network_search: String::new(),
            media_search: String::new(),
            sniffer_active: false,
            auto_reload_triggered: false,
            open_time: 0.0,
        }
    }
}

pub struct LogEntry {
    pub message: String,
    pub level: tracing::Level,
    pub timestamp: String,
}

pub struct Notification {
    pub title: String,
    pub message: String,
    pub is_error: bool,
}

pub struct AppState {
    pub active_tab: Tab,
    pub config: AppConfig,
    pub session_timestamp: String,
    pub profile_confirmed: bool,
    pub use_custom_profile: bool,
    pub notification: Option<Notification>,
    pub scrape_url: String,
    pub available_tabs: Vec<ChromeTabInfo>,
    pub selected_tab_id: Option<String>,
    pub is_browser_running: bool,
    pub last_tab_refresh: f64,
    pub is_translating: bool,
    pub workspaces: HashMap<String, TabWorkspace>,
    pub logs: Vec<LogEntry>,
}

impl AppState {
    pub fn new(config: AppConfig, timestamp: String) -> Self {
        Self {
            active_tab: Tab::Scrape,
            config,
            session_timestamp: timestamp,
            profile_confirmed: false,
            use_custom_profile: true,
            notification: None,
            scrape_url: String::new(),
            available_tabs: Vec::new(),
            selected_tab_id: None,
            is_browser_running: false,
            last_tab_refresh: 0.0,
            is_translating: false,
            workspaces: HashMap::new(),
            logs: Vec::new(),
        }
    }

    pub fn notify(&mut self, title: &str, message: &str, is_error: bool) {
        self.notification = Some(Notification {
            title: title.to_string(),
            message: message.to_string(),
            is_error,
        });
    }
}
