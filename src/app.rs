use crate::core::events::AppEvent;
use crate::state::{AppState, AutomationStatus, Tab, AutomationStep, LogEntry};
use crate::ui;
use eframe::egui;
use egui::{Color32, RichText};
use std::sync::{Arc, Mutex};

pub struct CrawlerApp {
    pub state: AppState,
    pub log_receiver: tokio::sync::mpsc::UnboundedReceiver<LogEntry>,
    pub event_receiver: tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    pub browser_process: Arc<Mutex<Option<std::process::Child>>>,
}

impl CrawlerApp {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        state: AppState,
        log_receiver: tokio::sync::mpsc::UnboundedReceiver<LogEntry>,
        event_receiver: tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    ) -> Self {
        Self {
            state,
            log_receiver,
            event_receiver,
            browser_process: Arc::new(Mutex::new(None)),
        }
    }

    fn ensure_workspace(&mut self, tid: &str) -> &mut crate::state::TabWorkspace {
        if !self.state.workspaces.contains_key(tid) {
            self.state.workspaces.insert(
                tid.to_string(),
                crate::state::TabWorkspace::new(tid.to_string(), "New Tab".into()),
            );
        }
        self.state.workspaces.get_mut(tid).unwrap()
    }
}

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
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(log) = self.log_receiver.try_recv() {
            self.state.logs.push(log);
            if self.state.logs.len() > 1000 { self.state.logs.remove(0); }
        }

        while let Ok(event) = self.event_receiver.try_recv() {
            // Guard for browser-dependent events
            if !self.state.is_browser_running {
                match &event {
                    AppEvent::RequestCookies(_) | AppEvent::RequestPageReload(_) | 
                    AppEvent::RequestScriptExecution(_, _) | AppEvent::RequestAutomationRun(..) |
                    AppEvent::RequestCapture(_, _, _) | AppEvent::RequestPageSelectors(_) |
                    AppEvent::RequestTabRefresh => {
                        let msg = "Action Denied: Browser is not running.";
                        self.state.notify("Denied", msg, true);
                        tracing::warn!("[APP] {}", msg);
                        continue;
                    }
                    _ => {}
                }
            }

            match event {
                AppEvent::RequestLogPathSet(path) => {
                    crate::logger::set_log_path(path, &self.state.session_timestamp);
                }
                AppEvent::BrowserStarted(child) => {
                    *self.browser_process.lock().unwrap() = Some(child);
                    self.state.is_browser_running = true;
                    self.state.notify("System", "Browser instance launched.", false);
                    tracing::info!("[APP] Browser started.");
                }
                AppEvent::BrowserTerminated => {
                    self.state.is_browser_running = false;
                    self.state.available_tabs.clear();
                    self.state.selected_tab_id = None;
                    self.state.notify("System", "Browser instance disconnected.", true);
                    tracing::warn!("[APP] Browser terminated.");
                }
                AppEvent::TerminateBrowser => {
                    if let Some(mut child) = self.browser_process.lock().unwrap().take() {
                        let _ = child.kill();
                        self.state.is_browser_running = false;
                        self.state.available_tabs.clear();
                        self.state.selected_tab_id = None;
                        self.state.notify("System", "Browser instance terminated.", false);
                        tracing::info!("[APP] Browser terminated by user.");
                    }
                }
                AppEvent::TabsUpdated(tabs) => {
                    self.state.available_tabs = tabs;
                }
                AppEvent::ConsoleLogAdded(tid, msg) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.console_logs.push(msg.clone());
                    tracing::debug!("[CONSOLE][{}] {}", tid, msg);
                }
                AppEvent::SelectorsReceived(tid, sels) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.discovered_selectors = sels;
                }
                AppEvent::MediaCaptured(tid, asset) => {
                    let ws = self.ensure_workspace(&tid);
                    if !ws.media_assets.iter().any(|a| a.url == asset.url) {
                        ws.media_assets.push(asset);
                    }
                }
                AppEvent::CookiesReceived(tid, cookies) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.cookies = cookies;
                }
                AppEvent::AutomationProgress(tid, step) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.auto_status = AutomationStatus::Running(step);
                }
                AppEvent::AutomationFinished(tid) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.auto_status = AutomationStatus::Finished;
                    tracing::info!("[APP][{}] Automation finished.", tid);
                }
                AppEvent::AutomationError(tid, err) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.auto_status = AutomationStatus::Error(err.clone());
                    tracing::error!("[APP][{}] Automation error: {}", tid, err);
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
                    self.state.notify("Success", &msg, false);
                    tracing::info!("[OP] {}", msg);
                }
                AppEvent::OperationError(msg) => {
                    self.state.notify("Error", &msg, true);
                    tracing::error!("[OP] {}", msg);
                }
                
                AppEvent::RequestCookies(tid) => {
                    let port = self.state.config.remote_debug_port;
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::get_cookies(port, tid.clone()).await {
                            Ok(cookies) => crate::ui::scrape::emit(AppEvent::CookiesReceived(tid, cookies)),
                            Err(e) => crate::ui::scrape::emit(AppEvent::OperationError(format!("Cookie Error: {}", e))),
                        }
                    });
                }
                AppEvent::RequestCookieDelete(tid, name, domain) => {
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    tokio::spawn(async move {
                        if let Err(e) = crate::core::browser::BrowserManager::delete_cookie(port, tid.clone(), name, domain).await {
                            crate::ui::scrape::emit(AppEvent::OperationError(format!("Delete Error: {}", e)));
                        } else {
                            let _ = crate::core::browser::BrowserManager::get_cookies(port, tid_clone.clone()).await.map(|c| {
                                crate::ui::scrape::emit(AppEvent::CookiesReceived(tid_clone, c));
                            });
                        }
                    });
                }
                AppEvent::RequestCookieAdd(tid, cookie) => {
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    tokio::spawn(async move {
                        if let Err(e) = crate::core::browser::BrowserManager::add_cookie(port, tid.clone(), cookie).await {
                            crate::ui::scrape::emit(AppEvent::OperationError(format!("Add Error: {}", e)));
                        } else {
                            let _ = crate::core::browser::BrowserManager::get_cookies(port, tid_clone.clone()).await.map(|c| {
                                crate::ui::scrape::emit(AppEvent::CookiesReceived(tid_clone, c));
                            });
                        }
                    });
                }
                AppEvent::RequestPageReload(tid) => {
                    let port = self.state.config.remote_debug_port;
                    tokio::spawn(async move {
                        if let Err(e) = crate::core::browser::BrowserManager::reload_page(port, tid).await {
                            crate::ui::scrape::emit(AppEvent::OperationError(format!("Reload Error: {}", e)));
                        }
                    });
                }
                AppEvent::RequestTabRefresh => {
                    let port = self.state.config.remote_debug_port;
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::list_tabs(port).await {
                            Ok(tabs) => crate::ui::scrape::emit(AppEvent::TabsUpdated(tabs)),
                            Err(e) => crate::ui::scrape::emit(AppEvent::OperationError(format!("Refresh Error: {}", e))),
                        }
                    });
                }
                AppEvent::RequestScriptExecution(tid, script) => {
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::execute_script(port, tid, script).await {
                            Ok(res) => crate::ui::scrape::emit(AppEvent::ScriptFinished(tid_clone, res)),
                            Err(e) => crate::ui::scrape::emit(AppEvent::OperationError(format!("Script Error: {}", e))),
                        }
                    });
                }
                AppEvent::ScriptFinished(tid, res) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.js_result = res;
                }
                AppEvent::RequestPageSelectors(tid) => {
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::get_page_selectors(port, tid).await {
                            Ok(sels) => crate::ui::scrape::emit(AppEvent::SelectorsReceived(tid_clone, sels)),
                            Err(e) => crate::ui::scrape::emit(AppEvent::OperationError(format!("Selector Error: {}", e))),
                        }
                    });
                }
                AppEvent::RequestNetworkToggle(_tid, _active) => {}
                AppEvent::RequestUrlBlock(tid, pattern) => {
                    let port = self.state.config.remote_debug_port;
                    let ws = self.ensure_workspace(&tid);
                    ws.blocked_urls.insert(pattern);
                    let blocked = ws.blocked_urls.iter().cloned().collect();
                    tokio::spawn(async move {
                        let _ = crate::core::browser::BrowserManager::set_url_blocking(port, tid, blocked).await;
                    });
                }
                AppEvent::RequestUrlUnblock(tid, pattern) => {
                    let port = self.state.config.remote_debug_port;
                    let ws = self.ensure_workspace(&tid);
                    ws.blocked_urls.remove(&pattern);
                    let blocked = ws.blocked_urls.iter().cloned().collect();
                    tokio::spawn(async move {
                        let _ = crate::core::browser::BrowserManager::set_url_blocking(port, tid, blocked).await;
                    });
                }
                AppEvent::RequestCapture(tid, mode, _) => {
                    let port = self.state.config.remote_debug_port;
                    let root = self.state.config.output_dir.clone();
                    tokio::spawn(async move {
                        let res = match mode.as_str() {
                            "html" => crate::core::browser::BrowserManager::capture_html(port, tid, root).await,
                            "complete" => crate::core::browser::BrowserManager::capture_complete(port, tid, root).await,
                            "mirror" => crate::core::browser::BrowserManager::capture_mirror(port, tid, root).await,
                            _ => Err(crate::core::error::AppError::Internal("Unknown capture mode".into())),
                        };
                        match res {
                            Ok(path) => crate::ui::scrape::emit(AppEvent::OperationSuccess(format!("Captured ({}): {:?}", mode, path))),
                            Err(e) => {
                                tracing::error!("[OP] Capture Error: {}", e);
                                crate::ui::scrape::emit(AppEvent::OperationError(format!("Capture Error: {}", e)));
                            }
                        }
                    });
                }

                AppEvent::RequestAutomationRun(tid, steps, funcs, auto_config) => {
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    let mut dsl_funcs = std::collections::HashMap::new();
                    for (name, f_steps) in funcs { dsl_funcs.insert(name, map_ui_steps_to_dsl(&f_steps)); }
                    let dsl = crate::core::automation::dsl::AutomationDsl {
                        dsl_version: 1, metadata: None, functions: dsl_funcs, steps: map_ui_steps_to_dsl(&steps),
                    };
                    tracing::info!("[USER] Automation pipeline started for tab: {}", tid_clone);
                    let output_dir = self.state.config.output_dir.clone();
                    tokio::spawn(async move {
                        let mut engine = crate::core::automation::engine::AutomationEngine::new(port, tid_clone, output_dir);
                        engine.config = crate::core::automation::engine::ExecutionConfig {
                            step_timeout: std::time::Duration::from_millis(auto_config.step_timeout_ms),
                            retry_attempts: auto_config.retry_attempts,
                            screenshot_on_error: auto_config.screenshot_on_error,
                        };
                        if let Err(e) = engine.run(dsl).await {
                            crate::ui::scrape::emit(AppEvent::OperationError(format!("Automation Failed: {}", e)));
                        }
                    });
                }
            }
        }

        // --- UI RENDER ---
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.state.active_tab, Tab::Scrape, " 🎯 SCRAPE");
                ui.selectable_value(&mut self.state.active_tab, Tab::Settings, " ⚙ SETTINGS");
                ui.selectable_value(&mut self.state.active_tab, Tab::Logs, " 📝 LOGS");
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.state.is_browser_running {
                        ui.label(RichText::new("● BROWSER ACTIVE").color(Color32::GREEN).small());
                    } else {
                        ui.label(RichText::new("○ BROWSER DOWN").color(Color32::RED).small());
                    }
                });
            });
        });

        // Modals for confirmation
        if !self.state.output_confirmed {
            egui::Window::new("Sniper Core - Initial Setup")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false).resizable(false)
                .show(ctx, |ui| {
                    ui.heading("Select Output Directory");
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{:?}", self.state.config.output_dir)).color(Color32::KHAKI));
                        if ui.button("Browse...").clicked() {
                            if let Some(path) = rfd::FileDialog::new().pick_folder() { self.state.config.output_dir = path; }
                        }
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
            egui::Window::new("Browser Profile Setup")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false).resizable(false)
                .show(ctx, |ui| {
                    ui.heading("Select Browser Profile Mode");
                    ui.vertical(|ui| {
                        if ui.selectable_label(self.state.use_custom_profile, "🏠 ISOLATED PROFILE").clicked() { self.state.use_custom_profile = true; }
                        if ui.selectable_label(!self.state.use_custom_profile, "👤 SYSTEM PROFILE").clicked() { self.state.use_custom_profile = false; }
                    });
                    ui.add_space(20.0);
                    if ui.button(RichText::new("CONFIRM & LAUNCH").strong()).clicked() {
                        self.state.profile_confirmed = true;
                    }
                });
            return;
        }

        egui::CentralPanel::default().show(ctx, |ui| match self.state.active_tab {
            Tab::Scrape => ui::scrape::render(ui, &mut self.state),
            Tab::Settings => ui::config_panel::render(ui, &mut self.state),
            Tab::Logs => ui::log_panel::render(ui, &mut self.state),
            _ => { ui.label("Tab not implemented"); }
        });

        // Notification Overlay
        if let Some(notif) = &self.state.notification {
            let mut open = true;
            egui::Window::new(&notif.title)
                .open(&mut open).anchor(egui::Align2::RIGHT_BOTTOM, [-20.0, -20.0])
                .collapsible(false).resizable(false)
                .show(ctx, |ui| { ui.label(&notif.message); });
            if !open { self.state.notification = None; }
        }
        ctx.request_repaint();
    }
}
