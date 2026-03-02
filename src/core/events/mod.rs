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
    RequestTabRefresh,
    
    // Durum Olayları
    OperationSuccess(String),
    OperationError(String),
}
