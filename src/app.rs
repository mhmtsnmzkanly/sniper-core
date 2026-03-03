use crate::core::events::AppEvent;
use crate::state::{AppState, AutomationStatus, Tab};
use crate::ui;
use eframe::egui;
use egui::{Color32, RichText};
use std::sync::{Arc, Mutex};

pub struct CrawlerApp {
    pub state: AppState,
    pub log_receiver: tokio::sync::mpsc::UnboundedReceiver<crate::state::LogEntry>,
    pub event_receiver: tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    pub browser_process: Arc<Mutex<Option<std::process::Child>>>,
    pub browser_handle: Arc<Mutex<Option<chromiumoxide::Browser>>>,
}

impl CrawlerApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        state: AppState,
        log_receiver: tokio::sync::mpsc::UnboundedReceiver<crate::state::LogEntry>,
        event_receiver: tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    ) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        Self {
            state,
            log_receiver,
            event_receiver,
            browser_process: Arc::new(Mutex::new(None)),
            browser_handle: Arc::new(Mutex::new(None)),
        }
    }

    fn ensure_workspace(&mut self, tid: &str) -> &mut crate::state::TabWorkspace {
        if !self.state.workspaces.contains_key(tid) {
            let title = self
                .state
                .available_tabs
                .iter()
                .find(|t| t.id == tid)
                .map(|t| t.title.clone())
                .unwrap_or_else(|| "Tab".into());
            self.state.workspaces.insert(
                tid.to_string(),
                crate::state::TabWorkspace::new(tid.to_string(), title),
            );
        }
        self.state.workspaces.get_mut(tid).unwrap()
    }
}

impl eframe::App for CrawlerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // --- LOG INGESTION ---
        while let Ok(log) = self.log_receiver.try_recv() {
            self.state.logs.push(log);
            if self.state.logs.len() > 1000 {
                self.state.logs.remove(0);
            }
        }

        // --- EVENT HANDLING ---
        while let Ok(event) = self.event_receiver.try_recv() {
            match event {
                AppEvent::RequestLogPathSet(path) => {
                    crate::logger::set_log_path(path, &self.state.session_timestamp);
                }
                AppEvent::BrowserStarted(child) => {
                    let mut lock = self.browser_process.lock().unwrap();
                    *lock = Some(child);
                    self.state.is_browser_running = true;
                    self.state.notify("System", "Browser instance launched.", false);
                    tracing::info!("[USER] Browser instance launched successfully.");
                }
                AppEvent::BrowserTerminated => {
                    self.state.is_browser_running = false;
                    self.state.notify("System", "Browser instance closed.", false);
                    tracing::info!("[USER] Browser instance terminated externally.");
                }
                AppEvent::TerminateBrowser => {
                    let mut lock = self.browser_process.lock().unwrap();
                    if let Some(mut child) = lock.take() {
                        let _ = child.kill();
                        self.state.is_browser_running = false;
                        self.state.notify("System", "Browser instance terminated.", false);
                        tracing::info!("[USER] Browser instance terminated by user.");
                    }
                }
                AppEvent::TabsUpdated(tabs) => {
                    self.state.available_tabs = tabs;
                }
                AppEvent::ConsoleLogAdded(tid, msg) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.console_logs.push(msg);
                    if ws.console_logs.len() > 500 {
                        ws.console_logs.remove(0);
                    }
                }
                AppEvent::RequestTabRefresh => {
                    let port = self.state.config.remote_debug_port;
                    tokio::spawn(async move {
                        if let Ok(tabs) = crate::core::browser::BrowserManager::list_tabs(port).await
                        {
                            ui::scrape::emit(AppEvent::TabsUpdated(tabs));
                        }
                    });
                }
                AppEvent::RequestPageReload(tid) => {
                    let port = self.state.config.remote_debug_port;
                    tracing::info!("[USER] Page reload requested for tab: {}", tid);
                    tokio::spawn(async move {
                        let _ = crate::core::browser::BrowserManager::reload_page(port, tid).await;
                    });
                }
                AppEvent::RequestNetworkToggle(tid, _enabled) => {
                    let port = self.state.config.remote_debug_port;
                    tokio::spawn(async move {
                        let _ =
                            crate::core::browser::BrowserManager::setup_tab_listeners(port, tid)
                                .await;
                    });
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
                AppEvent::MediaCaptured(tid, asset) => {
                    let ws = self.ensure_workspace(&tid);
                    if !ws.media_assets.iter().any(|a| a.url == asset.url) {
                        ws.media_assets.push(asset);
                    }
                }
                AppEvent::RequestCookies(tid) => {
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    tokio::spawn(async move {
                        if let Ok(cookies) =
                            crate::core::browser::BrowserManager::get_cookies(port, tid_clone.clone())
                                .await
                        {
                            ui::scrape::emit(AppEvent::CookiesReceived(tid_clone, cookies));
                        }
                    });
                }
                AppEvent::CookiesReceived(tid, cookies) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.cookies = cookies;
                }
                AppEvent::RequestCookieDelete(tid, name, domain) => {
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    tracing::info!("[USER] Cookie deletion requested: {} for domain: {}", name, domain);
                    tokio::spawn(async move {
                        if let Ok(_) = crate::core::browser::BrowserManager::delete_cookie(
                            port, tid_clone, name, domain,
                        )
                        .await
                        {
                            ui::scrape::emit(AppEvent::RequestCookies(tid.clone()));
                        }
                    });
                }
                AppEvent::RequestCookieAdd(tid, cookie) => {
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    tracing::info!("[USER] Cookie addition/update requested: {}", cookie.name);
                    tokio::spawn(async move {
                        if let Ok(_) =
                            crate::core::browser::BrowserManager::add_cookie(port, tid_clone, cookie)
                                .await
                        {
                            ui::scrape::emit(AppEvent::RequestCookies(tid.clone()));
                        }
                    });
                }
                AppEvent::RequestPageSelectors(tid) => {
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    tokio::spawn(async move {
                        if let Ok(sels) =
                            crate::core::browser::BrowserManager::get_page_selectors(port, tid_clone.clone())
                                .await
                        {
                            ui::scrape::emit(AppEvent::SelectorsReceived(tid_clone, sels));
                        }
                    });
                }
                AppEvent::SelectorsReceived(tid, sels) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.discovered_selectors = sels;
                }
                AppEvent::RequestUrlBlock(tid, pattern) => {
                    let port = self.state.config.remote_debug_port;
                    let ws = self.ensure_workspace(&tid);
                    ws.blocked_urls.insert(pattern);
                    let tid_clone = tid.clone();
                    let blocked = ws.blocked_urls.iter().cloned().collect();
                    tokio::spawn(async move {
                        let _ = crate::core::browser::BrowserManager::set_url_blocking(
                            port, tid_clone, blocked,
                        )
                        .await;
                    });
                }
                AppEvent::RequestUrlUnblock(tid, pattern) => {
                    let port = self.state.config.remote_debug_port;
                    let ws = self.ensure_workspace(&tid);
                    ws.blocked_urls.remove(&pattern);
                    let tid_clone = tid.clone();
                    let blocked = ws.blocked_urls.iter().cloned().collect();
                    tokio::spawn(async move {
                        let _ = crate::core::browser::BrowserManager::set_url_blocking(
                            port, tid_clone, blocked,
                        )
                        .await;
                    });
                }
                AppEvent::RequestAutomationRun(tid, steps) => {
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    let dsl = crate::core::automation::dsl::AutomationDsl {
                        dsl_version: 1,
                        steps: map_ui_steps_to_dsl(&steps),
                    };
                    tracing::info!("[USER] Automation pipeline started for tab: {}", tid_clone);

                    let output_dir = self.state.config.output_dir.clone();
                    tokio::spawn(async move {
                        let mut engine = crate::core::automation::AutomationEngine::new(
                            port, tid_clone, output_dir,
                        );
                        let _ = engine.run(dsl).await;
                    });
                }
                AppEvent::AutomationProgress(tid, step) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.auto_status = AutomationStatus::Running(step);
                }
                AppEvent::AutomationFinished(tid) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.auto_status = AutomationStatus::Finished;
                }
                AppEvent::AutomationError(tid, err) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.auto_status = AutomationStatus::Error(err);
                }
                AppEvent::AutomationDatasetUpdated(tid, data) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.extracted_data = data;
                }
                AppEvent::OperationSuccess(msg) => {
                    self.state.notify("Success", &msg, false);
                }
                AppEvent::OperationError(msg) => {
                    self.state.notify("Error", &msg, true);
                }
                AppEvent::RequestScriptExecution(tid, script) => {
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    tracing::info!("[USER] JS Execution requested for tab: {}", tid_clone);
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::execute_script(
                            port, tid_clone.clone(), script,
                        )
                        .await
                        {
                            Ok(res) => ui::scrape::emit(AppEvent::ScriptFinished(tid_clone, res)),
                            Err(e) => ui::scrape::emit(AppEvent::OperationError(e.to_string())),
                        }
                    });
                }
                AppEvent::ScriptFinished(tid, res) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.js_result = res;
                }
                AppEvent::RequestCapture(tid, mirror, assets) => {
                    let port = self.state.config.remote_debug_port;
                    let root = self.state.config.output_dir.clone();
                    tracing::info!("[USER] Page capture requested. Mirror: {}, Assets: {}", mirror, assets);
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::capture_html(
                            port, tid, root, mirror, assets,
                        )
                        .await
                        {
                            Ok(p) => ui::scrape::emit(AppEvent::OperationSuccess(format!(
                                "Saved: {:?}",
                                p
                            ))),
                            Err(e) => {
                                tracing::error!("[COMMAND <-> ERROR] Capture failed: {}", e);
                                ui::scrape::emit(AppEvent::OperationError(e.to_string()));
                            }
                        }
                    });
                }
            }
        }

        // --- UI RENDERING ---
        // INITIAL SETUP OVERLAYS
        if !self.state.output_confirmed {
            egui::Window::new("INITIAL SETUP: OUTPUT DIRECTORY")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .resizable(false).collapsible(false).show(ctx, |ui| {
                    ui.set_width(450.0);
                    ui.label("Choose where to save all logs, captures, and extracted data:");
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.state.config.output_dir.to_string_lossy().to_string()).desired_width(300.0));
                        if ui.button("📁 BROWSE").clicked() {
                            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                self.state.config.output_dir = path;
                            }
                        }
                    });
                    ui.add_space(15.0);
                    if ui.add(egui::Button::new(RichText::new("CONFIRM & ACTIVATE LOGGER").strong())
                        .min_size([430.0, 40.0].into())
                        .fill(Color32::from_rgb(0, 120, 215))).clicked() {
                        self.state.output_confirmed = true;
                        ui::scrape::emit(AppEvent::RequestLogPathSet(self.state.config.output_dir.clone()));
                        tracing::info!("[USER] Output directory confirmed: {:?}", self.state.config.output_dir);
                    }
                });
            return; // Don't show main UI yet
        }

        if !self.state.profile_confirmed {
            egui::Window::new("INITIAL SETUP: BROWSER PROFILE")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .resizable(false).collapsible(false).show(ctx, |ui| {
                    ui.set_width(450.0);
                    ui.label("Select which browser profile to use for sessions:");
                    ui.add_space(10.0);
                    
                    ui.vertical(|ui| {
                        if ui.selectable_label(self.state.use_custom_profile, "🏠 ISOLATED PROFILE (Recommended - Created in output folder)").clicked() {
                            self.state.use_custom_profile = true;
                        }
                        ui.add_space(5.0);
                        if ui.selectable_label(!self.state.use_custom_profile, "👤 SYSTEM PROFILE (Uses your existing Chrome/Chromium data)").clicked() {
                            self.state.use_custom_profile = false;
                        }
                    });

                    ui.add_space(15.0);
                    if ui.add(egui::Button::new(RichText::new("START SNIPER STUDIO").strong())
                        .min_size([430.0, 40.0].into())
                        .fill(Color32::from_rgb(0, 150, 100))).clicked() {
                        self.state.profile_confirmed = true;
                        tracing::info!("[USER] Profile type selected: {}", if self.state.use_custom_profile { "Isolated" } else { "System" });
                    }
                });
            return;
        }

        egui::SidePanel::left("nav_panel")
            .resizable(false)
            .default_width(180.0)
            .show(ctx, |ui| {
                ui.add_space(10.0);
                ui.heading(
                    RichText::new("SNIPER CORE")
                        .strong()
                        .color(Color32::from_rgb(0, 200, 255)),
                );
                ui.add_space(20.0);

                ui.selectable_value(&mut self.state.active_tab, Tab::Scrape, " 🎯 SCRAPE");
                ui.selectable_value(&mut self.state.active_tab, Tab::Settings, " ⚙ SETTINGS");

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                    ui.label(RichText::new("V1.2.6").small().color(Color32::from_gray(100)));
                    if let Some(id) = &self.state.selected_tab_id {
                        ui.label(
                            RichText::new(format!("Active: {}", &id[..8]))
                                .small()
                                .color(Color32::GREEN),
                        );
                    }
                });
            });

        egui::TopBottomPanel::top("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let status_col = if self.state.is_browser_running {
                    Color32::GREEN
                } else {
                    Color32::RED
                };
                ui.label(RichText::new("●").color(status_col));
                ui.label(if self.state.is_browser_running {
                    "BROWSER LIVE"
                } else {
                    "BROWSER DOWN"
                });

                ui.separator();

                let ws_count = self.state.workspaces.len();
                ui.label(format!("ACTIVE WORKSPACES: {}", ws_count));

                let ws_ids: Vec<String> = self.state.workspaces.keys().cloned().collect();
                for id in ws_ids {
                    let mut ss = "".to_string();
                    if let Some(ws) = self.state.workspaces.get(&id) {
                        if ws.show_network {
                            ss += "N";
                        }
                        if ws.show_media {
                            ss += "M";
                        }
                        if ws.show_storage {
                            ss += "S";
                        }
                        if ws.show_automation {
                            ss += "A";
                        }
                        if ws.sniffer_active {
                            ss += "C";
                        }

                        if !ss.is_empty()
                            && ui
                                .selectable_label(true, format!(" 📦 {}: {}", &ws.title, ss))
                                .clicked()
                        {
                            self.state.selected_tab_id = Some(id.clone());
                        }
                    }
                }
            });
        });

        // --- WINDOWS (MDI) ---
        let workspace_ids: Vec<String> = self.state.workspaces.keys().cloned().collect();
        for tid in workspace_ids {
            let (mut show_net, mut show_med, mut show_sto, mut show_auto, mut sniffer_active, title) = {
                let ws = self.state.workspaces.get(&tid).unwrap();
                (
                    ws.show_network,
                    ws.show_media,
                    ws.show_storage,
                    ws.show_automation,
                    ws.sniffer_active,
                    ws.title.clone(),
                )
            };

            if show_net {
                egui::Window::new(format!("{} - NETWORK", title))
                    .id(egui::Id::new(format!("{}_net", tid)))
                    .open(&mut show_net)
                    .show(ctx, |ui| {
                        state_bridge_render_network(ui, &mut self.state, &tid);
                    });
            }
            if show_med {
                egui::Window::new(format!("{} - MEDIA", title))
                    .id(egui::Id::new(format!("{}_med", tid)))
                    .open(&mut show_med)
                    .show(ctx, |ui| {
                        state_bridge_render_media(ui, &mut self.state, &tid);
                    });
            }
            if show_sto {
                egui::Window::new(format!("COOKIE MANAGER // {}", title))
                    .id(egui::Id::new(format!("{}_sto", tid)))
                    .open(&mut show_sto)
                    .default_size([800.0, 600.0])
                    .show(ctx, |ui| {
                        state_bridge_render_storage(ui, &mut self.state, &tid);
                    });
            }
            if show_auto {
                egui::Window::new(format!("AUTOMATION // {}", title))
                    .id(egui::Id::new(format!("{}_auto", tid)))
                    .open(&mut show_auto)
                    .default_size([900.0, 700.0])
                    .show(ctx, |ui| {
                        state_bridge_render_automation(ui, &mut self.state, &tid);
                    });
            }
            if sniffer_active {
                egui::Window::new(format!("CONSOLE // {}", title))
                    .id(egui::Id::new(format!("{}_sniffer", tid)))
                    .open(&mut sniffer_active)
                    .default_size([700.0, 500.0])
                    .show(ctx, |ui| {
                        state_bridge_render_sniffer(ui, &mut self.state, &tid);
                    });
            }

            if let Some(ws) = self.state.workspaces.get_mut(&tid) {
                ws.show_network = show_net;
                ws.show_media = show_med;
                ws.show_storage = show_sto;
                ws.show_automation = show_auto;
                ws.sniffer_active = sniffer_active;
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| match self.state.active_tab {
            Tab::Scrape => ui::scrape::render(ui, &mut self.state),
            Tab::Settings => ui::config_panel::render(ui, &mut self.state),
            _ => {}
        });

        egui::TopBottomPanel::bottom("log_panel")
            .resizable(true)
            .default_height(150.0)
            .show(ctx, |ui| {
                ui::log_panel::render(ui, &mut self.state);
            });

        if let Some(notif) = &self.state.notification {
            let mut close = false;
            egui::Window::new(&notif.title)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(&notif.message);
                    if ui.button("OK").clicked() {
                        close = true;
                    }
                });
            if close {
                self.state.notification = None;
            }
        }

        ctx.request_repaint();
    }
}

fn map_ui_steps_to_dsl(
    steps: &[crate::state::AutomationStep],
) -> Vec<crate::core::automation::dsl::Step> {
    steps
        .iter()
        .map(|s| match s {
            crate::state::AutomationStep::Navigate(u) => {
                crate::core::automation::dsl::Step::Navigate { url: u.clone() }
            }
            crate::state::AutomationStep::Click(sel) => crate::core::automation::dsl::Step::Click {
                selector: sel.clone(),
            },
            crate::state::AutomationStep::RightClick(sel) => crate::core::automation::dsl::Step::RightClick {
                selector: sel.clone(),
            },
            crate::state::AutomationStep::Hover(sel) => crate::core::automation::dsl::Step::Hover {
                selector: sel.clone(),
            },
            crate::state::AutomationStep::Type {
                selector,
                value,
                is_variable,
            } => crate::core::automation::dsl::Step::Type {
                selector: selector.clone(),
                value: value.clone(),
                is_variable: *is_variable,
            },
            crate::state::AutomationStep::Wait(secs) => crate::core::automation::dsl::Step::Wait {
                seconds: *secs,
            },
            crate::state::AutomationStep::WaitSelector {
                selector,
                timeout_ms,
            } => crate::core::automation::dsl::Step::WaitSelector {
                selector: selector.clone(),
                timeout_ms: *timeout_ms,
            },
            crate::state::AutomationStep::WaitUntilIdle { timeout_ms } => {
                crate::core::automation::dsl::Step::WaitUntilIdle {
                    timeout_ms: *timeout_ms,
                }
            }
            crate::state::AutomationStep::WaitNetworkIdle { timeout_ms, min_idle_ms } => {
                crate::core::automation::dsl::Step::WaitNetworkIdle {
                    timeout_ms: *timeout_ms,
                    min_idle_ms: *min_idle_ms,
                }
            }
            crate::state::AutomationStep::ScrollBottom => {
                crate::core::automation::dsl::Step::ScrollBottom
            }
            crate::state::AutomationStep::Extract {
                selector,
                as_key,
                add_to_dataset,
            } => crate::core::automation::dsl::Step::Extract {
                selector: selector.clone(),
                as_key: as_key.clone(),
                add_to_row: *add_to_dataset,
            },
            crate::state::AutomationStep::SetVariable { key, value } => {
                crate::core::automation::dsl::Step::SetVariable {
                    key: key.clone(),
                    value: value.clone(),
                }
            }
            crate::state::AutomationStep::NewRow => crate::core::automation::dsl::Step::NewRow,
            crate::state::AutomationStep::Export(f) => crate::core::automation::dsl::Step::Export {
                filename: f.clone(),
            },
            crate::state::AutomationStep::Screenshot(f) => {
                crate::core::automation::dsl::Step::Screenshot {
                    filename: f.clone(),
                }
            }
            crate::state::AutomationStep::SwitchFrame(sel) => {
                crate::core::automation::dsl::Step::SwitchFrame {
                    selector: sel.clone(),
                }
            }
            crate::state::AutomationStep::If {
                selector,
                then_steps,
            } => crate::core::automation::dsl::Step::If {
                selector: selector.clone(),
                then_steps: map_ui_steps_to_dsl(then_steps),
            },
            crate::state::AutomationStep::ForEach { selector, body } => {
                crate::core::automation::dsl::Step::ForEach {
                    selector: selector.clone(),
                    body: map_ui_steps_to_dsl(body),
                }
            }
        })
        .collect()
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
fn state_bridge_render_automation(ui: &mut egui::Ui, state: &mut AppState, tid: &str) {
    let old_id = state.selected_tab_id.clone();
    state.selected_tab_id = Some(tid.to_string());
    crate::ui::automation::render_embedded(ui, state, tid);
    state.selected_tab_id = old_id;
}
fn state_bridge_render_sniffer(ui: &mut egui::Ui, state: &mut AppState, tid: &str) {
    let old_id = state.selected_tab_id.clone();
    state.selected_tab_id = Some(tid.to_string());
    if let Some(ws) = state.workspaces.get_mut(tid) {
        ui.label(RichText::new("JAVASCRIPT INJECTOR").strong());
        ui.add(
            egui::TextEdit::multiline(&mut ws.js_script)
                .font(egui::FontId::monospace(13.0))
                .desired_rows(6)
                .desired_width(f32::INFINITY),
        );
        if ui.button("EXECUTE SCRIPT").clicked() {
            crate::ui::scrape::emit(AppEvent::RequestScriptExecution(
                tid.to_string(),
                ws.js_script.clone(),
            ));
        }
        if !ws.js_result.is_empty() {
            ui.label(
                RichText::new(format!("> {}", ws.js_result))
                    .color(Color32::GREEN)
                    .monospace(),
            );
        }

        ui.separator();

        ui.horizontal(|ui| {
            ui.label(RichText::new("SYSTEM LOGS").strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("CLEAR").clicked() {
                    ws.console_logs.clear();
                }
                if ui.button("SAVE LOG").clicked() {
                    let content = ws.console_logs.join("\n");
                    if let Some(path) = rfd::FileDialog::new()
                        .set_file_name("console.log")
                        .save_file()
                    {
                        let _ = std::fs::write(path, content);
                    }
                }
                if ui.button("COPY ALL").clicked() {
                    ui.ctx().copy_text(ws.console_logs.join("\n"));
                }
            });
        });

        ui.add_space(5.0);
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .max_height(f32::INFINITY)
            .show(ui, |ui| {
                for log in &ws.console_logs {
                    ui.label(RichText::new(log).small().font(egui::FontId::monospace(11.0)));
                }
            });
    }
    state.selected_tab_id = old_id;
}
