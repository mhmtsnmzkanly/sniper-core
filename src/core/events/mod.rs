use crate::state::{ChromeTabInfo, AutomationStep, ChromeCookie, NetworkRequest};
use std::process::Child;

#[derive(Debug)]
pub enum AppEvent {
    // Browser
    BrowserStarted(Child),
    BrowserTerminated,
    TabsUpdated(Vec<ChromeTabInfo>),
    ConsoleLogAdded(String),
    
    // Commands
    RequestCapture(String, bool),
    RequestScriptExecution(String, String),
    RequestNetworkToggle(String, bool),
    RequestAutomationRun(String, Vec<AutomationStep>),
    RequestTabRefresh,
    TerminateBrowser,
    
    // Storage Commands
    RequestCookies(String),
    RequestCookieDelete(String, String, String), // tab_id, name, domain
    RequestCookieAdd(String, ChromeCookie),
    
    // Data Returns
    CookiesReceived(Vec<ChromeCookie>),
    AutomationProgress(usize),
    AutomationFinished,
    AutomationError(String),
    NetworkRequestSent(NetworkRequest),
    NetworkResponseReceived(String, u16, Option<String>), // id, status, body
    ScriptFinished(String),
    OperationSuccess(String),
    OperationError(String),
}
