use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// AppConfig: Holds global system settings.
/// Values are initialized with defaults and can be modified via the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Root directory where all captures, logs, and datasets are saved.
    pub output_dir: PathBuf,
    /// Absolute path to the Chrome/Chromium executable.
    pub chrome_binary_path: String,
    /// Absolute path to the user data directory for the browser instance.
    pub chrome_profile_path: String,
    /// The port used for Remote Debugging (CDP) communication.
    pub remote_debug_port: u16,
    /// The URL the browser opens when launched.
    pub default_launch_url: String,
    /// Optional API key for AI-powered features (e.g., Translate).
    pub gemini_api_key: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        // Platform-agnostic default output path: ~/SniperOutput
        let mut output = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        output.push("SniperOutput");

        Self {
            output_dir: output,
            chrome_binary_path: String::new(),
            chrome_profile_path: String::new(),
            remote_debug_port: 9222,
            default_launch_url: "https://www.google.com".to_string(),
            gemini_api_key: String::new(),
        }
    }
}

/// Tab: Represents the different primary views in the main UI.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum Tab {
    Scrape,
    Automation,
    Translate,
    Media,
    Network,
    Storage,
    Settings,
    Logs,
}

/// LogEntry: A structured system log message displayed in the UI and saved to disk.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

/// AutomationStatus: Tracks the execution state of the automation engine for a specific tab.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum AutomationStatus {
    Idle,
    /// Currently executing step at index X.
    Running(usize),
    Finished,
    Error(String),
}

/// AutomationStep: Symmetric with UI blocks and DSL steps.
/// Used to build and serialize automation pipelines.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AutomationStep {
    Navigate(String),
    Click(String),
    RightClick(String),
    Hover(String),
    Type { selector: String, value: String, is_variable: bool },
    Wait(u64),
    WaitSelector { selector: String, timeout_ms: u64 },
    WaitUntilIdle { timeout_ms: u64 },
    WaitNetworkIdle { timeout_ms: u64, min_idle_ms: u64 },
    Extract { selector: String, as_key: String, add_to_dataset: bool },
    NewRow,
    Export(String),
    Screenshot(String),
    SetVariable { key: String, value: String },
    ScrollBottom,
    SwitchFrame(String),
    If { selector: String, then_steps: Vec<AutomationStep> },
    ForEach { selector: String, body: Vec<AutomationStep> },
    CallFunction(String),
    ImportDataset(String),
}

/// AutomationConfig: Global execution rules for automation pipelines.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationConfig {
    pub retry_attempts: u32,
    pub screenshot_on_error: bool,
    pub step_timeout_ms: u64,
}

impl Default for AutomationConfig {
    fn default() -> Self {
        Self {
            retry_attempts: 0,
            screenshot_on_error: true,
            step_timeout_ms: 30000,
        }
    }
}

/// ChromeTabInfo: Metadata about an active browser tab retrieved via /json.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ChromeTabInfo {
    pub id: String,
    pub title: String,
    pub url: String,
    #[serde(rename = "type")]
    pub tab_type: String,
}

/// ChromeCookie: Representation of a browser cookie for the Cookie Manager.
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct ChromeCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub expires: f64,
    pub secure: bool,
    pub http_only: bool,
}

/// MediaAsset: A sniffed resource (image, css, script, etc.) with optional binary data.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MediaAsset {
    pub name: String,
    pub url: String,
    pub mime_type: String,
    pub size_bytes: usize,
    pub data: Option<Vec<u8>>,
}

/// NetworkRequest: Represents an intercepted HTTP request/response.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct NetworkRequest {
    pub request_id: String,
    pub url: String,
    pub method: String,
    pub resource_type: String,
    pub status: Option<u16>,
    pub request_body: Option<String>,
    pub response_body: Option<String>,
}

/// TabWorkspace: Independent state container for every tracked browser tab.
/// Implements MDI (Multi-Document Interface) logic.
pub struct TabWorkspace {
    pub title: String,
    // Visibility flags for sub-windows
    pub show_network: bool,
    pub show_media: bool,
    pub show_storage: bool,
    pub show_automation: bool,
    pub show_console: bool,
    // Data collections
    pub network_requests: Vec<NetworkRequest>,
    pub network_type_filter: HashSet<String>,
    pub media_assets: Vec<MediaAsset>,
    pub selected_media_urls: HashSet<String>,
    pub console_logs: Vec<String>,
    pub cookies: Vec<ChromeCookie>,
    pub cookie_edit_buffer: ChromeCookie,
    pub show_cookie_modal: bool,
    // Automation state
    pub auto_steps: Vec<AutomationStep>,
    pub auto_functions: HashMap<String, Vec<AutomationStep>>,
    pub active_fn_editor: Option<String>,
    pub auto_status: AutomationStatus,
    pub auto_config: AutomationConfig,
    // Intelligent discovery
    pub discovered_selectors: Vec<String>,
    pub variables: HashMap<String, String>,
    pub extracted_data: Vec<HashMap<String, String>>,
    // UI temporary buffers
    pub var_edit_key: String,
    pub var_edit_val: String,
    pub js_script: String,
    pub js_result: String,
    pub network_search: String,
    pub media_search: String,
    pub media_type_filter: HashSet<String>,
    pub media_preview_size: f32,
    pub show_media_export: bool,
    pub media_export_types: HashSet<String>,
    pub media_export_cols: HashSet<String>,
    pub media_sort_col: String,
    pub media_sort_asc: bool,
    pub sniffer_active: bool,
    pub auto_reload_triggered: bool,
    pub open_time: f64,
    pub active_request_id: Option<String>,
    pub active_media_url: Option<String>,
    pub selector_search: String,
    pub blocked_urls: HashSet<String>,
}

impl TabWorkspace {
    pub fn new(_tid: String, title: String) -> Self {
        Self {
            title,
            show_network: false,
            show_media: false,
            show_storage: false,
            show_automation: false,
            show_console: false,
            network_requests: Vec::new(),
            network_type_filter: HashSet::new(),
            media_assets: Vec::new(),
            selected_media_urls: HashSet::new(),
            console_logs: Vec::new(),
            cookies: Vec::new(),
            cookie_edit_buffer: ChromeCookie::default(),
            show_cookie_modal: false,
            auto_steps: Vec::new(),
            auto_functions: HashMap::new(),
            active_fn_editor: None,
            auto_status: AutomationStatus::Idle,
            auto_config: AutomationConfig::default(),
            discovered_selectors: Vec::new(),
            variables: HashMap::new(),
            extracted_data: Vec::new(),
            var_edit_key: String::new(),
            var_edit_val: String::new(),
            js_script: String::new(),
            js_result: String::new(),
            network_search: String::new(),
            media_search: String::new(),
            media_type_filter: HashSet::new(),
            media_preview_size: 100.0,
            show_media_export: false,
            media_export_types: HashSet::new(),
            media_export_cols: HashSet::new(),
            media_sort_col: "name".to_string(),
            media_sort_asc: true,
            sniffer_active: false,
            auto_reload_triggered: false,
            open_time: 0.0,
            active_request_id: None,
            active_media_url: None,
            selector_search: String::new(),
            blocked_urls: HashSet::new(),
        }
    }
}

pub struct Notification {
    pub title: String,
    pub message: String,
}

/// AppState: The root state object for the entire application.
pub struct AppState {
    pub active_tab: Tab,
    pub config: AppConfig,
    pub is_browser_running: bool,
    pub available_tabs: Vec<ChromeTabInfo>,
    pub selected_tab_id: Option<String>,
    /// Map of TabID -> TabWorkspace.
    pub workspaces: HashMap<String, TabWorkspace>,
    pub logs: Vec<LogEntry>,
    pub session_timestamp: String,
    pub notification: Option<Notification>,
    pub last_tab_refresh: f64,
    pub is_translating: bool,
    // Setup wizard flags
    pub output_confirmed: bool,
    pub profile_confirmed: bool,
    pub use_custom_profile: bool,
}

impl AppState {
    pub fn new(config: AppConfig, session_ts: String) -> Self {
        Self {
            active_tab: Tab::Scrape,
            config,
            is_browser_running: false,
            available_tabs: Vec::new(),
            selected_tab_id: None,
            workspaces: HashMap::new(),
            logs: Vec::new(),
            session_timestamp: session_ts,
            notification: None,
            last_tab_refresh: 0.0,
            is_translating: false,
            output_confirmed: false,
            profile_confirmed: false,
            use_custom_profile: true,
        }
    }

    /// Triggers a toast-style notification in the UI.
    pub fn notify(&mut self, title: &str, message: &str, _is_error: bool) {
        self.notification = Some(Notification {
            title: title.to_string(),
            message: message.to_string(),
        });
    }
}
