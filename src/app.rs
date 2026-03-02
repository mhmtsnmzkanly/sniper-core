use crate::state::{AppState, Tab, TabWorkspace};
use crate::core::events::AppEvent;
use crate::ui;
use eframe::egui::{self, RichText, Color32};
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
        #[cfg(unix)] { unsafe { libc::kill(-(pid as i32), libc::SIGTERM); } }
        #[cfg(windows)] { let _ = std::process::Command::new("taskkill").arg("/F").arg("/T").arg("/PID").arg(pid.to_string()).spawn(); }
        let _ = child.kill();
    }

    pub fn new(_cc: &eframe::CreationContext<'_>, state: AppState, log_receiver: tokio::sync::mpsc::UnboundedReceiver<crate::state::LogEntry>, event_receiver: tokio::sync::mpsc::UnboundedReceiver<AppEvent>) -> Self {
        Self { state, log_receiver, event_receiver, browser_process: Arc::new(Mutex::new(None)), browser_handle: Arc::new(Mutex::new(None)) }
    }

    fn ensure_workspace(&mut self, tid: &str) -> &mut TabWorkspace {
        if !self.state.workspaces.contains_key(tid) {
            let title = self.state.available_tabs.iter().find(|t| t.id == tid).map(|t| t.title.clone()).unwrap_or_else(|| format!("Tab {}", &tid[..8]));
            self.state.workspaces.insert(tid.to_string(), TabWorkspace::new(tid.to_string(), title));
        }
        self.state.workspaces.get_mut(tid).unwrap()
    }
}

impl Drop for CrawlerApp {
    fn drop(&mut self) {
        let mut lock = self.browser_process.lock().unwrap();
        if let Some(mut child) = lock.take() { Self::kill_browser_group(&mut child); }
    }
}

impl eframe::App for CrawlerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let current_time = ctx.input(|i| i.time);

        // --- STARTUP STAGES ---
        if !self.state.output_confirmed {
            egui::Window::new("Sniper Studio Setup 1/2").anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Unified Output Directory").strong());
                    if ui.button("✅ USE DEFAULT").clicked() {
                        self.state.output_confirmed = true;
                        crate::logger::set_log_path(self.state.config.output_dir.join("logs"));
                    }
                    if ui.button("📁 CHOOSE FOLDER").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.state.config.output_dir = path;
                            self.state.output_confirmed = true;
                            crate::logger::set_log_path(self.state.config.output_dir.join("logs"));
                        }
                    }
                });
            });
            return;
        }

        if !self.state.profile_confirmed {
            egui::Window::new("Sniper Studio Setup 2/2").anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Browser Identity").strong());
                    if ui.button("👤 SYSTEM PROFILE").clicked() {
                        self.state.config.default_profile_dir = crate::core::browser::BrowserManager::get_system_profile_path();
                        self.state.profile_confirmed = true;
                    }
                    if ui.button("🆕 ISOLATED PROFILE").clicked() { self.state.profile_confirmed = true; }
                });
            });
            return;
        }

        // --- ASYNC BUFFERS & AUTO-TASKS ---
        while let Ok(log) = self.log_receiver.try_recv() { self.state.logs.push(log); }

        if self.state.is_browser_running && (current_time - self.state.last_tab_refresh) > 2.0 {
            self.state.last_tab_refresh = current_time;
            let port = self.state.config.remote_debug_port;
            tokio::spawn(async move {
                match crate::core::browser::BrowserManager::list_tabs(port).await {
                    Ok(tabs) => ui::scrape::emit(AppEvent::TabsUpdated(tabs)),
                    Err(e) => tracing::debug!("[BROWSER <-> LIST] Could not reach debug port: {}", e),
                }
            });
        }

        // --- EVENT ROUTING ---
        while let Ok(event) = self.event_receiver.try_recv() {
            match event {
                AppEvent::TabsUpdated(tabs) => { self.state.available_tabs = tabs; }
                AppEvent::BrowserStarted(child) => {
                    let mut lock = self.browser_process.lock().unwrap();
                    *lock = Some(child);
                    self.state.is_browser_running = true;
                    tracing::info!("[BROWSER <-> STATUS] Instance Online.");
                }
                AppEvent::BrowserTerminated | AppEvent::TerminateBrowser => {
                    let mut lock = self.browser_process.lock().unwrap();
                    if let Some(mut child) = lock.take() { Self::kill_browser_group(&mut child); }
                    self.state.is_browser_running = false;
                    self.state.available_tabs.clear();
                    self.state.selected_tab_id = None;
                    self.state.workspaces.clear();
                    tracing::warn!("[BROWSER <-> STATUS] Instance Terminated.");
                }
                AppEvent::MediaCaptured(tid, asset) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.media_assets.push(asset);
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
                AppEvent::ConsoleLogAdded(tid, msg) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.console_logs.push(msg);
                }
                AppEvent::RequestPageReload(tid) => {
                    let port = self.state.config.remote_debug_port;
                    tokio::spawn(async move {
                        let _ = crate::core::browser::BrowserManager::reload_page(port, tid).await;
                    });
                }
                AppEvent::RequestUrlBlock(tid, pattern) => {
                    let port = self.state.config.remote_debug_port;
                    let ws = self.ensure_workspace(&tid);
                    ws.blocked_urls.insert(pattern);
                    let tid_clone = tid.clone();
                    let blocked = ws.blocked_urls.iter().cloned().collect::<Vec<_>>();
                    tokio::spawn(async move {
                        let _ = crate::core::browser::BrowserManager::set_url_blocking(port, tid_clone, blocked).await;
                    });
                }
                AppEvent::RequestUrlUnblock(tid, pattern) => {
                    let port = self.state.config.remote_debug_port;
                    let ws = self.ensure_workspace(&tid);
                    ws.blocked_urls.remove(&pattern);
                    let tid_clone = tid.clone();
                    let blocked = ws.blocked_urls.iter().cloned().collect::<Vec<_>>();
                    tokio::spawn(async move {
                        let _ = crate::core::browser::BrowserManager::set_url_blocking(port, tid_clone, blocked).await;
                    });
                }
                AppEvent::RequestScriptExecution(tid, script) => {
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::execute_script(port, tid_clone.clone(), script).await {
                            Ok(res) => ui::scrape::emit(AppEvent::ScriptFinished(tid_clone, res)),
                            Err(e) => ui::scrape::emit(AppEvent::OperationError(e.to_string())),
                        }
                    });
                }
                AppEvent::ScriptFinished(tid, res) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.js_result = res;
                }
                AppEvent::RequestCapture(tid, mirror) => {
                    let port = self.state.config.remote_debug_port;
                    let root = self.state.config.output_dir.clone();
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::capture_html(port, tid, root, mirror).await {
                            Ok(p) => ui::scrape::emit(AppEvent::OperationSuccess(format!("Saved: {:?}", p))),
                            Err(e) => {
                                tracing::error!("[COMMAND <-> ERROR] Capture failed: {}", e);
                                ui::scrape::emit(AppEvent::OperationError(e.to_string()));
                            }
                        }
                    });
                }
                AppEvent::RequestCookies(tid) => {
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::get_cookies(port, tid_clone.clone()).await {
                            Ok(cookies) => {
                                tracing::info!("[STORAGE <-> DATA] Received {} cookies.", cookies.len());
                                ui::scrape::emit(AppEvent::CookiesReceived(tid_clone, cookies));
                            },
                            Err(e) => {
                                tracing::error!("[STORAGE <-> ERROR] Cookie fetch failed: {}", e);
                                ui::scrape::emit(AppEvent::OperationError(e.to_string()));
                            }
                        }
                    });
                }
                AppEvent::RequestCookieDelete(tid, name, domain) => {
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::delete_cookie(port, tid_clone.clone(), name, domain).await {
                            Ok(_) => ui::scrape::emit(AppEvent::RequestCookies(tid_clone)),
                            Err(e) => ui::scrape::emit(AppEvent::OperationError(e.to_string())),
                        }
                    });
                }
                AppEvent::RequestCookieAdd(tid, cookie) => {
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::add_cookie(port, tid_clone.clone(), cookie).await {
                            Ok(_) => ui::scrape::emit(AppEvent::RequestCookies(tid_clone)),
                            Err(e) => ui::scrape::emit(AppEvent::OperationError(e.to_string())),
                        }
                    });
                }
                AppEvent::CookiesReceived(tid, cookies) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.cookies = cookies;
                }
                AppEvent::RequestNetworkToggle(tid, _) => {
                    let port = self.state.config.remote_debug_port;
                    tokio::spawn(async move {
                        if let Err(e) = crate::core::browser::BrowserManager::setup_tab_listeners(port, tid).await {
                            tracing::error!("[CDP <-> ERROR] Listener failed: {}", e);
                        }
                    });
                }
                AppEvent::OperationSuccess(msg) => { self.state.notify("Success", &msg, false); }
                AppEvent::OperationError(msg) => { self.state.notify("Error", &msg, true); }
                _ => {}
            }
        }

        // --- UI DRAWING ---
        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading(RichText::new("SNIPER STUDIO").strong().color(Color32::KHAKI));
            ui.add_space(15.0);
            ui.label("CORE TOOLS");
            ui.selectable_value(&mut self.state.active_tab, Tab::Scrape, " 🎯 SCRAPE");
            ui.selectable_value(&mut self.state.active_tab, Tab::Automation, " 🤖 AUTOMATION");
            ui.add_space(15.0);
            ui.label("SYSTEM");
            ui.selectable_value(&mut self.state.active_tab, Tab::Settings, " ⚙ SETTINGS");
            
            ui.add_space(20.0);
            ui.label(RichText::new("ACTIVE INSPECTORS").small().color(Color32::LIGHT_BLUE));
            let open_ws: Vec<(String, String, bool, bool, bool)> = self.state.workspaces.iter()
                .map(|(id, ws)| (id.clone(), ws.title.clone(), ws.show_network, ws.show_media, ws.show_storage))
                .collect();
            for (_tid, title, sn, sm, ss) in open_ws {
                let trunc: String = title.chars().take(10).collect();
                if sn && ui.selectable_label(true, format!(" 🌐 Net: {}", trunc)).clicked() { }
                if sm && ui.selectable_label(true, format!(" 🖼 Med: {}", trunc)).clicked() { }
                if ss && ui.selectable_label(true, format!(" 📦 Sto: {}", ss)).clicked() { }
            }
        });

        // --- WINDOWS (MDI) ---
        let workspace_ids: Vec<String> = self.state.workspaces.keys().cloned().collect();
        for tid in workspace_ids {
            let (mut show_net, mut show_med, mut show_sto, title) = {
                let ws = self.state.workspaces.get(&tid).unwrap();
                (ws.show_network, ws.show_media, ws.show_storage, ws.title.clone())
            };

            if show_net {
                egui::Window::new(format!("{} - NETWORK", title)).id(egui::Id::new(format!("{}_net", tid))).open(&mut show_net).show(ctx, |ui| {
                    state_bridge_render_network(ui, &mut self.state, &tid);
                });
            }
            if show_med {
                egui::Window::new(format!("{} - MEDIA", title)).id(egui::Id::new(format!("{}_med", tid))).open(&mut show_med).show(ctx, |ui| {
                    state_bridge_render_media(ui, &mut self.state, &tid);
                });
            }
            if show_sto {
                egui::Window::new(format!("{} - STORAGE", title)).id(egui::Id::new(format!("{}_sto", tid))).open(&mut show_sto).show(ctx, |ui| {
                    state_bridge_render_storage(ui, &mut self.state, &tid);
                });
            }

            if let Some(ws) = self.state.workspaces.get_mut(&tid) {
                ws.show_network = show_net; ws.show_media = show_med; ws.show_storage = show_sto;
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.state.active_tab {
                Tab::Scrape => ui::scrape::render(ui, &mut self.state),
                Tab::Automation => ui::automation::render(ui, &mut self.state),
                Tab::Settings => ui::config_panel::render(ui, &mut self.state),
                _ => {}
            }
        });

        egui::TopBottomPanel::bottom("log_panel").resizable(true).default_height(150.0).show(ctx, |ui| {
            ui::log_panel::render(ui, &mut self.state);
        });

        if let Some(notif) = &self.state.notification {
            let mut close = false;
            egui::Window::new(&notif.title).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
                ui.label(&notif.message);
                if ui.button("OK").clicked() { close = true; }
            });
            if close { self.state.notification = None; }
        }

        ctx.request_repaint();
    }
}

fn state_bridge_render_network(ui: &mut egui::Ui, state: &mut AppState, tid: &str) {
    let old_id = state.selected_tab_id.clone();
    state.selected_tab_id = Some(tid.to_string());
    ui::network_panel::render(ui, state);
    state.selected_tab_id = old_id;
}
fn state_bridge_render_media(ui: &mut egui::Ui, state: &mut AppState, tid: &str) {
    let old_id = state.selected_tab_id.clone();
    state.selected_tab_id = Some(tid.to_string());
    ui::media_panel::render(ui, state);
    state.selected_tab_id = old_id;
}
fn state_bridge_render_storage(ui: &mut egui::Ui, state: &mut AppState, tid: &str) {
    let old_id = state.selected_tab_id.clone();
    state.selected_tab_id = Some(tid.to_string());
    ui::storage_panel::render(ui, state);
    state.selected_tab_id = old_id;
}
