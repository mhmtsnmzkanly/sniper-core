use crate::core::events::AppEvent;
use crate::state::{AppState, AutomationStatus, Tab, AutomationStep, LogEntry, NotificationLevel};
use crate::ui;
use eframe::egui;
use egui::{Color32, RichText};
use std::sync::{Arc, Mutex};

/// CrawlerApp: The main application entry point for the UI thread.
/// It orchestrates the global state, processes events from background tasks, and renders the GUI.
pub struct CrawlerApp {
    /// Unified application state.
    pub state: AppState,
    /// Receives system logs from the logging bridge.
    pub log_receiver: tokio::sync::mpsc::UnboundedReceiver<LogEntry>,
    /// Receives command results and system events from background threads.
    pub event_receiver: tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    /// Thread-safe handle to the active browser process.
    pub browser_process: Arc<Mutex<Option<std::process::Child>>>,
}

impl CrawlerApp {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        state: AppState,
        log_receiver: tokio::sync::mpsc::UnboundedReceiver<LogEntry>,
        event_receiver: tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    ) -> Self {
        Self { state, log_receiver, event_receiver, browser_process: Arc::new(Mutex::new(None)) }
    }

    /// Ensures a workspace exists for the given tab ID.
    /// Lazy-initializes the workspace if it's the first time seeing this tab.
    fn ensure_workspace(&mut self, tid: &str) -> &mut crate::state::TabWorkspace {
        if !self.state.workspaces.contains_key(tid) {
            let title = self.state.available_tabs.iter().find(|t| t.id == tid).map(|t| t.title.clone()).unwrap_or_else(|| "New Tab".into());
            tracing::debug!("[APP] Creating workspace: {} ({})", title, tid);
            self.state.workspaces.insert(tid.to_string(), crate::state::TabWorkspace::new(tid.to_string(), title));
        }
        self.state.workspaces.get_mut(tid).unwrap()
    }
}

/// Helper to map high-level UI automation steps into low-level engine DSL variants.
fn map_ui_steps_to_dsl(steps: &[AutomationStep]) -> Vec<crate::core::automation::dsl::Step> {
    steps.iter().map(|s| match s {
        AutomationStep::Navigate(u) => crate::core::automation::dsl::Step::Navigate { url: u.clone() },
        AutomationStep::Click(sel) => crate::core::automation::dsl::Step::Click { selector: sel.clone() },
        AutomationStep::RightClick(sel) => crate::core::automation::dsl::Step::RightClick { selector: sel.clone() },
        AutomationStep::Hover(sel) => crate::core::automation::dsl::Step::Hover { selector: sel.clone() },
        AutomationStep::Type { selector, value, is_variable } => crate::core::automation::dsl::Step::Type { selector: selector.clone(), value: value.clone(), is_variable: *is_variable },
        AutomationStep::Wait(secs) => crate::core::automation::dsl::Step::Wait { seconds: *secs },
        AutomationStep::WaitSelector { selector, timeout_ms } => crate::core::automation::dsl::Step::WaitSelector { selector: selector.clone(), timeout_ms: *timeout_ms },
        AutomationStep::WaitUntilIdle { timeout_ms } => crate::core::automation::dsl::Step::WaitUntilIdle { timeout_ms: *timeout_ms },
        AutomationStep::WaitNetworkIdle { timeout_ms, min_idle_ms } => crate::core::automation::dsl::Step::WaitNetworkIdle { timeout_ms: *timeout_ms, min_idle_ms: *min_idle_ms },
        AutomationStep::ScrollBottom => crate::core::automation::dsl::Step::ScrollBottom,
        AutomationStep::Extract { selector, as_key, add_to_dataset } => crate::core::automation::dsl::Step::Extract { selector: selector.clone(), as_key: as_key.clone(), add_to_row: *add_to_dataset },
        AutomationStep::SetVariable { key, value } => crate::core::automation::dsl::Step::SetVariable { key: key.clone(), value: value.clone() },
        AutomationStep::NewRow => crate::core::automation::dsl::Step::NewRow,
        AutomationStep::Export(f) => crate::core::automation::dsl::Step::Export { filename: f.clone() },
        AutomationStep::Screenshot(f) => crate::core::automation::dsl::Step::Screenshot { filename: f.clone() },
        AutomationStep::SwitchFrame(sel) => crate::core::automation::dsl::Step::SwitchFrame { selector: sel.clone() },
        AutomationStep::If { selector, then_steps } => crate::core::automation::dsl::Step::If { selector: selector.clone(), then_steps: map_ui_steps_to_dsl(then_steps) },
        AutomationStep::ForEach { selector, body } => crate::core::automation::dsl::Step::ForEach { selector: selector.clone(), body: map_ui_steps_to_dsl(body) },
        AutomationStep::CallFunction(name) => crate::core::automation::dsl::Step::CallFunction { name: name.clone() },
        AutomationStep::ImportDataset(f) => crate::core::automation::dsl::Step::ImportDataset { filename: f.clone() },
    }).collect()
}

impl eframe::App for CrawlerApp {
    /// The GUI update loop. Called ~60 times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // KOD NOTU: Tüm ekranlarda tutarlı görsel dil için global tema her frame uygulanır.
        ui::design::apply_theme(ctx);

        // KOD NOTU: Her 2 saniyede bir browser'ın hayatta olup olmadığını kontrol eder.
        // Bu, manuel kapatılan browser instance'larını tespit etmek için kritiktir.
        if self.state.is_browser_running {
            let now = ctx.input(|i| i.time);
            if now - self.state.last_health_check > 2.0 {
                self.state.last_health_check = now;
                let port = self.state.config.remote_debug_port;
                tokio::spawn(async move {
                    if !crate::core::browser::BrowserManager::check_health(port).await {
                        crate::ui::scrape::emit(AppEvent::BrowserTerminated);
                    }
                });
            }
        }

        // 1. Drain the log queue and update the system logs list.
        while let Ok(log) = self.log_receiver.try_recv() {
            self.state.logs.push(log);
            if self.state.logs.len() > 1500 { self.state.logs.remove(0); }
        }

        // 2. GLOBAL EVENT DISPATCHER: Process all incoming events and route them to state or core.
        while let Ok(event) = self.event_receiver.try_recv() {
            tracing::debug!("[EVENT] Dispatching: {:?}", event);

            // Command Guard: Block browser commands if instance is down.
            if !self.state.is_browser_running {
                match &event {
                    AppEvent::RequestCookies(_) | AppEvent::RequestPageReload(_) | 
                    AppEvent::RequestScriptExecution(_, _) | AppEvent::RequestAutomationRun(..) |
                    AppEvent::RequestCapture(..) | AppEvent::RequestPageSelectors(_) |
                    AppEvent::RequestScriptingRun(..) |
                    AppEvent::RequestTabRefresh => {
                        let msg = "Action Denied: Browser instance is not active.";
                        self.state.notify(NotificationLevel::Warn, "Denied", msg);
                        tracing::warn!("[APP] {}", msg);
                        continue;
                    }
                    _ => {}
                }
            }

            match event {
                AppEvent::RequestLogPathSet(path) => {
                    tracing::info!("[APP -> CORE] Output path confirmed: {:?}", path);
                    crate::logger::set_log_path(path, &self.state.session_timestamp);
                }
                AppEvent::BrowserStarted(child) => {
                    // KOD NOTU: Launch edilen process handle'ını saklıyoruz; terminate komutu bunu kullanacak.
                    *self.browser_process.lock().unwrap() = Some(child);
                    self.state.is_browser_running = true;
                    self.state.notify(NotificationLevel::Ok, "System", "Browser connected.");
                    tracing::info!("[BROWSER -> APP] Remote instance handshake successful.");
                }
                AppEvent::BrowserTerminated => {
                    // KOD NOTU: Kopan instance sonrası process handle ve listener flag'leri temizlenir.
                    self.browser_process.lock().unwrap().take();
                    self.state.is_browser_running = false;
                    self.state.available_tabs.clear();
                    self.state.selected_tab_id = None;
                    for ws in self.state.workspaces.values_mut() {
                        ws.sniffer_active = false;
                        if let Some(token) = ws.sniffer_token.take() {
                            token.store(false, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                    self.state.notify(NotificationLevel::Error, "System", "Browser disconnected.");
                    tracing::warn!("[BROWSER -> APP] Remote instance heartbeat lost.");
                }
                AppEvent::TerminateBrowser => {
                    if let Some(mut child) = self.browser_process.lock().unwrap().take() {
                        let _ = child.kill();
                    }
                    self.state.is_browser_running = false;
                    self.state.available_tabs.clear();
                    self.state.selected_tab_id = None;
                    for ws in self.state.workspaces.values_mut() {
                        ws.sniffer_active = false;
                        if let Some(token) = ws.sniffer_token.take() {
                            token.store(false, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                    tracing::info!("[UI -> CORE] User terminated browser instance.");
                }
                AppEvent::TabsUpdated(tabs) => {
                    tracing::debug!("[BROWSER -> CORE] Received {} active tab targets.", tabs.len());
                    self.state.available_tabs = tabs;
                }
                AppEvent::ConsoleLogAdded(tid, msg) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.console_logs.push(msg.clone());
                    self.state.logs.push(LogEntry {
                        timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                        level: "CHROME".to_string(),
                        message: format!("[{}] {}", tid, msg),
                    });
                    crate::logger::write_chrome_log_line(&format!("[{}] {}", tid, msg));
                    if self.state.logs.len() > 1500 { self.state.logs.remove(0); }
                    // AUDIT MIRROR: Copy browser console output to Sniper logs.
                    tracing::info!("[BROWSER-CONSOLE][{}] {}", tid, msg);
                }
                AppEvent::SelectorsReceived(tid, sels) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.discovered_selectors = sels;
                    tracing::info!("[BROWSER -> CORE] Captured {} selectors from tab {}", ws.discovered_selectors.len(), tid);
                }
                AppEvent::MediaCaptured(tid, asset) => {
                    let ws = self.ensure_workspace(&tid);
                    if let Some(existing) = ws.media_assets.iter_mut().find(|a| a.url == asset.url) {
                        // KOD NOTU: Eğer asset zaten varsa ve yeni gelen asset bir thumbnail içeriyorsa güncelle.
                        if asset.thumbnail.is_some() {
                            existing.thumbnail = asset.thumbnail;
                        }
                    } else {
                        tracing::debug!("[BROWSER -> CORE] Media sniffed: {} ({})", asset.name, asset.mime_type);
                        ws.media_assets.push(asset);
                    }
                }
                AppEvent::CookiesReceived(tid, cookies) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.cookies = cookies;
                    tracing::info!("[BROWSER -> CORE] Syncing {} cookies for tab {}", ws.cookies.len(), tid);
                }
                AppEvent::AutomationProgress(tid, step) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.auto_status = AutomationStatus::Running(step);
                    tracing::debug!("[ENGINE -> APP] Pipeline progress: Step {} on tab {}", step + 1, tid);
                }
                AppEvent::AutomationFinished(tid) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.auto_status = AutomationStatus::Finished;
                    tracing::info!("[ENGINE -> APP] Pipeline successfully completed on tab {}", tid);
                }
                AppEvent::AutomationError(tid, err) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.auto_status = AutomationStatus::Error(err.clone());
                    tracing::error!("[ENGINE -> APP] Pipeline ABORTED on tab {}: {}", tid, err);
                }
                AppEvent::AutomationDatasetUpdated(tid, data) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.extracted_data = data;
                }
                AppEvent::NetworkRequestSent(tid, req) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.network_requests.push(req);
                }
                AppEvent::NetworkResponseReceived(tid, rid, status, body) => {
                    let ws = self.ensure_workspace(&tid);
                    if let Some(req) = ws.network_requests.iter_mut().find(|r| r.request_id == rid) {
                        req.status = Some(status);
                        req.response_body = body;
                    }
                }
                AppEvent::OperationSuccess(msg) => {
                    self.state.notify(NotificationLevel::Ok, "Success", &msg);
                    tracing::info!("[CORE -> APP] Success: {}", msg);
                }
                AppEvent::OperationError(msg) => {
                    self.state.notify(NotificationLevel::Error, "Error", &msg);
                    tracing::error!("[CORE -> APP] Failure: {}", msg);
                }
                AppEvent::ScriptingOutput(msg) => {
                    self.state.logs.push(LogEntry {
                        timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                        level: "SCRIPT".to_string(),
                        message: msg,
                    });
                    if self.state.logs.len() > 1500 { self.state.logs.remove(0); }
                }
                AppEvent::ScriptingCheckResult(report) => {
                    for line in report.diagnostics {
                        self.state.logs.push(LogEntry {
                            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                            level: if report.ok { "CHECK".to_string() } else { "CHECK-ERR".to_string() },
                            message: line,
                        });
                    }
                    self.state.notify(
                        if report.ok { NotificationLevel::Ok } else { NotificationLevel::Error },
                        "Scripting Check",
                        if report.ok { "Check completed successfully." } else { "Check failed. See System Telemetry." },
                    );
                }
                AppEvent::ScriptingDryRunResult(lines) => {
                    for line in lines {
                        self.state.logs.push(LogEntry {
                            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                            level: "DRYRUN".to_string(),
                            message: line,
                        });
                    }
                    self.state.notify(NotificationLevel::Ok, "Scripting Dry-Run", "Dry-run plan generated.");
                }
                AppEvent::ScriptingDebugPlanResult(lines) => {
                    // KOD NOTU: Debug plan Scripting sekmesinde step-by-step gezinti için state'e kaydedilir.
                    self.state.scripting_debug_plan = lines;
                    self.state.scripting_debug_index = 0;
                    self.state.notify(NotificationLevel::Ok, "Script Debugger", "Debug plan generated.");
                }
                AppEvent::ScriptingFinished => {
                    self.state.is_script_running = false;
                    if let Some(token) = self.state.scripting_cancel_token.take() {
                        token.store(false, std::sync::atomic::Ordering::Relaxed);
                    }
                    self.state.notify(NotificationLevel::Ok, "Scripting", "Script execution completed.");
                }
                AppEvent::ScriptingError(msg) => {
                    self.state.is_script_running = false;
                    if let Some(token) = self.state.scripting_cancel_token.take() {
                        token.store(false, std::sync::atomic::Ordering::Relaxed);
                    }
                    for hint in crate::core::scripting::knowledge::hints_for_error(&msg) {
                        self.state.logs.push(LogEntry {
                            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                            level: "KB".to_string(),
                            message: format!("[Hint] {}", hint),
                        });
                    }
                    self.state.script_error = Some(msg.clone());
                    self.state.notify(NotificationLevel::Error, "Scripting", &msg);
                }
                
                // --- COMMAND ROUTING WITH AUDIT LOGS ---
                AppEvent::RequestCookies(tid) => {
                    tracing::info!("[UI -> CORE] Requesting cookies for tab: {}", tid);
                    let port = self.state.config.remote_debug_port;
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::get_cookies(port, tid.clone()).await {
                            Ok(cookies) => crate::ui::scrape::emit(AppEvent::CookiesReceived(tid, cookies)),
                            Err(e) => crate::ui::scrape::emit(AppEvent::OperationError(format!("Cookie fetch failed: {}", e))),
                        }
                    });
                }
                AppEvent::RequestCookieDelete(tid, name, domain) => {
                    tracing::info!("[UI -> CORE] Deleting cookie: {} on domain: {}", name, domain);
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    tokio::spawn(async move {
                        if let Err(e) = crate::core::browser::BrowserManager::delete_cookie(port, tid.clone(), name, domain).await {
                            crate::ui::scrape::emit(AppEvent::OperationError(format!("Delete failed: {}", e)));
                        } else {
                            let _ = crate::core::browser::BrowserManager::get_cookies(port, tid_clone.clone()).await.map(|c| {
                                crate::ui::scrape::emit(AppEvent::CookiesReceived(tid_clone, c));
                            });
                        }
                    });
                }
                AppEvent::RequestCookieAdd(tid, cookie) => {
                    tracing::info!("[UI -> CORE] Injecting new cookie: {}", cookie.name);
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    tokio::spawn(async move {
                        if let Err(e) = crate::core::browser::BrowserManager::add_cookie(port, tid.clone(), cookie).await {
                            crate::ui::scrape::emit(AppEvent::OperationError(format!("Add failed: {}", e)));
                        } else {
                            let _ = crate::core::browser::BrowserManager::get_cookies(port, tid_clone.clone()).await.map(|c| {
                                crate::ui::scrape::emit(AppEvent::CookiesReceived(tid_clone, c));
                            });
                        }
                    });
                }
                AppEvent::RequestPageReload(tid) => {
                    tracing::info!("[UI -> CORE] User requested page reload for {}", tid);
                    let port = self.state.config.remote_debug_port;
                    tokio::spawn(async move {
                        if let Err(e) = crate::core::browser::BrowserManager::reload_page(port, tid).await {
                            crate::ui::scrape::emit(AppEvent::OperationError(format!("Reload failed: {}", e)));
                        }
                    });
                }
                AppEvent::RequestTabRefresh => {
                    tracing::debug!("[UI -> CORE] Refreshing active tab targets.");
                    let port = self.state.config.remote_debug_port;
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::list_tabs(port).await {
                            Ok(tabs) => crate::ui::scrape::emit(AppEvent::TabsUpdated(tabs)),
                            Err(e) => crate::ui::scrape::emit(AppEvent::OperationError(format!("Target sync failed: {}", e))),
                        }
                    });
                }
                AppEvent::RequestScriptExecution(tid, script) => {
                    tracing::info!("[UI -> CORE] Injecting custom JS script ({} bytes) to tab {}", script.len(), tid);
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::execute_script(port, tid, script).await {
                            Ok(res) => crate::ui::scrape::emit(AppEvent::ScriptFinished(tid_clone, res)),
                            Err(e) => crate::ui::scrape::emit(AppEvent::OperationError(format!("Script failed: {}", e))),
                        }
                    });
                }
                AppEvent::ScriptFinished(tid, res) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.js_result = res.clone();
                    tracing::info!("[BROWSER -> CORE] Script execution result: {}", res);
                }
                AppEvent::RequestPageSelectors(tid) => {
                    tracing::info!("[UI -> CORE] Scanning tab {} for CSS selectors.", tid);
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::get_page_selectors(port, tid).await {
                            Ok(sels) => crate::ui::scrape::emit(AppEvent::SelectorsReceived(tid_clone, sels)),
                            Err(e) => crate::ui::scrape::emit(AppEvent::OperationError(format!("Scan failed: {}", e))),
                        }
                    });
                }
                AppEvent::RequestNetworkToggle(tid, active) => {
                    // KOD NOTU: Aynı tab için listener'ı sadece bir kez başlatıyoruz (duplicate spawn engeli).
                    // Sniffer kapatıldığında gerçek bir stop signal (AtomicBool) gönderilir.
                    if !active {
                        if let Some(ws) = self.state.workspaces.get_mut(&tid) {
                            ws.sniffer_active = false;
                            if let Some(token) = ws.sniffer_token.take() {
                                token.store(false, std::sync::atomic::Ordering::Relaxed);
                            }
                        }
                        continue;
                    }

                    let (should_start, token) = {
                        let ws = self.ensure_workspace(&tid);
                        if ws.sniffer_active {
                            (false, None)
                        } else {
                            ws.sniffer_active = true;
                            let token = Arc::new(std::sync::atomic::AtomicBool::new(true));
                            ws.sniffer_token = Some(token.clone());
                            (true, Some(token))
                        }
                    };

                    if !should_start {
                        tracing::debug!("[UI -> CORE] Listener already active for tab {}, skipping.", tid);
                        continue;
                    }

                    tracing::info!("[UI -> CORE] Activating listeners for tab {}", tid);
                    let port = self.state.config.remote_debug_port;
                    tokio::spawn(async move {
                        if let Err(e) = crate::core::browser::BrowserManager::setup_tab_listeners(port, tid, token.unwrap()).await {
                            crate::ui::scrape::emit(AppEvent::OperationError(format!("Setup failed: {}", e)));
                        }
                    });
                }
                AppEvent::RequestCapture(tid, mode) => {
                    tracing::info!("[UI -> CORE] Initiating {} Capture for tab {}", mode, tid);
                    let port = self.state.config.remote_debug_port;
                    let root = self.state.config.output_dir.clone();
                    tokio::spawn(async move {
                        let res = match mode.as_str() {
                            "html" => crate::core::browser::BrowserManager::capture_html(port, tid, root).await,
                            "complete" => crate::core::browser::BrowserManager::capture_complete(port, tid, root).await,
                            "mirror" => crate::core::browser::BrowserManager::capture_mirror(port, tid, root).await,
                            _ => Err(crate::core::error::AppError::Internal("Mode not found".into())),
                        };
                        match res {
                            Ok(path) => crate::ui::scrape::emit(AppEvent::OperationSuccess(format!("Captured to: {:?}", path))),
                            Err(e) => crate::ui::scrape::emit(AppEvent::OperationError(format!("Capture failed: {}", e))),
                        }
                    });
                }
                AppEvent::RequestAutomationRun(tid, steps, funcs, auto_config) => {
                    tracing::info!("[UI -> CORE] Handing over pipeline to Engine for tab {}", tid);
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    let mut dsl_funcs = std::collections::HashMap::new();
                    for (name, f_steps) in funcs { dsl_funcs.insert(name, map_ui_steps_to_dsl(&f_steps)); }
                    let dsl = crate::core::automation::dsl::AutomationDsl {
                        dsl_version: 1, metadata: None, functions: dsl_funcs, steps: map_ui_steps_to_dsl(&steps),
                    };
                    let output_dir = self.state.config.output_dir.clone();
                    tokio::spawn(async move {
                        let config = crate::core::automation::engine::ExecutionConfig {
                            step_timeout: std::time::Duration::from_millis(auto_config.step_timeout_ms),
                            retry_attempts: auto_config.retry_attempts,
                            screenshot_on_error: auto_config.screenshot_on_error,
                        };
                        // KOD NOTU: UI Automation ve Scripting ortak runtime helper kullanır.
                        if let Err(e) = crate::core::automation::runtime::run_dsl_on_tab(
                            port,
                            tid_clone,
                            output_dir,
                            config,
                            dsl,
                        )
                        .await
                        {
                            tracing::error!("[ENGINE -> APP] Pipeline ABORTED on tab {}: {}", tid, e);
                        }
                    });
                }
                AppEvent::RequestScriptingRun(package, selected_tab_id) => {
                    if self.state.is_script_running {
                        self.state.notify(NotificationLevel::Warn, "Scripting", "Another script is already running.");
                        continue;
                    }
                    self.state.is_script_running = true;
                    let token = Arc::new(std::sync::atomic::AtomicBool::new(true));
                    self.state.scripting_cancel_token = Some(token.clone());
                    let (selected_tab_console_logs, selected_tab_cookies) = if let Some(tab_id) = &selected_tab_id {
                        if let Some(ws) = self.state.workspaces.get(tab_id) {
                            let cookies = ws.cookies.iter().map(|c| (c.name.clone(), c.value.clone())).collect();
                            (ws.console_logs.clone(), cookies)
                        } else {
                            (Vec::new(), std::collections::HashMap::new())
                        }
                    } else {
                        (Vec::new(), std::collections::HashMap::new())
                    };
                    let req = crate::core::scripting::types::ScriptExecutionRequest {
                        package,
                        selected_tab_id,
                        selected_tab_console_logs,
                        selected_tab_cookies,
                        port: self.state.config.remote_debug_port,
                        output_dir: self.state.config.output_dir.clone(),
                        cancel_token: token,
                    };
                    tokio::spawn(async move {
                        let result = crate::core::scripting::engine::run_script(req).await;
                        match result {
                            Ok(_) => crate::ui::scrape::emit(AppEvent::ScriptingFinished),
                            Err(e) => crate::ui::scrape::emit(AppEvent::ScriptingError(e.to_string())),
                        }
                    });
                }
                AppEvent::RequestScriptingCheck(package, selected_tab_id) => {
                    let _selected = selected_tab_id.or_else(|| self.state.selected_tab_id.clone());
                    let report = crate::core::scripting::engine::check_script(&package);
                    crate::ui::scrape::emit(AppEvent::ScriptingCheckResult(report));
                }
                AppEvent::RequestScriptingDryRun(package, selected_tab_id) => {
                    let selected = selected_tab_id.or_else(|| self.state.selected_tab_id.clone());
                    let (selected_tab_console_logs, selected_tab_cookies) = if let Some(tab_id) = &selected {
                        if let Some(ws) = self.state.workspaces.get(tab_id) {
                            let cookies = ws.cookies.iter().map(|c| (c.name.clone(), c.value.clone())).collect();
                            (ws.console_logs.clone(), cookies)
                        } else {
                            (Vec::new(), std::collections::HashMap::new())
                        }
                    } else {
                        (Vec::new(), std::collections::HashMap::new())
                    };
                    let req = crate::core::scripting::types::ScriptExecutionRequest {
                        package,
                        selected_tab_id: selected,
                        selected_tab_console_logs,
                        selected_tab_cookies,
                        port: self.state.config.remote_debug_port,
                        output_dir: self.state.config.output_dir.clone(),
                        cancel_token: Arc::new(std::sync::atomic::AtomicBool::new(true)),
                    };
                    match crate::core::scripting::engine::dry_run_script(req) {
                        Ok(lines) => crate::ui::scrape::emit(AppEvent::ScriptingDryRunResult(lines)),
                        Err(e) => crate::ui::scrape::emit(AppEvent::ScriptingError(format!("Dry-run failed: {}", e))),
                    }
                }
                AppEvent::RequestScriptingDebugPlan(package, selected_tab_id) => {
                    let selected = selected_tab_id.or_else(|| self.state.selected_tab_id.clone());
                    let (selected_tab_console_logs, selected_tab_cookies) = if let Some(tab_id) = &selected {
                        if let Some(ws) = self.state.workspaces.get(tab_id) {
                            let cookies = ws.cookies.iter().map(|c| (c.name.clone(), c.value.clone())).collect();
                            (ws.console_logs.clone(), cookies)
                        } else {
                            (Vec::new(), std::collections::HashMap::new())
                        }
                    } else {
                        (Vec::new(), std::collections::HashMap::new())
                    };
                    let req = crate::core::scripting::types::ScriptExecutionRequest {
                        package,
                        selected_tab_id: selected,
                        selected_tab_console_logs,
                        selected_tab_cookies,
                        port: self.state.config.remote_debug_port,
                        output_dir: self.state.config.output_dir.clone(),
                        cancel_token: Arc::new(std::sync::atomic::AtomicBool::new(true)),
                    };
                    match crate::core::scripting::engine::dry_run_script(req) {
                        Ok(lines) => crate::ui::scrape::emit(AppEvent::ScriptingDebugPlanResult(lines)),
                        Err(e) => crate::ui::scrape::emit(AppEvent::ScriptingError(format!("Debug plan failed: {}", e))),
                    }
                }
                AppEvent::RequestScriptingStop => {
                    if let Some(token) = &self.state.scripting_cancel_token {
                        token.store(false, std::sync::atomic::Ordering::Relaxed);
                    }
                    self.state.notify(NotificationLevel::Warn, "Scripting", "Stop requested.");
                }
                AppEvent::RequestScriptingImport(path) => {
                    match std::fs::read_to_string(path) {
                        Ok(content) => match serde_json::from_str::<crate::core::scripting::types::ScriptPackage>(&content) {
                            Ok(pkg) => {
                                self.state.script_package = pkg;
                                self.state.script_error = None;
                                self.state.scripting_debug_plan.clear();
                                self.state.scripting_debug_index = 0;
                                self.state.notify(NotificationLevel::Ok, "Scripting", "Script imported.");
                            }
                            Err(e) => self.state.script_error = Some(format!("Import parse error: {}", e)),
                        },
                        Err(e) => self.state.script_error = Some(format!("Import read error: {}", e)),
                    }
                }
                AppEvent::RequestScriptingExport(path, mut pkg) => {
                    pkg.updated_at = chrono::Local::now().timestamp();
                    match serde_json::to_string_pretty(&pkg) {
                        Ok(json) => match std::fs::write(path, json) {
                            Ok(_) => self.state.notify(NotificationLevel::Ok, "Scripting", "Script exported."),
                            Err(e) => self.state.script_error = Some(format!("Export write error: {}", e)),
                        },
                        Err(e) => self.state.script_error = Some(format!("Export serialize error: {}", e)),
                    }
                }
                _ => {}
            }
        }

        // --- UI RENDERING ---
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("SNIPER STUDIO").strong().size(20.0).color(ui::design::ACCENT_ORANGE));
                    ui.label(RichText::new("Browser Forensics + Automation Console").small().color(ui::design::TEXT_MUTED));
                });

                ui.add_space(14.0);
                ui.selectable_value(&mut self.state.active_tab, Tab::Scrape, "Ops");
                ui.selectable_value(&mut self.state.active_tab, Tab::Scripting, "Scripting");
                ui.selectable_value(&mut self.state.active_tab, Tab::Settings, "Config");
                ui.selectable_value(&mut self.state.active_tab, Tab::Logs, "Logs");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.state.is_browser_running {
                        ui.label(RichText::new("LIVE").color(ui::design::ACCENT_GREEN).strong());
                    } else {
                        ui.label(RichText::new("OFFLINE").color(Color32::from_rgb(255, 119, 119)).strong());
                    }
                });
            });
            ui.add_space(4.0);
        });

        // Setup Modals
        if !self.state.output_confirmed {
            egui::Window::new("Sniper Studio - Initial Setup").anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false).resizable(false).show(ctx, |ui| {
                    ui.heading("Select Output Directory");
                    ui.label("Data, logs, and assets will be stored in this location.");
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{:?}", self.state.config.output_dir)).color(Color32::KHAKI));
                        if ui.button("Browse...").clicked() { if let Some(path) = rfd::FileDialog::new().pick_folder() { self.state.config.output_dir = path; } }
                    });
                    ui.add_space(20.0);
                    if ui.button(RichText::new("CONFIRM & PROCEED").strong()).clicked() {
                        self.state.output_confirmed = true;
                        let _ = std::fs::create_dir_all(&self.state.config.output_dir);
                        crate::ui::scrape::emit(AppEvent::RequestLogPathSet(self.state.config.output_dir.clone()));
                    }
                });
            return;
        }

        if !self.state.profile_confirmed {
            egui::Window::new("Browser Profile Configuration").anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false).resizable(false).show(ctx, |ui| {
                    ui.heading("Select Profile Mode");
                    ui.vertical(|ui| {
                        if ui.selectable_label(self.state.use_custom_profile, "🏠 ISOLATED PROFILE (Recommended)").clicked() { self.state.use_custom_profile = true; }
                        if ui.selectable_label(!self.state.use_custom_profile, "👤 SYSTEM PROFILE (Existing data)").clicked() { self.state.use_custom_profile = false; }
                    });
                    ui.add_space(20.0);
                    if ui.button(RichText::new("CONFIRM & LAUNCH").strong()).clicked() { self.state.profile_confirmed = true; }
                });
            return;
        }

        // Main Panel Dispatcher
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::Frame::new()
                .fill(ui::design::BG_PRIMARY)
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| match self.state.active_tab {
                    Tab::Scrape => ui::scrape::render(ui, &mut self.state),
                    Tab::Scripting => ui::scripting::render(ui, &mut self.state),
                    Tab::Settings => ui::config_panel::render(ui, &mut self.state),
                    Tab::Logs => ui::log_panel::render(ui, &mut self.state),
                    _ => { ui.label("Panel not implemented."); }
                });
        });

        // --- MDI WORKSPACE WINDOWS ---
        // Decouple workspace info from mut borrow of self.state.workspaces
        let active_workspaces: Vec<(String, String, bool, bool, bool, bool, bool)> = self.state.workspaces.iter().map(|(id, ws)| {
            (id.clone(), ws.title.clone(), ws.show_network, ws.show_media, ws.show_storage, ws.show_automation, ws.show_console)
        }).collect();

        for (tid, title, show_net, show_med, show_stor, show_auto, show_cons) in active_workspaces {
            if show_net {
                let mut open = true;
                egui::Window::new(format!("Network - {}", title)).open(&mut open).id(egui::Id::new(format!("net_{}", tid))).show(ctx, |ui| {
                    ui::network_panel::render(ui, &mut self.state);
                });
                if !open { if let Some(ws) = self.state.workspaces.get_mut(&tid) { ws.show_network = false; } }
            }
            if show_med {
                let mut open = true;
                egui::Window::new(format!("Media - {}", title)).open(&mut open).id(egui::Id::new(format!("med_{}", tid))).show(ctx, |ui| {
                    ui::media_panel::render(ui, &mut self.state);
                });
                if !open { if let Some(ws) = self.state.workspaces.get_mut(&tid) { ws.show_media = false; } }
            }
            if show_stor {
                let mut open = true;
                egui::Window::new(format!("Cookies - {}", title)).open(&mut open).id(egui::Id::new(format!("stor_{}", tid))).show(ctx, |ui| {
                    ui::storage_panel::render(ui, &mut self.state);
                });
                if !open { if let Some(ws) = self.state.workspaces.get_mut(&tid) { ws.show_storage = false; } }
            }
            if show_auto {
                let mut open = true;
                egui::Window::new(format!("Automation - {}", title)).open(&mut open).id(egui::Id::new(format!("auto_{}", tid))).resizable(true).show(ctx, |ui| {
                    ui::automation::render_embedded(ui, &mut self.state, &tid);
                });
                if !open { if let Some(ws) = self.state.workspaces.get_mut(&tid) { ws.show_automation = false; } }
            }
            if show_cons {
                let mut open = true;
                egui::Window::new(format!("Console - {}", title)).open(&mut open).id(egui::Id::new(format!("cons_{}", tid))).show(ctx, |ui| {
                    let (mut script, res, logs) = {
                        let ws = self.state.workspaces.get(&tid).unwrap();
                        (ws.js_script.clone(), ws.js_result.clone(), ws.console_logs.clone())
                    };
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.heading(RichText::new("JS CONSOLE").color(Color32::LIGHT_BLUE));
                            if ui.button("📂 LOAD JS").clicked() {
                                if let Some(path) = rfd::FileDialog::new().add_filter("JavaScript", &["js"]).pick_file() {
                                    if let Ok(c) = std::fs::read_to_string(path) { script = c; }
                                }
                            }
                            if ui.button("🗑 CLEAR").clicked() { if let Some(ws) = self.state.workspaces.get_mut(&tid) { ws.console_logs.clear(); } }
                        });
                        ui.add(egui::TextEdit::multiline(&mut script).font(egui::FontId::monospace(12.0)).desired_rows(5).desired_width(f32::INFINITY));
                        if ui.button(RichText::new("▶ EXECUTE SCRIPT").strong()).clicked() {
                            ui::scrape::emit(AppEvent::RequestScriptExecution(tid.clone(), script.clone()));
                        }
                        if !res.is_empty() { ui.label(RichText::new(format!("Result: {}", res)).color(Color32::GREEN).monospace()); }
                        ui.separator();
                        egui::ScrollArea::vertical().stick_to_bottom(true).max_height(200.0).show(ui, |ui| {
                            for log in logs { ui.label(RichText::new(format!("> {}", log)).monospace().size(11.0)); }
                        });
                    });
                    if let Some(ws) = self.state.workspaces.get_mut(&tid) { ws.js_script = script; }
                });
                if !open { if let Some(ws) = self.state.workspaces.get_mut(&tid) { ws.show_console = false; } }
            }
        }

        // Notification Overlay
        let notifications = self
            .state
            .notifications
            .iter()
            .map(|n| (n.id, n.level, n.title.clone(), n.message.clone()))
            .collect::<Vec<_>>();
        for (idx, (id, level, title, message)) in notifications.into_iter().enumerate() {
            let mut open = true;
            let bg = match level {
                NotificationLevel::Ok => Color32::from_rgb(23, 56, 34),
                NotificationLevel::Error => Color32::from_rgb(62, 29, 29),
                NotificationLevel::Warn => Color32::from_rgb(61, 48, 24),
                NotificationLevel::Info => Color32::from_rgb(27, 44, 61),
            };
            egui::Window::new(title)
                .open(&mut open)
                .anchor(egui::Align2::RIGHT_BOTTOM, [-20.0, -20.0 - (idx as f32 * 100.0)])
                .collapsible(false)
                .resizable(false)
                .frame(egui::Frame::window(&ctx.style()).fill(bg))
                .show(ctx, |ui| {
                    ui.label(message);
                });
            if !open {
                self.state.dismiss_notification(id);
            }
        }
        ctx.request_repaint();
    }
}
