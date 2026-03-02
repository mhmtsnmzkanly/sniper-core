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

        // --- 1. STARTUP SPLASH ---
        if !self.state.profile_confirmed {
            egui::Window::new("Sniper Studio 1.1.0 - Startup")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false).resizable(false).show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("Select Browser Identity").strong().size(18.0));
                        ui.add_space(10.0);
                        if ui.button(RichText::new("👤 USE MY SYSTEM PROFILE (Chrome)").strong()).clicked() {
                            self.state.use_custom_profile = true;
                            self.state.config.default_profile_dir = crate::core::browser::BrowserManager::get_system_profile_path();
                            self.state.profile_confirmed = true;
                            tracing::info!("[SYSTEM <-> INIT] User selected System Profile: {:?}", self.state.config.default_profile_dir);
                        }
                        ui.add_space(5.0);
                        if ui.button("🆕 USE FRESH ISOLATED PROFILE").clicked() {
                            self.state.use_custom_profile = false;
                            self.state.profile_confirmed = true;
                            tracing::info!("[SYSTEM <-> INIT] User selected Isolated Profile.");
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

        while let Ok(log) = self.log_receiver.try_recv() { self.state.logs.push(log); }

        // --- 3. EVENT ROUTING (TAB-AWARE) ---
        while let Ok(event) = self.event_receiver.try_recv() {
            match event {
                AppEvent::TabsUpdated(tabs) => { self.state.available_tabs = tabs; }
                AppEvent::BrowserStarted(child) => {
                    let mut lock = self.browser_process.lock().unwrap();
                    *lock = Some(child);
                    self.state.is_browser_running = true;
                    tracing::info!("[BROWSER <-> STATUS] Instance online.");
                }
                AppEvent::BrowserTerminated | AppEvent::TerminateBrowser => {
                    let mut lock = self.browser_process.lock().unwrap();
                    if let Some(mut child) = lock.take() { Self::kill_browser_group(&mut child); }
                    self.state.is_browser_running = false;
                    self.state.available_tabs.clear();
                    self.state.workspaces.clear();
                    tracing::warn!("[BROWSER <-> STATUS] Instance terminated.");
                }
                AppEvent::MediaCaptured(tid, asset) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.media_assets.push(asset);
                    tracing::info!("[MEDIA <-> DATA] Intercepted asset for tab {}", tid);
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
                        tracing::info!("[NETWORK <-> DATA] Captured Response: {} ({})", req.url, status);
                    }
                }
                AppEvent::ConsoleLogAdded(tid, msg) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.console_logs.push(msg);
                }
                AppEvent::CookiesReceived(tid, cookies) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.cookies = cookies;
                    tracing::info!("[STORAGE <-> DATA] Received {} cookies for tab {}", ws.cookies.len(), tid);
                }
                AppEvent::RequestCapture(tid, mirror) => {
                    let port = self.state.config.remote_debug_port;
                    let root = self.state.config.output_dir.clone();
                    tracing::info!("[COMMAND <-> CAPTURE] Requesting HTML for tab {}", tid);
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::capture_html(port, tid, root, mirror).await {
                            Ok(p) => ui::scrape::emit(AppEvent::OperationSuccess(format!("Captured: {:?}", p))),
                            Err(e) => ui::scrape::emit(AppEvent::OperationError(e.to_string())),
                        }
                    });
                }
                AppEvent::RequestCookies(tid) => {
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    tokio::spawn(async move {
                        if let Ok(cookies) = crate::core::browser::BrowserManager::get_cookies(port, tid_clone.clone()).await {
                            ui::scrape::emit(AppEvent::CookiesReceived(tid_clone, cookies));
                        }
                    });
                }
                AppEvent::OperationSuccess(msg) => { self.state.notify("Success", &msg, false); }
                AppEvent::OperationError(msg) => { self.state.notify("Error", &msg, true); }
                _ => {}
            }
        }

        // --- 4. UI RENDERING ---
        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading(RichText::new("SNIPER STUDIO").strong().color(Color32::KHAKI));
            ui.label(RichText::new("v1.1.0 Stable").small().color(Color32::DARK_GRAY));
            ui.add_space(15.0);

            ui.label("CORE TOOLS");
            ui.selectable_value(&mut self.state.active_tab, Tab::Scrape, " 🎯 SCRAPE");
            ui.selectable_value(&mut self.state.active_tab, Tab::Automation, " 🤖 AUTOMATION");
            ui.selectable_value(&mut self.state.active_tab, Tab::Translate, " 🌎 TRANSLATE");
            ui.add_space(15.0);
            ui.label("SYSTEM");
            ui.selectable_value(&mut self.state.active_tab, Tab::Settings, " ⚙ SETTINGS");
            
            ui.add_space(20.0);
            ui.label(RichText::new("ACTIVE INSPECTORS").small().color(Color32::LIGHT_BLUE));
            
            let inspector_info: Vec<(String, String, bool, bool, bool)> = self.state.workspaces.iter()
                .map(|(id, ws)| (id.clone(), ws.title.clone(), ws.show_network, ws.show_media, ws.show_storage))
                .collect();

            for (tid, title, sn, sm, ss) in inspector_info {
                let short_title: String = title.chars().take(10).collect();
                if sn && ui.selectable_label(true, format!(" 🌐 Net: {}", short_title)).clicked() { /* Focus */ }
                if sm && ui.selectable_label(true, format!(" 🖼 Med: {}", short_title)).clicked() { /* Focus */ }
                if ss && ui.selectable_label(true, format!(" 📦 Sto: {}", short_title)).clicked() { /* Focus */ }
            }
        });

        // --- 5. WORKSPACE WINDOWS (MDI) ---
        let workspace_ids: Vec<String> = self.state.workspaces.keys().cloned().collect();
        for tid in workspace_ids {
            let mut show_net = self.state.workspaces.get(&tid).map(|w| w.show_network).unwrap_or(false);
            if show_net {
                let title = self.state.workspaces.get(&tid).unwrap().title.clone();
                egui::Window::new(format!("{} - NETWORK", title)).id(egui::Id::new(format!("{}_net", tid))).open(&mut show_net).default_size([700.0, 500.0]).show(ctx, |ui| {
                    state_bridge_render_network(ui, &mut self.state, &tid);
                });
                if let Some(ws) = self.state.workspaces.get_mut(&tid) { ws.show_network = show_net; }
            }

            let mut show_med = self.state.workspaces.get(&tid).map(|w| w.show_media).unwrap_or(false);
            if show_med {
                let title = self.state.workspaces.get(&tid).unwrap().title.clone();
                egui::Window::new(format!("{} - MEDIA", title)).id(egui::Id::new(format!("{}_med", tid))).open(&mut show_med).default_size([800.0, 600.0]).show(ctx, |ui| {
                    state_bridge_render_media(ui, &mut self.state, &tid);
                });
                if let Some(ws) = self.state.workspaces.get_mut(&tid) { ws.show_media = show_med; }
            }

            let mut show_sto = self.state.workspaces.get(&tid).map(|w| w.show_storage).unwrap_or(false);
            if show_sto {
                let title = self.state.workspaces.get(&tid).unwrap().title.clone();
                egui::Window::new(format!("{} - STORAGE", title)).id(egui::Id::new(format!("{}_sto", tid))).open(&mut show_sto).default_size([600.0, 400.0]).show(ctx, |ui| {
                    state_bridge_render_storage(ui, &mut self.state, &tid);
                });
                if let Some(ws) = self.state.workspaces.get_mut(&tid) { ws.show_storage = show_sto; }
            }
        }

        egui::TopBottomPanel::bottom("log_panel").resizable(true).default_height(150.0).show(ctx, |ui| {
            ui::log_panel::render(ui, &mut self.state);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.state.active_tab {
                Tab::Scrape => ui::scrape::render(ui, &mut self.state),
                Tab::Automation => ui::automation::render(ui, &mut self.state),
                Tab::Translate => ui::translate::render(ui, &mut self.state),
                Tab::Settings => ui::config_panel::render(ui, &mut self.state),
                _ => {}
            }
        });

        ctx.request_repaint();
    }
}

// Bridges to set the active workspace context for panels
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
