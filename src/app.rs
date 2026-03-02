use crate::state::{AppState, Tab};
use crate::core::events::AppEvent;
use crate::ui;
use eframe::egui;
use std::sync::{Arc, Mutex};

pub struct CrawlerApp {
    pub state: AppState,
    pub log_receiver: tokio::sync::mpsc::UnboundedReceiver<crate::state::LogEntry>,
    pub event_receiver: tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    pub browser_process: Arc<Mutex<Option<std::process::Child>>>,
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
        
        // 1. Otomatik Sekme Yenileme
        if self.state.is_browser_running && (current_time - self.state.last_tab_refresh) > 2.0 {
            self.state.last_tab_refresh = current_time;
            let port = self.state.config.remote_debug_port;
            tokio::spawn(async move {
                if let Ok(tabs) = crate::core::browser::BrowserManager::list_tabs(port).await {
                    ui::scrape::emit(AppEvent::TabsUpdated(tabs));
                }
            });
        }

        // 2. Logları İşle
        while let Ok(log) = self.log_receiver.try_recv() {
            self.state.logs.push(log);
        }

        // 3. Olayları İşle
        while let Ok(event) = self.event_receiver.try_recv() {
            match event {
                AppEvent::TabsUpdated(tabs) => {
                    self.state.available_tabs = tabs;
                }
                AppEvent::BrowserStarted(child) => {
                    let mut lock = self.browser_process.lock().unwrap();
                    *lock = Some(child);
                    self.state.is_browser_running = true;
                    tracing::info!("Browser started and connected.");
                }
                AppEvent::BrowserTerminated => {
                    let mut lock = self.browser_process.lock().unwrap();
                    if let Some(mut child) = lock.take() {
                        Self::kill_browser_group(&mut child);
                    }
                    self.state.is_browser_running = false;
                    self.state.available_tabs.clear();
                    self.state.selected_tab_id = None;
                    tracing::warn!("Browser terminated.");
                }
                AppEvent::RequestCapture(tab_id, mirror_mode) => {
                    let port = self.state.config.remote_debug_port;
                    let root = self.state.config.raw_output_dir.clone();
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::capture_html(port, tab_id, root, mirror_mode).await {
                            Ok(p) => tracing::info!("✅ Captured: {:?}", p),
                            Err(e) => tracing::error!("❌ Capture Failed: {}", e),
                        }
                    });
                }
                AppEvent::RequestTabRefresh => {
                    let port = self.state.config.remote_debug_port;
                    tokio::spawn(async move {
                        if let Ok(tabs) = crate::core::browser::BrowserManager::list_tabs(port).await {
                            ui::scrape::emit(AppEvent::TabsUpdated(tabs));
                        }
                    });
                }
                AppEvent::OperationError(msg) => {
                    tracing::error!("System Error: {}", msg);
                }
                _ => {}
            }
        }

        // 4. UI Çizim
        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading("SCRAPER STUDIO");
            ui.add_space(10.0);
            ui.selectable_value(&mut self.state.active_tab, Tab::Scrape, "SCRAPE");
            ui.selectable_value(&mut self.state.active_tab, Tab::Translate, "TRANSLATE");
            ui.selectable_value(&mut self.state.active_tab, Tab::Settings, "SETTINGS");
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.label(format!("v{}", env!("CARGO_PKG_VERSION")));
            });
        });

        egui::TopBottomPanel::bottom("log_panel").resizable(true).default_height(350.0).show(ctx, |ui| {
            ui::log_panel::render(ui, &mut self.state);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.state.active_tab {
                Tab::Scrape => ui::scrape::render(ui, &mut self.state),
                Tab::Translate => ui::translate::render(ui, &mut self.state),
                Tab::Settings => ui::config_panel::render(ui, &mut self.state),
            }
        });

        ctx.request_repaint();
    }
}
