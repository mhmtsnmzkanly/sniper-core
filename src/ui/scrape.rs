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
    ui.heading(RichText::new("PRECISION EXTRACTION STUDIO").strong().size(20.0));
    ui.add_space(10.0);

    // ACTION 1: Browser Environment
    ui.group(|ui| {
        ui.label(RichText::new("PHASE 1: BROWSER ENVIRONMENT").strong().color(Color32::KHAKI));
        ui.add_space(5.0);
        
        ui.horizontal(|ui| {
            ui.label("Target URL:");
            ui.text_edit_singleline(&mut state.scrape_url);
            
            if !state.is_browser_running {
                if ui.button(RichText::new("🚀 LAUNCH").strong()).clicked() {
                    let url = if state.scrape_url.is_empty() { state.config.default_launch_url.clone() } else { state.scrape_url.clone() };
                    let profile = if state.use_custom_profile { 
                        state.config.default_profile_dir.clone() 
                    } else {
                        std::env::current_dir().unwrap().join("temp_profile")
                    };
                    let port = state.config.remote_debug_port;
                    let ts = state.session_timestamp.clone();
                    
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::launch(&url, profile, port, ts).await {
                            Ok(child) => emit(AppEvent::BrowserStarted(child)),
                            Err(e) => emit(AppEvent::OperationError(e.to_string())),
                        }
                    });
                }
            } else {
                if ui.button(RichText::new("🛑 TERMINATE").color(Color32::RED).strong()).clicked() {
                    emit(AppEvent::TerminateBrowser);
                }
            }
        });
    });

    ui.add_space(10.0);

    // ACTION 2: Tab Selection
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("PHASE 2: LIVE TAB SELECTION").strong().color(Color32::KHAKI));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🔄 REFRESH").clicked() {
                    emit(AppEvent::RequestTabRefresh);
                }
            });
        });

        ui.add_space(8.0);
        
        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
            if state.available_tabs.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("No active tabs found. Launch browser first.").italics().color(Color32::GRAY));
                });
            } else {
                let total_width = ui.available_width();
                let id_width = 80.0;
                let select_width = 100.0;
                let info_width = total_width - id_width - select_width - 40.0;

                egui::Grid::new("tab_grid_v6")
                    .num_columns(3)
                    .spacing([10.0, 12.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(RichText::new("ID").strong());
                        ui.label(RichText::new("PAGE INFO").strong());
                        ui.label(RichText::new("ACTION").strong());
                        ui.end_row();

                        let tabs = state.available_tabs.clone();
                        for tab in tabs {
                            let is_selected = Some(tab.id.clone()) == state.selected_tab_id;
                            
                            // ID
                            let short_id = if tab.id.len() > 8 { &tab.id[..8] } else { &tab.id };
                            ui.label(RichText::new(short_id).monospace().small().color(Color32::DARK_GRAY));

                            // Info
                            ui.allocate_ui(egui::vec2(info_width, 40.0), |ui| {
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&tab.title).strong().color(if is_selected { Color32::LIGHT_BLUE } else { Color32::WHITE }));
                                    ui.label(RichText::new(&tab.url).small().italics().color(Color32::GRAY));
                                });
                            });

                            // Select
                            if ui.selectable_label(is_selected, if is_selected { "SELECTED" } else { "SELECT" }).clicked() {
                                state.selected_tab_id = Some(tab.id.clone());
                            }
                            ui.end_row();
                        }
                    });
            }
        });
    });

    ui.add_space(10.0);

    // ACTION 3: Capture
    ui.group(|ui| {
        ui.label(RichText::new("PHASE 3: EXECUTE CAPTURE").strong().color(Color32::KHAKI));
        ui.add_space(5.0);
        
        let can_capture = state.selected_tab_id.is_some();
        
        ui.columns(2, |cols| {
            if cols[0].add_enabled(can_capture, egui::Button::new(RichText::new("📄 CAPTURE HTML ONLY").strong()).min_size([cols[0].available_width(), 45.0].into())).clicked() {
                if let Some(tid) = state.selected_tab_id.clone() {
                    emit(AppEvent::RequestCapture(tid, false));
                }
            }
            if cols[1].add_enabled(can_capture, egui::Button::new(RichText::new("🖼 CAPTURE FULL MIRROR").strong()).min_size([cols[1].available_width(), 45.0].into())).clicked() {
                if let Some(tid) = state.selected_tab_id.clone() {
                    emit(AppEvent::RequestCapture(tid, true));
                }
            }
        });
    });
}
