use crate::state::{ChromeTabInfo, AutomationStep, ChromeCookie, NetworkRequest, MediaAsset};
use std::process::Child;

#[derive(Debug)]
pub enum AppEvent {
    // Browser
    BrowserStarted(Child),
    BrowserTerminated,
    TabsUpdated(Vec<ChromeTabInfo>),
    ConsoleLogAdded(String, String), // tab_id, message
    
    // Commands
    RequestCapture(String, bool),
    RequestScriptExecution(String, String),
    RequestNetworkToggle(String, bool),
    RequestAutomationRun(String, Vec<AutomationStep>),
    RequestTabRefresh,
    RequestPageReload(String),
    TerminateBrowser,
    
    // Storage Commands
    RequestCookies(String),
    RequestCookieDelete(String, String, String),
    RequestCookieAdd(String, ChromeCookie),
    
    // Data Returns (Target-Aware)
    MediaCaptured(String, MediaAsset), // tab_id, asset
    CookiesReceived(String, Vec<ChromeCookie>), // tab_id, cookies
    AutomationProgress(String, usize),
    AutomationFinished(String),
    AutomationError(String, String),
    NetworkRequestSent(String, NetworkRequest), // tab_id, req
    NetworkResponseReceived(String, String, u16, Option<String>), // tab_id, request_id, status, body
    ScriptFinished(String, String), // tab_id, result
    
    OperationSuccess(String),
    OperationError(String),
}
