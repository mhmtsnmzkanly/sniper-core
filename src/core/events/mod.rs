use crate::state::{ChromeTabInfo, AutomationStep, ChromeCookie, NetworkRequest, MediaAsset};
use std::process::Child;

/// Central event bus for the application.
#[derive(Debug)]
pub enum AppEvent {
    // --- BROWSER PROCESS EVENTS ---
    BrowserStarted(Child),
    BrowserTerminated,
    TabsUpdated(Vec<ChromeTabInfo>),
    ConsoleLogAdded(String, String),
    
    // --- COMMAND EVENTS ---
    RequestCapture(String, String), // tab_id, mode (html/complete/mirror)
    RequestScriptExecution(String, String),
    RequestNetworkToggle(String, bool),
    RequestAutomationRun(String, Vec<AutomationStep>, std::collections::HashMap<String, Vec<AutomationStep>>, crate::state::AutomationConfig),
    RequestTabRefresh,
    RequestPageReload(String),
    RequestUrlBlock(String, String),
    RequestUrlUnblock(String, String),
    RequestPageSelectors(String),
    SelectorsReceived(String, Vec<String>),
    AutomationDatasetUpdated(String, Vec<std::collections::HashMap<String, String>>),
    TerminateBrowser,
    
    // --- SETUP EVENTS ---
    RequestLogPathSet(std::path::PathBuf),

    // --- STORAGE COMMANDS ---
    RequestCookies(String),
    RequestCookieDelete(String, String, String),
    RequestCookieAdd(String, ChromeCookie),
    
    // --- DATA RETURN EVENTS (TAB-AWARE) ---
    MediaCaptured(String, MediaAsset),
    CookiesReceived(String, Vec<ChromeCookie>),
    AutomationProgress(String, usize),
    AutomationFinished(String),
    AutomationError(String, String),
    NetworkRequestSent(String, NetworkRequest),
    NetworkResponseReceived(String, String, u16, Option<String>),
    ScriptFinished(String, String),
    
    // --- FEEDBACK EVENTS ---
    OperationSuccess(String),
    OperationError(String),
}
