use crate::state::AppState;
use egui::{Ui, Color32, RichText};
use tokio::sync::mpsc;
use crate::core::events::AppEvent;
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
    ui.heading("SNIPER SCRAPER STUDIO 1.0.0");
    ui.add_space(10.0);

    // Config Section
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Target Port:").strong());
            ui.add(egui::DragValue::new(&mut state.config.remote_debug_port).range(1024..=65535));
            ui.add_space(20.0);
            ui.label(RichText::new("Output:").strong());
            ui.label(RichText::new(state.config.raw_output_dir.to_string_lossy()).small().color(Color32::LIGHT_BLUE));
        });
    });

    ui.add_space(10.0);

    // Step 1: Browser Launch
    ui.group(|ui| {
        ui.label(RichText::new("Step 1: Browser Control").strong().size(14.0));
        ui.horizontal(|ui| {
            ui.label("URL:");
            ui.text_edit_singleline(&mut state.scrape_url);
            
            if !state.is_browser_running {
                if ui.button("LAUNCH BROWSER").clicked() {
                    let url = if state.scrape_url.is_empty() { state.config.default_launch_url.clone() } else { state.scrape_url.clone() };
                    let profile = state.config.default_profile_dir.clone();
                    let port = state.config.remote_debug_port;
                    let ts = state.session_timestamp.clone();
                    
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::launch(&url, profile, port, ts).await {
                            Ok(child) => emit(AppEvent::BrowserStarted(child)),
                            Err(e) => {
                                tracing::error!("Launch failed: {}", e);
                                emit(AppEvent::OperationError(e.to_string()));
                            }
                        }
                    });
                }
            } else {
                if ui.button(RichText::new("STOP BROWSER").color(Color32::RED)).clicked() {
                    emit(AppEvent::BrowserTerminated);
                }
            }
        });
    });

    ui.add_space(10.0);

    // Step 2: Tab Selection
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Step 2: Select Target Tab").strong().size(14.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("REFRESH LIST").clicked() {
                    emit(AppEvent::RequestTabRefresh);
                }
            });
        });

        ui.add_space(8.0);
        
        egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
            if state.available_tabs.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("Waiting for tabs... (Auto-refresh active)").italics().color(Color32::GRAY));
                });
            } else {
                let column_width = (ui.available_width() - 40.0) / 3.0;
                egui::Grid::new("tab_grid_phase1")
                    .num_columns(3)
                    .min_col_width(column_width)
                    .max_col_width(column_width)
                    .spacing([15.0, 12.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(RichText::new("ID").strong().color(Color32::KHAKI));
                        ui.label(RichText::new("TITLE").strong().color(Color32::KHAKI));
                        ui.label(RichText::new("URL").strong().color(Color32::KHAKI));
                        ui.end_row();

                        let tabs = state.available_tabs.clone();
                        for tab in tabs {
                            let is_selected = Some(tab.id.clone()) == state.selected_tab_id;
                            let font_size = 12.0;
                            
                            // Col 1: ID
                            let short_id = if tab.id.chars().count() > 8 { 
                                format!("{}...", tab.id.chars().take(8).collect::<String>()) 
                            } else { 
                                tab.id.clone() 
                            };

                            if ui.selectable_label(is_selected, RichText::new(short_id).monospace().size(font_size)).clicked() {
                                state.selected_tab_id = Some(tab.id.clone());
                            }

                            // Col 2: Title
                            let trunc_title = if tab.title.chars().count() > 40 { 
                                format!("{}...", tab.title.chars().take(40).collect::<String>()) 
                            } else { 
                                tab.title.clone() 
                            };
                            ui.label(RichText::new(trunc_title).strong().size(font_size));

                            // Col 3: URL
                            let trunc_url = if tab.url.chars().count() > 50 { 
                                format!("{}...", tab.url.chars().take(50).collect::<String>()) 
                            } else { 
                                tab.url.clone() 
                            };
                            ui.label(RichText::new(trunc_url).italics().color(Color32::GRAY).size(font_size));
                            
                            ui.end_row();
                        }
                    });
            }
        });
    });

    ui.add_space(10.0);

    // Step 3: Actions
    ui.group(|ui| {
        ui.label(RichText::new("Step 3: Available Actions").strong().size(14.0));
        ui.add_space(5.0);
        
        ui.checkbox(&mut state.mirror_mode, "Mirror Mode (Download Images, CSS, JS)");
        ui.add_space(5.0);

        let can_capture = state.selected_tab_id.is_some();
        let btn_label = if state.mirror_mode { "📸 CAPTURE FULL MIRROR (UTF-8)" } else { "📸 CAPTURE HTML ONLY (UTF-8)" };
        
        let btn = ui.add_enabled(
            can_capture, 
            egui::Button::new(RichText::new(btn_label).size(15.0).strong())
                .min_size([ui.available_width(), 45.0].into())
        );
        
        if btn.clicked() {
            if let Some(tab_id) = state.selected_tab_id.clone() {
                emit(AppEvent::RequestCapture(tab_id, state.mirror_mode));
            }
        }
    });
}
