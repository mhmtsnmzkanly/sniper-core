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
    RequestCapture(String, bool),
    RequestScriptExecution(String, String), // Added back
    RequestNetworkToggle(String, bool),
    RequestAutomationRun(String, Vec<AutomationStep>), // Added back
    RequestTabRefresh,
    RequestPageReload(String),
    TerminateBrowser,
    
    // --- STORAGE COMMANDS ---
    RequestCookies(String),
    RequestCookieDelete(String, String, String),
    RequestCookieAdd(String, ChromeCookie),
    
    // --- DATA RETURN EVENTS (TAB-AWARE) ---
    MediaCaptured(String, MediaAsset),
    CookiesReceived(String, Vec<ChromeCookie>),
    AutomationProgress(String, usize), // Added back
    AutomationFinished(String), // Added back
    AutomationError(String, String), // Added back
    NetworkRequestSent(String, NetworkRequest),
    NetworkResponseReceived(String, String, u16, Option<String>),
    ScriptFinished(String, String), // Added back
    
    // --- FEEDBACK EVENTS ---
    OperationSuccess(String),
    OperationError(String),
}
