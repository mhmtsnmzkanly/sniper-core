use crate::state::{AppState, Tab};
use crate::core::events::AppEvent;
use crate::ui;
use eframe::egui::{self, RichText};
use std::sync::{Arc, Mutex};
use chromiumoxide::Browser;
use futures::StreamExt;

pub struct CrawlerApp {
    pub state: AppState,
    pub log_receiver: tokio::sync::mpsc::UnboundedReceiver<crate::state::LogEntry>,
    pub event_receiver: tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    pub browser_process: Arc<Mutex<Option<std::process::Child>>>,
    pub browser_handle: Arc<Mutex<Option<Browser>>>,
}

impl CrawlerApp {
    pub fn kill_browser_group(child: &mut std::process::Child) {
        let pid = child.id();
        #[cfg(unix)]
        {
            unsafe { libc::kill(-(pid as i32), libc::SIGTERM); }
        }
        #[cfg(windows)]
        {
            let _ = std::process::Command::new("taskkill").arg("/F").arg("/T").arg("/PID").arg(pid.to_string()).spawn();
        }
        let _ = child.kill();
    }

    pub fn new(
        _cc: &eframe::CreationContext<'_>, 
        state: AppState, 
        log_receiver: tokio::sync::mpsc::UnboundedReceiver<crate::state::LogEntry>,
        event_receiver: tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    ) -> Self {
        Self { 
            state, 
            log_receiver, 
            event_receiver,
            browser_process: Arc::new(Mutex::new(None)),
            browser_handle: Arc::new(Mutex::new(None)),
        }
    }
}

impl Drop for CrawlerApp {
    fn drop(&mut self) {
        let mut lock = self.browser_process.lock().unwrap();
        if let Some(mut child) = lock.take() {
            Self::kill_browser_group(&mut child);
        }
    }
}

impl eframe::App for CrawlerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let current_time = ctx.input(|i| i.time);
        
        if self.state.is_browser_running && (current_time - self.state.last_tab_refresh) > 2.0 {
            self.state.last_tab_refresh = current_time;
            let port = self.state.config.remote_debug_port;
            tokio::spawn(async move {
                if let Ok(tabs) = crate::core::browser::BrowserManager::list_tabs(port).await {
                    ui::scrape::emit(AppEvent::TabsUpdated(tabs));
                }
            });
        }

        while let Ok(log) = self.log_receiver.try_recv() {
            self.state.logs.push(log);
        }

        while let Ok(event) = self.event_receiver.try_recv() {
            match event {
                AppEvent::TabsUpdated(tabs) => { self.state.available_tabs = tabs; }
                AppEvent::BrowserStarted(child) => {
                    let mut lock = self.browser_process.lock().unwrap();
                    *lock = Some(child);
                    self.state.is_browser_running = true;
                    
                    // Persistent connection setup
                    let port = self.state.config.remote_debug_port;
                    let handle_store = Arc::clone(&self.browser_handle);
                    tokio::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        if let Ok(ws_url) = crate::core::browser::BrowserManager::get_ws_url(port).await {
                            if let Ok((browser, mut handler)) = Browser::connect(ws_url).await {
                                tokio::spawn(async move {
                                    while let Some(h) = handler.next().await {
                                        if h.is_err() { break; }
                                    }
                                });
                                let mut handle_lock = handle_store.lock().unwrap();
                                *handle_lock = Some(browser);
                                tracing::info!("Persistent browser connection established.");
                            }
                        }
                    });
                }
                AppEvent::BrowserTerminated | AppEvent::TerminateBrowser => {
                    {
                        let mut lock = self.browser_process.lock().unwrap();
                        if let Some(mut child) = lock.take() { Self::kill_browser_group(&mut child); }
                    }
                    {
                        let mut handle_lock = self.browser_handle.lock().unwrap();
                        *handle_lock = None;
                    }
                    self.state.is_browser_running = false;
                    self.state.available_tabs.clear();
                    self.state.selected_tab_id = None;
                    tracing::warn!("Browser terminated.");
                }
                AppEvent::RequestCapture(tab_id, mirror) => {
                    let port = self.state.config.remote_debug_port;
                    let root = self.state.config.raw_output_dir.clone();
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::capture_html(port, tab_id, root, mirror).await {
                            Ok(p) => {
                                tracing::info!("✅ Capture finished successfully: {:?}", p);
                                ui::scrape::emit(AppEvent::OperationSuccess(format!("Page saved to: {:?}", p)));
                            }
                            Err(e) => {
                                tracing::error!("❌ Capture failed: {}", e);
                                ui::scrape::emit(AppEvent::OperationError(format!("Capture failed: {}", e)));
                            }
                        }
                    });
                }
                AppEvent::RequestScriptExecution(tab_id, script) => {
                    let port = self.state.config.remote_debug_port;
                    tokio::spawn(async move {
                        if let Ok(res) = crate::core::browser::BrowserManager::execute_script(port, tab_id, script).await {
                            ui::scrape::emit(AppEvent::ScriptFinished(res));
                        }
                    });
                }
                AppEvent::ScriptFinished(res) => {
                    self.state.js_result = res;
                    self.state.js_execution_active = false;
                    tracing::info!("Script execution finished.");
                }
                AppEvent::RequestNetworkToggle(tab_id, enabled) => {
                    if enabled {
                        let port = self.state.config.remote_debug_port;
                        tokio::spawn(async move {
                            let _ = crate::core::browser::BrowserManager::enable_network_monitoring(port, tab_id).await;
                        });
                    }
                }
                AppEvent::NetworkRequestSent(req) => { self.state.network_requests.push(req); }
                AppEvent::NetworkResponseReceived(id, status) => {
                    if let Some(req) = self.state.network_requests.iter_mut().find(|r| r.request_id == id) {
                        req.status = Some(status);
                    }
                }
                AppEvent::RequestAutomationRun(tab_id, steps) => {
                    let port = self.state.config.remote_debug_port;
                    tokio::spawn(async move {
                        if let Err(e) = crate::core::automation::AutomationEngine::run_pipeline(port, tab_id, steps).await {
                            ui::scrape::emit(AppEvent::AutomationError(e.to_string()));
                        } else {
                            ui::scrape::emit(AppEvent::AutomationFinished);
                        }
                    });
                }
                AppEvent::AutomationProgress(idx) => { self.state.auto_status = crate::state::AutomationStatus::Running(idx); }
                AppEvent::AutomationFinished => { 
                    self.state.auto_status = crate::state::AutomationStatus::Finished; 
                    self.state.notify("Automation Success", "Pipeline finished all steps.", false);
                }
                AppEvent::AutomationError(msg) => { 
                    self.state.auto_status = crate::state::AutomationStatus::Error(msg.clone()); 
                    self.state.notify("Automation Failed", &msg, true);
                }
                
                AppEvent::RequestCookies(tab_id) => {
                    let port = self.state.config.remote_debug_port;
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::get_cookies(port, tab_id).await {
                            Ok(cookies) => ui::scrape::emit(AppEvent::CookiesReceived(cookies)),
                            Err(e) => {
                                tracing::error!("Cookie fetch failed: {}", e);
                                ui::scrape::emit(AppEvent::OperationError(format!("Cookie fetch failed: {}", e)));
                            }
                        }
                    });
                }
                AppEvent::CookiesReceived(cookies) => {
                    self.state.cookies = cookies;
                    tracing::info!("Successfully fetched {} cookies.", self.state.cookies.len());
                    self.state.notify("Storage", "Cookies fetched successfully.", false);
                }
                AppEvent::OperationSuccess(msg) => {
                    self.state.notify("Operation Successful", &msg, false);
                }
                AppEvent::OperationError(msg) => {
                    self.state.notify("Operation Error", &msg, true);
                }
                _ => {}
            }
        }

        // --- UI RENDERING ---
        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading("STUDIO");
            ui.add_space(10.0);
            ui.selectable_value(&mut self.state.active_tab, Tab::Scrape, "SCRAPE");
            ui.selectable_value(&mut self.state.active_tab, Tab::Automation, "AUTOMATION");
            ui.selectable_value(&mut self.state.active_tab, Tab::Network, "NETWORK");
            ui.selectable_value(&mut self.state.active_tab, Tab::Storage, "STORAGE");
            ui.selectable_value(&mut self.state.active_tab, Tab::Translate, "TRANSLATE");
            ui.selectable_value(&mut self.state.active_tab, Tab::Settings, "SETTINGS");
        });

        egui::TopBottomPanel::bottom("log_panel").resizable(true).show(ctx, |ui| {
            ui::log_panel::render(ui, &mut self.state);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.state.active_tab {
                Tab::Scrape => ui::scrape::render(ui, &mut self.state),
                Tab::Automation => ui::automation::render(ui, &mut self.state),
                Tab::Network => ui::network_panel::render(ui, &mut self.state),
                Tab::Storage => ui::storage_panel::render(ui, &mut self.state),
                Tab::Translate => ui::translate::render(ui, &mut self.state),
                Tab::Settings => ui::config_panel::render(ui, &mut self.state),
            }
        });

        // --- NOTIFICATION MODAL ---
        let mut close_notification = false;
        if let Some(notif) = &self.state.notification {
            let title = notif.title.clone();
            let msg = notif.message.clone();
            let is_error = notif.is_error;

            egui::Window::new(RichText::new(&title).strong())
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        let color = if is_error { egui::Color32::RED } else { egui::Color32::GREEN };
                        ui.label(RichText::new(&msg).color(color));
                        ui.add_space(10.0);
                        if ui.button("OK").clicked() {
                            close_notification = true;
                        }
                    });
                });
        }
        
        if close_notification {
            self.state.notification = None;
        }

        ctx.request_repaint();
    }
}
