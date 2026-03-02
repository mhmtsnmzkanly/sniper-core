use crate::state::ChromeTabInfo;
use std::process::Child;

#[derive(Debug)]
pub enum AppEvent {
    // Browser Olayları
    BrowserStarted(Child),
    BrowserTerminated,
    TabsUpdated(Vec<ChromeTabInfo>),
    
    // UI Komutları
    RequestCapture(String), // tab_id
    RequestTabRefresh,
    
    // Durum Olayları
    OperationSuccess(String),
    OperationError(String),
}
