use crate::state::{AppState, Tab, ChromeTabInfo};
use crate::ui;
use eframe::egui;
use std::sync::{Arc, Mutex};

pub enum AppEvent {
    TabsUpdated(Vec<ChromeTabInfo>),
    BrowserStarted(std::process::Child),
    TerminateBrowser,
}

pub struct CrawlerApp {
    pub state: AppState,
    pub log_receiver: tokio::sync::mpsc::UnboundedReceiver<crate::state::LogEntry>,
    pub event_receiver: tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    pub browser_process: Arc<Mutex<Option<std::process::Child>>>,
}

impl CrawlerApp {
    fn kill_browser_group(child: &mut std::process::Child) {
        crate::backend::utils::kill_process_group(child);
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

impl CrawlerApp {
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

impl eframe::App for CrawlerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let current_time = ctx.input(|i| i.time);
        
        // Otomatik Sekme Yenileme (Her 2 saniyede bir)
        if self.state.is_browser_running && (current_time - self.state.last_tab_refresh) > 2.0 {
            self.state.last_tab_refresh = current_time;
            let port = self.state.remote_port;
            tokio::spawn(async move {
                if let Ok(tabs) = crate::backend::Scraper::list_tabs(port).await {
                    crate::ui::scrape::emit(AppEvent::TabsUpdated(tabs));
                }
            });
        }

        while let Ok(log) = self.log_receiver.try_recv() {
            self.state.logs.push(log);
        }

        while let Ok(event) = self.event_receiver.try_recv() {
            match event {
                AppEvent::TabsUpdated(tabs) => {
                    self.state.available_tabs = tabs;
                }
                AppEvent::BrowserStarted(child) => {
                    let mut lock = self.browser_process.lock().unwrap();
                    *lock = Some(child);
                    self.state.is_browser_running = true;
                }
                AppEvent::TerminateBrowser => {
                    let mut lock = self.browser_process.lock().unwrap();
                    if let Some(mut child) = lock.take() {
                        Self::kill_browser_group(&mut child);
                    }
                    self.state.is_browser_running = false;
                    self.state.available_tabs.clear();
                    self.state.selected_tab_id = None;
                }
            }
        }

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading("MENU");
            ui.add_space(10.0);

            ui.selectable_value(&mut self.state.active_tab, Tab::Scrape, "SCRAPE");
            ui.selectable_value(&mut self.state.active_tab, Tab::Translate, "TRANSLATE");

            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.label(format!("v{}", env!("CARGO_PKG_VERSION")));
            });
        });


        egui::TopBottomPanel::bottom("log_panel")
            .resizable(true)
            .default_height(400.0)
            .show(ctx, |ui| {
                ui::log_panel::render(ui, &mut self.state);
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.state.active_tab {
                Tab::Scrape => ui::scrape::render(ui, &mut self.state),
                Tab::Translate => ui::translate::render(ui, &mut self.state),
            }
        });

        ctx.request_repaint();
    }
}
