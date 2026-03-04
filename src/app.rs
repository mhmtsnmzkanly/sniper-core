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
        Self { state, log_receiver, event_receiver, browser_process: Arc::new(Mutex::new(None)) }
    }

    /// Belirtilen sekme için çalışma alanının varlığını garanti eder.
    fn ensure_workspace(&mut self, tid: &str) -> &mut crate::state::TabWorkspace {
        if !self.state.workspaces.contains_key(tid) {
            let title = self.state.available_tabs.iter().find(|t| t.id == tid).map(|t| t.title.clone()).unwrap_or_else(|| "New Tab".into());
            tracing::debug!("[APP] Creating new workspace for tab: {} ({})", title, tid);
            self.state.workspaces.insert(tid.to_string(), crate::state::TabWorkspace::new(tid.to_string(), title));
        }
        self.state.workspaces.get_mut(tid).unwrap()
    }
}

/// UI'daki basitleştirilmiş adımları motorun anlayacağı DSL adımlarına dönüştürür.
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
        // Sistem loglarını UI listesine aktar
        while let Ok(log) = self.log_receiver.try_recv() {
            self.state.logs.push(log);
            if self.state.logs.len() > 1500 { self.state.logs.remove(0); }
        }

        // --- OLAY İŞLEME DÖNGÜSÜ ---
        while let Ok(event) = self.event_receiver.try_recv() {
            tracing::debug!("[EVENT] Processing: {:?}", event);

            // Tarayıcı bağımlı komutlar için koruma
            if !self.state.is_browser_running {
                match &event {
                    AppEvent::RequestCookies(_) | AppEvent::RequestPageReload(_) | 
                    AppEvent::RequestScriptExecution(_, _) | AppEvent::RequestAutomationRun(..) |
                    AppEvent::RequestCapture(..) | AppEvent::RequestPageSelectors(_) |
                    AppEvent::RequestTabRefresh => {
                        let msg = "Komut Reddedildi: Tarayıcı aktif değil.";
                        self.state.notify("Denied", msg, true);
                        tracing::warn!("[APP] {}", msg);
                        continue;
                    }
                    _ => {}
                }
            }

            match event {
                AppEvent::RequestLogPathSet(path) => {
                    tracing::info!("[APP] Setting session log path to: {:?}", path);
                    crate::logger::set_log_path(path, &self.state.session_timestamp);
                }
                AppEvent::BrowserStarted(child) => {
                    *self.browser_process.lock().unwrap() = Some(child);
                    self.state.is_browser_running = true;
                    self.state.notify("System", "Tarayıcı başarıyla başlatıldı.", false);
                    tracing::info!("[APP] Browser connection established.");
                }
                AppEvent::BrowserTerminated => {
                    self.state.is_browser_running = false;
                    self.state.available_tabs.clear();
                    self.state.selected_tab_id = None;
                    self.state.notify("System", "Tarayıcı bağlantısı koptu.", true);
                    tracing::warn!("[APP] Browser connection lost.");
                }
                AppEvent::TerminateBrowser => {
                    if let Some(mut child) = self.browser_process.lock().unwrap().take() {
                        let _ = child.kill();
                        self.state.is_browser_running = false;
                        self.state.available_tabs.clear();
                        self.state.selected_tab_id = None;
                        self.state.notify("System", "Tarayıcı sonlandırıldı.", false);
                        tracing::info!("[APP] Browser terminated by user.");
                    }
                }
                AppEvent::TabsUpdated(tabs) => {
                    tracing::info!("[APP] Found {} active tabs.", tabs.len());
                    self.state.available_tabs = tabs;
                }
                AppEvent::ConsoleLogAdded(tid, msg) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.console_logs.push(msg.clone());
                }
                AppEvent::SelectorsReceived(tid, sels) => {
                    let ws = self.ensure_workspace(&tid);
                    ws.discovered_selectors = sels;
                    tracing::info!("[APP][{}] {} selectors discovered.", tid, ws.discovered_selectors.len());
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
                    tracing::info!("[APP][{}] {} cookies retrieved.", tid, ws.cookies.len());
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
                    tracing::info!("[OP] Success: {}", msg);
                }
                AppEvent::OperationError(msg) => {
                    self.state.notify("Error", &msg, true);
                    tracing::error!("[OP] Error: {}", msg);
                }
                
                // --- BROWSER COMMAND HANDLERS ---
                AppEvent::RequestCookies(tid) => {
                    tracing::info!("[APP -> BROWSER] Requesting cookies for {}", tid);
                    let port = self.state.config.remote_debug_port;
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::get_cookies(port, tid.clone()).await {
                            Ok(cookies) => crate::ui::scrape::emit(AppEvent::CookiesReceived(tid, cookies)),
                            Err(e) => crate::ui::scrape::emit(AppEvent::OperationError(format!("Cookie fetch failed: {}", e))),
                        }
                    });
                }
                AppEvent::RequestCookieDelete(tid, name, domain) => {
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
                    let port = self.state.config.remote_debug_port;
                    tokio::spawn(async move {
                        if let Err(e) = crate::core::browser::BrowserManager::reload_page(port, tid).await {
                            crate::ui::scrape::emit(AppEvent::OperationError(format!("Reload failed: {}", e)));
                        }
                    });
                }
                AppEvent::RequestTabRefresh => {
                    let port = self.state.config.remote_debug_port;
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::list_tabs(port).await {
                            Ok(tabs) => crate::ui::scrape::emit(AppEvent::TabsUpdated(tabs)),
                            Err(e) => crate::ui::scrape::emit(AppEvent::OperationError(format!("Tab listing failed: {}", e))),
                        }
                    });
                }
                AppEvent::RequestScriptExecution(tid, script) => {
                    let port = self.state.config.remote_debug_port;
                    let tid_clone = tid.clone();
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::execute_script(port, tid, script).await {
                            Ok(res) => crate::ui::scrape::emit(AppEvent::ScriptFinished(tid_clone, res)),
                            Err(e) => crate::ui::scrape::emit(AppEvent::OperationError(format!("JS error: {}", e))),
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
                            Err(e) => crate::ui::scrape::emit(AppEvent::OperationError(format!("Selector scan failed: {}", e))),
                        }
                    });
                }
                AppEvent::RequestNetworkToggle(tid, _active) => {
                    tracing::info!("[APP -> BROWSER] Enabling sniffer/listeners for tab: {}", tid);
                    let port = self.state.config.remote_debug_port;
                    tokio::spawn(async move {
                        if let Err(e) = crate::core::browser::BrowserManager::setup_tab_listeners(port, tid).await {
                            crate::ui::scrape::emit(AppEvent::OperationError(format!("Listener setup failed: {}", e)));
                        }
                    });
                }
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
                    tracing::info!("[APP -> BROWSER] Starting capture. Mode: {} for tab: {}", mode, tid);
                    let port = self.state.config.remote_debug_port;
                    let root = self.state.config.output_dir.clone();
                    tokio::spawn(async move {
                        let res = match mode.as_str() {
                            "html" => crate::core::browser::BrowserManager::capture_html(port, tid, root).await,
                            "complete" => crate::core::browser::BrowserManager::capture_complete(port, tid, root).await,
                            "mirror" => crate::core::browser::BrowserManager::capture_mirror(port, tid, root).await,
                            _ => Err(crate::core::error::AppError::Internal("Mod bulunamadı".into())),
                        };
                        match res {
                            Ok(path) => crate::ui::scrape::emit(AppEvent::OperationSuccess(format!("Yakalandı: {:?}", path))),
                            Err(e) => crate::ui::scrape::emit(AppEvent::OperationError(format!("Yakalama hatası: {}", e))),
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
                    tracing::info!("[APP -> ENGINE] Automation started for {}", tid_clone);
                    let output_dir = self.state.config.output_dir.clone();
                    tokio::spawn(async move {
                        let mut engine = crate::core::automation::engine::AutomationEngine::new(port, tid_clone, output_dir);
                        engine.config = crate::core::automation::engine::ExecutionConfig {
                            step_timeout: std::time::Duration::from_millis(auto_config.step_timeout_ms),
                            retry_attempts: auto_config.retry_attempts,
                            screenshot_on_error: auto_config.screenshot_on_error,
                        };
                        if let Err(e) = engine.run(dsl).await {
                            crate::ui::scrape::emit(AppEvent::AutomationError(tid.clone(), e.to_string()));
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

        // KURULUM MODALI
        if !self.state.output_confirmed {
            egui::Window::new("Sniper Core - Initial Setup").anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false).resizable(false).show(ctx, |ui| {
                    ui.heading("Çıktı Dizini Seçin");
                    ui.label("Veriler, loglar ve varlıklar bu klasöre kaydedilecek.");
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{:?}", self.state.config.output_dir)).color(Color32::KHAKI));
                        if ui.button("Gözat...").clicked() { if let Some(path) = rfd::FileDialog::new().pick_folder() { self.state.config.output_dir = path; } }
                    });
                    ui.add_space(20.0);
                    if ui.button(RichText::new("ONAYLA VE DEVAM ET").strong()).clicked() {
                        self.state.output_confirmed = true;
                        let _ = std::fs::create_dir_all(&self.state.config.output_dir);
                        crate::ui::scrape::emit(AppEvent::RequestLogPathSet(self.state.config.output_dir.clone()));
                    }
                });
            return;
        }

        if !self.state.profile_confirmed {
            egui::Window::new("Tarayıcı Profil Ayarı").anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false).resizable(false).show(ctx, |ui| {
                    ui.heading("Profil Modu Seçin");
                    ui.vertical(|ui| {
                        if ui.selectable_label(self.state.use_custom_profile, "🏠 İZOLE PROFİL (Önerilen)").clicked() { self.state.use_custom_profile = true; }
                        if ui.selectable_label(!self.state.use_custom_profile, "👤 SİSTEM PROFİLİ").clicked() { self.state.use_custom_profile = false; }
                    });
                    ui.add_space(20.0);
                    if ui.button(RichText::new("ONAYLA VE BAŞLAT").strong()).clicked() { self.state.profile_confirmed = true; }
                });
            return;
        }

        egui::CentralPanel::default().show(ctx, |ui| match self.state.active_tab {
            Tab::Scrape => ui::scrape::render(ui, &mut self.state),
            Tab::Settings => ui::config_panel::render(ui, &mut self.state),
            Tab::Logs => ui::log_panel::render(ui, &mut self.state),
            _ => { ui.label("Bölüm henüz eklenmedi."); }
        });

        // Bildirimler
        if let Some(notif) = &self.state.notification {
            let mut open = true;
            egui::Window::new(&notif.title).open(&mut open).anchor(egui::Align2::RIGHT_BOTTOM, [-20.0, -20.0])
                .collapsible(false).resizable(false).show(ctx, |ui| { ui.label(&notif.message); });
            if !open { self.state.notification = None; }
        }
        ctx.request_repaint();
    }
}
