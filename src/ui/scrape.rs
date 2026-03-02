use crate::state::AppState;
use egui::{Ui, Color32, RichText};
use tokio::sync::mpsc;
use crate::app::AppEvent;
use std::sync::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    static ref EVENT_SENDER: Mutex<Option<mpsc::UnboundedSender<AppEvent>>> = Mutex::new(None);
}

pub fn set_event_sender(tx: mpsc::UnboundedSender<AppEvent>) {
    let mut lock = EVENT_SENDER.lock().unwrap();
    *lock = Some(tx);
}

pub fn emit(event: AppEvent) {
    let lock = EVENT_SENDER.lock().unwrap();
    if let Some(tx) = &*lock {
        let _: Result<(), _> = tx.send(event);
    }
}

pub fn render(ui: &mut Ui, state: &mut AppState) {
    ui.heading("SNIPER SCRAPER 3.0 (Direct Connect)");
    ui.add_space(10.0);

    // Settings
    ui.group(|ui| {
        ui.columns(2, |cols| {
            cols[0].horizontal(|ui| {
                ui.label("🌐 PORT:");
                ui.add(egui::DragValue::new(&mut state.remote_port).range(1024..=65535));
            });
            cols[1].horizontal(|ui| {
                ui.label("📁 SAVE TO:");
                if ui.button("Browse").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        state.scrape_path = Some(path);
                    }
                }
            });
        });
        if let Some(path) = &state.scrape_path {
            ui.label(RichText::new(path.to_string_lossy()).small().color(Color32::LIGHT_BLUE));
        }
    });

    ui.add_space(10.0);

    // Step 1: Control
    ui.group(|ui| {
        ui.label(RichText::new("Step 1: Browser Control").strong());
        ui.horizontal(|ui| {
            ui.label("URL:");
            ui.text_edit_singleline(&mut state.scrape_url);
            
            if !state.is_browser_running {
                if ui.button("LAUNCH BROWSER").clicked() {
                    let url = state.scrape_url.clone();
                    let profile = state.custom_profile_path.clone();
                    let port = state.remote_port;
                    let ts = state.session_timestamp.clone();
                    tokio::spawn(async move {
                        match crate::backend::Scraper::launch(&url, profile, port, ts).await {
                            Ok(child) => emit(AppEvent::BrowserStarted(child)),
                            Err(e) => tracing::error!("Launch failed: {}", e),
                        }
                    });
                }
            } else {
                if ui.button(RichText::new("STOP BROWSER").color(Color32::RED)).clicked() {
                    emit(AppEvent::TerminateBrowser);
                }
            }
        });
    });

    ui.add_space(10.0);

    // Step 2: Tab Selection (Direct)
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Step 2: Target Selection").strong().size(16.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("REFRESH LIST").clicked() {
                    let port = state.remote_port;
                    tokio::spawn(async move {
                        if let Ok(tabs) = crate::backend::Scraper::list_tabs(port).await {
                            emit(AppEvent::TabsUpdated(tabs));
                        }
                    });
                }
            });
        });

        ui.add_space(8.0);
        
        egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
            if state.available_tabs.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("Waiting for browser tabs...").italics().color(Color32::GRAY));
                });
            } else {
                let column_width = (ui.available_width() - 40.0) / 3.0;
                
                egui::Grid::new("tab_grid_direct")
                    .num_columns(3)
                    .min_col_width(column_width)
                    .max_col_width(column_width)
                    .spacing([15.0, 12.0])
                    .striped(true)
                    .show(ui, |ui| {
                        // Header
                        ui.label(RichText::new("ID").strong().color(Color32::KHAKI));
                        ui.label(RichText::new("PAGE TITLE").strong().color(Color32::KHAKI));
                        ui.label(RichText::new("URL").strong().color(Color32::KHAKI));
                        ui.end_row();

                        let tabs = state.available_tabs.clone();
                        for tab in tabs {
                            let is_selected = Some(tab.id.clone()) == state.selected_tab_id;
                            let font_size = 13.0;
                            
                            // Col 1: ID
                            let short_id = if tab.id.len() > 8 { format!("{}...", &tab.id[..8]) } else { tab.id.clone() };
                            if ui.selectable_label(is_selected, RichText::new(short_id).monospace().size(font_size)).clicked() {
                                state.selected_tab_id = Some(tab.id.clone());
                            }

                            // Col 2: Title
                            let trunc_title = if tab.title.len() > 40 { format!("{}...", &tab.title[..40]) } else { tab.title.clone() };
                            ui.label(RichText::new(trunc_title).strong().size(font_size));

                            // Col 3: URL
                            let trunc_url = if tab.url.len() > 50 { format!("{}...", &tab.url[..50]) } else { tab.url.clone() };
                            ui.label(RichText::new(trunc_url).italics().color(Color32::GRAY).size(font_size));
                            
                            ui.end_row();
                        }
                    });
            }
        });
    });

    ui.add_space(10.0);

    // Step 3: Available Actions
    ui.group(|ui| {
        ui.label(RichText::new("Step 3: Available Actions").strong().size(16.0));
        ui.add_space(5.0);
        
        let can_capture = state.selected_tab_id.is_some() && state.scrape_path.is_some();
        
        let btn = ui.add_enabled(
            can_capture, 
            egui::Button::new(RichText::new("📸 CAPTURE (UTF-8 HTML)").size(16.0).strong())
                .min_size([ui.available_width(), 50.0].into())
        );
        
        if btn.clicked() {
            let port = state.remote_port;
            let tab_id = state.selected_tab_id.clone().unwrap();
            let root = state.scrape_path.clone().unwrap();
            
            tokio::spawn(async move {
                match crate::backend::Scraper::capture_tab_content(port, tab_id, root).await {
                    Ok(p) => tracing::info!("✅ Saved: {:?}", p),
                    Err(e) => tracing::error!("❌ Capture Failed: {}", e),
                }
            });
        }
    });
}
