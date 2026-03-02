use crate::state::ChromeTabInfo;
use std::process::Child;

#[derive(Debug)]
pub enum AppEvent {
    // Browser Olayları
    BrowserStarted(Child),
    BrowserTerminated,
    TabsUpdated(Vec<ChromeTabInfo>),
    
    // UI Komutları
    RequestCapture(String, bool), // tab_id, mirror_mode
    RequestScriptExecution(String, String), // tab_id, script
    RequestNetworkToggle(String, bool), // tab_id, enabled
    RequestAutomationRun(String, Vec<crate::state::AutomationStep>),
    RequestCookies(String),
    RequestEmulation(String, String, f64, f64), // tab_id, ua, lat, lon
    RequestTabRefresh,
    
    // Durum Olayları
    CookiesReceived(Vec<crate::state::ChromeCookie>),
    AutomationProgress(usize),
    AutomationFinished,
    AutomationError(String),
    NetworkRequestSent(crate::state::NetworkRequest),
    NetworkResponseReceived(String, u16), // request_id, status
    ScriptFinished(String),
    OperationSuccess(String),
    OperationError(String),
}
