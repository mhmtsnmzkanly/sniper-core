use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Scrape,
    Translate,
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
    pub session_timestamp: String,
    
    // Scrape Tab State
    pub scrape_url: String,
    pub scrape_path: Option<PathBuf>,
    pub remote_port: u16,
    pub custom_profile_path: Option<PathBuf>,
    pub available_tabs: Vec<ChromeTabInfo>,
    pub selected_tab_id: Option<String>,
    pub is_browser_running: bool,
    pub last_tab_refresh: f64,
    
    // Translate Tab State
    pub raw_path: Option<PathBuf>,
    pub trans_path: Option<PathBuf>,
    pub gemini_api_key: String,
    pub is_translating: bool,

    pub logs: Vec<LogEntry>,
}

impl AppState {
    pub fn new(timestamp: String) -> Self {
        let mut detected_profile = std::env::current_dir().unwrap_or_default().join("chrome_profile");
        if let Ok(home) = std::env::var("HOME") {
            let paths = vec![format!("{}/.config/google-chrome", home), format!("{}/.config/chromium", home)];
            for p in paths {
                let path = PathBuf::from(&p);
                if path.exists() { detected_profile = path; break; }
            }
        }

        Self {
            active_tab: Tab::Scrape,
            session_timestamp: timestamp,
            scrape_url: String::new(),
            scrape_path: Some(std::env::current_dir().unwrap_or_default().join("raw")),
            remote_port: 9222,
            custom_profile_path: Some(detected_profile),
            available_tabs: Vec::new(),
            selected_tab_id: None,
            is_browser_running: false,
            last_tab_refresh: 0.0,
            raw_path: None,
            trans_path: None,
            gemini_api_key: std::env::var("GEMINI_API_KEY").unwrap_or_default(),
            is_translating: false,
            logs: Vec::new(),
        }
    }
}
