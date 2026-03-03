use crate::core::events::AppEvent;
use crate::state::{AppState, Tab, TabWorkspace};
use crate::ui;
use chromiumoxide::Browser;
use eframe::egui::{self, Color32, RichText};
use std::sync::{Arc, Mutex};

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
            unsafe {
                libc::kill(-(pid as i32), libc::SIGTERM);
            }
        }
        #[cfg(windows)]
        {
            let _ = std::process::Command::new("taskkill")
                .arg("/F")
                .arg("/T")
                .arg("/PID")
                .arg(pid.to_string())
                .spawn();
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

    fn ensure_workspace(&mut self, tid: &str) -> &mut TabWorkspace {
        if !self.state.workspaces.contains_key(tid) {
            let title = self
                .state
                .available_tabs
                .iter()
                .find(|t| t.id == tid)
                .map(|t| t.title.clone())
                .unwrap_or_else(|| format!("Tab {}", &tid[..8]));
            self.state
                .workspaces
                .insert(tid.to_string(), TabWorkspace::new(tid.to_string(), title));
        }
        self.state.workspaces.get_mut(tid).unwrap()
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

        // --- STARTUP STAGES ---
        if !self.state.output_confirmed {
            egui::Window::new("Sniper Studio Setup 1/2")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("Unified Output Directory").strong());
                        if ui.button("✅ USE DEFAULT").clicked() {
                            self.state.output_confirmed = true;
                            crate::logger::set_log_path(self.state.config.output_dir.clone());
                        }
                        if ui.button("📁 CHOOSE FOLDER").clicked() {
                            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                self.state.config.output_dir = path;
                                self.state.output_confirmed = true;
                                crate::logger::set_log_path(self.state.config.output_dir.clone());
                            }
                        }
                    });
                });
            return;
        }

        if !self.state.profile_confirmed {
            egui::Window::new("Sniper Studio Setup 2/2")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("Browser Identity").strong());
                        if ui.button("👤 SYSTEM PROFILE").clicked() {
                            self.state.config.default_profile_dir =
                                crate::core::browser::BrowserManager::get_system_profile_path();
                            self.state.profile_confirmed = true;
                        }
                        if ui.button("🆕 ISOLATED PROFILE").clicked() {
                            self.state.profile_confirmed = true;
                        }
                    });
                });
            return;
        }

        // --- ASYNC BUFFERS & AUTO-TASKS ---
        while let Ok(log) = self.log_receiver.try_recv() {
            self.state.logs.push(log);
        }

        if self.state.is_browser_running && (current_time - self.state.last_tab_refresh) > 2.0 {
            self.state.last_tab_refresh = current_time;
            let port = self.state.config.remote_debug_port;
            tokio::spawn(async move {
                match crate::core::browser::BrowserManager::list_tabs(port).await {
                    Ok(tabs) => ui::scrape::emit(AppEvent::TabsUpdated(tabs)),
                    Err(e) => {
                        tracing::debug!("[BROWSER <-> LIST] Could not reach debug port: {}", e)
                    }
                }
            });
        }

        // --- EVENT ROUTING ---
        while let Ok(event) = self.event_receiver.try_recv() {
            match event {
                AppEvent::TabsUpdated(tabs) => {
                    self.state.available_tabs = tabs;
                }
                AppEvent::BrowserStarted(child) => {
                    let mut lock = self.browser_process.lock().unwrap();
                    *lock = Some(child);
                    self.state.is_browser_running = true;
                    tracing::info!("[BROWSER <-> STATUS] Instance Online.");
                }
                AppEvent::BrowserTerminated | AppEvent::TerminateBrowser => {
                    let mut lock = self.browser_process.lock().unwrap();
                    if let Some(mut child) = lock.take() {
                        Self::kill_browser_group(&mut child);
                    }
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
                    if let Some(req) = ws.network_requests.iter_mut().find(|r| r.request_id == rid)
                    {
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
                    let blocked = ws.blocked_urls.iter().cloned().collect::<Vec<_>>();
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

                    let output_dir = self.state.config.output_dir.clone();
                    tokio::spawn(async move {
                        let mut engine = crate::core::automation::AutomationEngine::new(
                            port, tid_clone, output_dir,
                        );
                        let _ = engine.run(dsl).await;
                    });
                }
                AppEvent::AutomationProgress(tid, step) => {
                    if let Some(ws) = self.state.workspaces.get_mut(&tid) {
                        ws.auto_status = crate::state::AutomationStatus::Running(step);
                    }
                }
                AppEvent::AutomationFinished(tid) => {
                    if let Some(ws) = self.state.workspaces.get_mut(&tid) {
                        ws.auto_status = crate::state::AutomationStatus::Finished;
                    }
                }
                AppEvent::AutomationError(tid, err) => {
                    if let Some(ws) = self.state.workspaces.get_mut(&tid) {
                        ws.auto_status = crate::state::AutomationStatus::Error(err);
                    }
                }
                AppEvent::RequestPageSelectors(tid) => {
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::get_page_selectors(
                            port,
                            tid_clone.clone(),
                        )
                        .await
                        {
                            Ok(selectors) => {
                                ui::scrape::emit(AppEvent::SelectorsReceived(tid_clone, selectors))
                            }
                            Err(_) => {}
                        }
                    });
                }
                AppEvent::SelectorsReceived(tid, selectors) => {
                    tracing::info!(
                        "[APP <-> EVENT] Received {} selectors for tab {}.",
                        selectors.len(),
                        tid
                    );
                    if let Some(ws) = self.state.workspaces.get_mut(&tid) {
                        ws.discovered_selectors = selectors;
                    }
                }
                AppEvent::AutomationDatasetUpdated(tid, data) => {
                    if let Some(ws) = self.state.workspaces.get_mut(&tid) {
                        ws.extracted_data = data;
                    }
                }
                AppEvent::RequestScriptExecution(tid, script) => {
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::execute_script(
                            port,
                            tid_clone.clone(),
                            script,
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
                AppEvent::RequestCapture(tid, mirror) => {
                    let port = self.state.config.remote_debug_port;
                    let root = self.state.config.output_dir.clone();
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::capture_html(
                            port, tid, root, mirror,
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
                AppEvent::RequestCookies(tid) => {
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::get_cookies(
                            port,
                            tid_clone.clone(),
                        )
                        .await
                        {
                            Ok(cookies) => {
                                tracing::info!(
                                    "[STORAGE <-> DATA] Received {} cookies.",
                                    cookies.len()
                                );
                                ui::scrape::emit(AppEvent::CookiesReceived(tid_clone, cookies));
                            }
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
                        match crate::core::browser::BrowserManager::delete_cookie(
                            port,
                            tid_clone.clone(),
                            name,
                            domain,
                        )
                        .await
                        {
                            Ok(_) => ui::scrape::emit(AppEvent::RequestCookies(tid_clone)),
                            Err(e) => ui::scrape::emit(AppEvent::OperationError(e.to_string())),
                        }
                    });
                }
                AppEvent::RequestCookieAdd(tid, cookie) => {
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::add_cookie(
                            port,
                            tid_clone.clone(),
                            cookie,
                        )
                        .await
                        {
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
                        if let Err(e) =
                            crate::core::browser::BrowserManager::setup_tab_listeners(port, tid)
                                .await
                        {
                            tracing::error!("[CDP <-> ERROR] Listener failed: {}", e);
                        }
                    });
                }
                AppEvent::OperationSuccess(msg) => {
                    self.state.notify("Success", &msg, false);
                }
                AppEvent::OperationError(msg) => {
                    self.state.notify("Error", &msg, true);
                }
                _ => {}
            }
        }

        // --- UI DRAWING ---
        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading(
                RichText::new("SNIPER STUDIO")
                    .strong()
                    .color(Color32::KHAKI),
            );
            ui.add_space(15.0);
            ui.label("CORE TOOLS");
            ui.selectable_value(&mut self.state.active_tab, Tab::Scrape, " 🎯 SCRAPE");
            ui.add_space(15.0);
            ui.label("SYSTEM");
            ui.selectable_value(&mut self.state.active_tab, Tab::Settings, " ⚙ SETTINGS");

            ui.add_space(20.0);
            ui.label(
                RichText::new("ACTIVE INSPECTORS")
                    .small()
                    .color(Color32::LIGHT_BLUE),
            );
            let open_ws: Vec<(String, String, bool, bool, bool)> = self
                .state
                .workspaces
                .iter()
                .map(|(id, ws)| {
                    (
                        id.clone(),
                        ws.title.clone(),
                        ws.show_network,
                        ws.show_media,
                        ws.show_storage,
                    )
                })
                .collect();
            for (_tid, title, sn, sm, ss) in open_ws {
                let trunc: String = title.chars().take(10).collect();
                if sn
                    && ui
                        .selectable_label(true, format!(" 🌐 Net: {}", trunc))
                        .clicked()
                {}
                if sm
                    && ui
                        .selectable_label(true, format!(" 🖼 Med: {}", trunc))
                        .clicked()
                {}
                if ss
                    && ui
                        .selectable_label(true, format!(" 📦 Sto: {}", ss))
                        .clicked()
                {}
            }
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
