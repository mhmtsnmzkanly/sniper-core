use std::collections::{HashSet, HashMap};
use crate::config::AppConfig;
use serde::{Serialize, Deserialize};

/// Represents the primary navigation sections of the application.
#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Scrape,
    Automation,
    Translate,
    Settings,
}

/// Captured binary media asset.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaAsset {
    pub name: String,
    pub url: String,
    pub mime_type: String,
    pub size_bytes: usize,
    pub data: Option<Vec<u8>>,
}

/// Atomic operation in the automation pipeline.
/// SIMMETRIC WITH dsl::Step FOR PERFECT SAVE/LOAD
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum AutomationStep {
    Navigate(String),
    Click(String),
    Type { selector: String, value: String, is_variable: bool },
    Wait(u64),
    WaitSelector { selector: String, timeout_ms: u64 },
    WaitUntilIdle { timeout_ms: u64 },
    Extract { selector: String, as_key: String, add_to_dataset: bool },
    SetVariable { key: String, value: String },
    NewRow,
    Export(String),
    Screenshot(String),
    ScrollBottom,
    If {
        selector: String,
        then_steps: Vec<AutomationStep>,
    },
    ForEach {
        selector: String,
        body: Vec<AutomationStep>,
    },
}

/// Current status of a tab's automation pipeline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AutomationStatus {
    Idle,
    Running(usize),
    Finished,
    Error(String),
}

/// Metadata for a Chrome/Chromium tab retrieved via CDP.
#[derive(Clone, Debug, serde::Deserialize, Serialize)]
pub struct ChromeTabInfo {
    pub id: String,
    pub title: String,
    pub url: String,
    #[serde(rename = "type")]
    pub tab_type: String,
    #[serde(rename = "webSocketDebuggerUrl")]
    pub web_socket_url: String,
}

/// Browser cookie metadata.
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

/// Intercepted network request/response pair.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NetworkRequest {
    pub request_id: String,
    pub url: String,
    pub method: String,
    pub resource_type: String,
    pub status: Option<u16>,
    pub request_body: Option<String>,
    pub response_body: Option<String>,
}

/// Independent workspace for a specific browser tab.
pub struct TabWorkspace {
    pub title: String,
    pub show_network: bool,
    pub show_media: bool,
    pub show_storage: bool,
    pub show_automation: bool,
    pub network_requests: Vec<NetworkRequest>,
    pub media_assets: Vec<MediaAsset>,
    pub selected_media_urls: HashSet<String>,
    pub console_logs: Vec<String>,
    pub cookies: Vec<ChromeCookie>,
    pub cookie_edit_buffer: ChromeCookie,
    pub show_cookie_modal: bool,
    pub auto_steps: Vec<AutomationStep>,
    pub auto_status: AutomationStatus,
    pub variables: HashMap<String, String>,
    pub extracted_data: Vec<HashMap<String, String>>,
    pub var_edit_key: String,
    pub var_edit_val: String,
    pub js_script: String,
    pub js_result: String,
    pub network_search: String,
    pub media_search: String,
    pub sniffer_active: bool,
    pub auto_reload_triggered: bool,
    pub open_time: f64,
    pub active_request_id: Option<String>,
    pub active_media_url: Option<String>,
    pub blocked_urls: HashSet<String>,
    pub discovered_selectors: Vec<String>,
    pub selector_search: String,
    pub media_sort_col: String,
    pub media_sort_asc: bool,
}

impl TabWorkspace {
    pub fn new(_id: String, title: String) -> Self {
        Self {
            title,
            show_network: false, show_media: false, show_storage: false, show_automation: false,
            network_requests: Vec::new(), media_assets: Vec::new(),
            selected_media_urls: HashSet::new(), console_logs: Vec::new(),
            cookies: Vec::new(), cookie_edit_buffer: ChromeCookie::default(),
            show_cookie_modal: false,
            auto_steps: Vec::new(), auto_status: AutomationStatus::Idle,
            variables: HashMap::new(),
            extracted_data: Vec::new(),
            var_edit_key: String::new(),
            var_edit_val: String::new(),
            js_script: String::new(), js_result: String::new(),
            network_search: String::new(), media_search: String::new(),
            sniffer_active: false, auto_reload_triggered: false, open_time: 0.0,
            active_request_id: None,
            active_media_url: None,
            blocked_urls: HashSet::new(),
            discovered_selectors: Vec::new(),
            selector_search: String::new(),
            media_sort_col: "name".to_string(),
            media_sort_asc: true,
        }
    }
}

#[derive(Clone)]
pub struct LogEntry {
    pub message: String,
    pub level: tracing::Level,
    pub timestamp: String,
}
