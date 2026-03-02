use crate::state::{AppState, Tab};
use crate::core::events::AppEvent;
use crate::ui;
use eframe::egui::{self, RichText};
use std::sync::{Arc, Mutex};
use chromiumoxide::Browser;

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

        // --- 1. STARTUP SPLASH ---
        if !self.state.profile_confirmed {
            egui::Window::new("Sniper Scraper Studio 1.1.0 - Startup")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("Choose Browser Profile Mode").strong().size(18.0));
                        ui.add_space(10.0);
                        
                        if ui.button("👤 Use My Real System Profile (Chrome/Chromium)").clicked() {
                            self.state.use_custom_profile = true;
                            self.state.profile_confirmed = true;
                            tracing::info!("Startup: User selected real system profile.");
                        }
                        
                        ui.add_space(5.0);
                        
                        if ui.button("🆕 Create New Isolated Profile (Temp)").clicked() {
                            self.state.use_custom_profile = false;
                            self.state.profile_confirmed = true;
                            tracing::info!("Startup: User selected isolated profile.");
                        }
                    });
                });
            return;
        }

        // --- 2. AUTOMATIC TASKS ---
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

        // --- 3. EVENT HANDLING ---
        while let Ok(event) = self.event_receiver.try_recv() {
            let tab_name = self.state.get_selected_tab_name();
            match event {
                AppEvent::TabsUpdated(tabs) => { self.state.available_tabs = tabs; }
                AppEvent::BrowserStarted(child) => {
                    let mut lock = self.browser_process.lock().unwrap();
                    *lock = Some(child);
                    self.state.is_browser_running = true;
                    tracing::info!("SCRAPER <-> Browser Launched on port {}", self.state.config.remote_debug_port);
                }
                AppEvent::BrowserTerminated | AppEvent::TerminateBrowser => {
                    let mut lock = self.browser_process.lock().unwrap();
                    if let Some(mut child) = lock.take() { Self::kill_browser_group(&mut child); }
                    self.state.is_browser_running = false;
                    self.state.available_tabs.clear();
                    tracing::warn!("SCRAPER <-> Browser Terminated.");
                }
                AppEvent::ConsoleLogAdded(msg) => {
                    self.state.console_logs.push(msg);
                    if self.state.console_logs.len() > 100 { self.state.console_logs.remove(0); }
                }
                AppEvent::RequestCapture(tab_id, mirror) => {
                    let port = self.state.config.remote_debug_port;
                    let root = self.state.config.raw_output_dir.clone();
                    tracing::info!("SCRAPER <-> Capture started for [{}]", tab_name);
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::capture_html(port, tab_id, root, mirror).await {
                            Ok(p) => {
                                tracing::info!("SCRAPER <-> Capture finished: {:?}", p);
                                ui::scrape::emit(AppEvent::OperationSuccess(format!("Saved: {:?}", p)));
                            }
                            Err(e) => {
                                tracing::error!("SCRAPER <-> Capture failed: {}", e);
                                ui::scrape::emit(AppEvent::OperationError(e.to_string()));
                            }
                        }
                    });
                }
                AppEvent::RequestScriptExecution(tab_id, script) => {
                    let port = self.state.config.remote_debug_port;
                    tracing::info!("AUTOMATION <-> Injecting script to [{}]", tab_name);
                    tokio::spawn(async move {
                        if let Ok(res) = crate::core::browser::BrowserManager::execute_script(port, tab_id, script).await {
                            ui::scrape::emit(AppEvent::ScriptFinished(res));
                        }
                    });
                }
                AppEvent::ScriptFinished(res) => {
                    self.state.js_result = res;
                    self.state.js_execution_active = false;
                    tracing::info!("AUTOMATION <-> Script result received.");
                }
                AppEvent::RequestNetworkToggle(tab_id, enabled) => {
                    if enabled {
                        let port = self.state.config.remote_debug_port;
                        tracing::info!("NETWORK <-> Monitoring enabled for [{}]", tab_name);
                        tokio::spawn(async move {
                            let _ = crate::core::browser::BrowserManager::setup_tab_listeners(port, tab_id).await;
                        });
                    }
                }
                AppEvent::NetworkRequestSent(req) => { self.state.network_requests.push(req); }
                AppEvent::NetworkResponseReceived(id, status, body) => {
                    if let Some(req) = self.state.network_requests.iter_mut().find(|r| r.request_id == id) {
                        req.status = Some(status);
                        req.response_body = body;
                    }
                }
                AppEvent::RequestCookies(tab_id) => {
                    let port = self.state.config.remote_debug_port;
                    tracing::info!("STORAGE <-> Fetching cookies from [{}]", tab_name);
                    tokio::spawn(async move {
                        if let Ok(cookies) = crate::core::browser::BrowserManager::get_cookies(port, tab_id).await {
                            ui::scrape::emit(AppEvent::CookiesReceived(cookies));
                        }
                    });
                }
                AppEvent::CookiesReceived(cookies) => {
                    self.state.cookies = cookies;
                    self.state.notify("Storage", "Cookies updated.", false);
                }
                AppEvent::OperationSuccess(msg) => { self.state.notify("Success", &msg, false); }
                AppEvent::OperationError(msg) => { self.state.notify("Error", &msg, true); }
                AppEvent::RequestCookieDelete(tab_id, name, domain) => {
                    let port = self.state.config.remote_debug_port;
                    let mut cookie = crate::state::ChromeCookie::default();
                    cookie.name = name;
                    cookie.domain = domain;
                    tokio::spawn(async move {
                        if let Ok(_) = crate::core::browser::BrowserManager::manage_cookie(port, tab_id.clone(), cookie, true).await {
                            ui::scrape::emit(AppEvent::RequestCookies(tab_id));
                        }
                    });
                }
                AppEvent::RequestCookieAdd(tab_id, cookie) => {
                    let port = self.state.config.remote_debug_port;
                    tokio::spawn(async move {
                        if let Ok(_) = crate::core::browser::BrowserManager::manage_cookie(port, tab_id.clone(), cookie, false).await {
                            ui::scrape::emit(AppEvent::RequestCookies(tab_id));
                        }
                    });
                }
                _ => {}
            }
        }

        // --- 4. UI DRAWING ---
        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading("STUDIO 1.1.0");
            ui.add_space(10.0);
            ui.selectable_value(&mut self.state.active_tab, Tab::Scrape, "SCRAPE");
            ui.selectable_value(&mut self.state.active_tab, Tab::Automation, "AUTOMATION");
            ui.selectable_value(&mut self.state.active_tab, Tab::Network, "NETWORK");
            ui.selectable_value(&mut self.state.active_tab, Tab::Storage, "STORAGE");
            ui.selectable_value(&mut self.state.active_tab, Tab::Translate, "TRANSLATE");
            ui.selectable_value(&mut self.state.active_tab, Tab::Settings, "SETTINGS");
        });

        egui::TopBottomPanel::bottom("log_panel").resizable(true).default_height(300.0).show(ctx, |ui| {
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

        // Notifications Modal
        let mut close_notification = false;
        if let Some(notif) = &self.state.notification {
            egui::Window::new(RichText::new(&notif.title).strong())
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new(&notif.message).color(if notif.is_error { egui::Color32::RED } else { egui::Color32::GREEN }));
                        if ui.button("OK").clicked() { close_notification = true; }
                    });
                });
        }
        if close_notification { self.state.notification = None; }

        ctx.request_repaint();
    }
}
