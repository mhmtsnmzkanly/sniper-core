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

    // Settings
    ui.group(|ui| {
        ui.columns(2, |cols| {
            cols[0].horizontal(|ui| {
                ui.label("🌐 PORT:");
                ui.add(egui::DragValue::new(&mut state.config.remote_debug_port).range(1024..=65535));
            });
            cols[1].horizontal(|ui| {
                ui.label("📁 SAVE TO:");
                if ui.button("Browse").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        state.config.raw_output_dir = path;
                        tracing::info!("Output directory changed to: {:?}", state.config.raw_output_dir);
                    }
                }
            });
        });
        ui.label(RichText::new(state.config.raw_output_dir.to_string_lossy()).small().color(Color32::LIGHT_BLUE));
    });

    ui.add_space(10.0);

    // Step 1: Browser Control
    ui.group(|ui| {
        ui.label(RichText::new("Step 1: Browser Control").strong());
        ui.horizontal(|ui| {
            ui.label("START URL:");
            ui.text_edit_singleline(&mut state.scrape_url);
            
            if !state.is_browser_running {
                if ui.button("LAUNCH BROWSER").clicked() {
                    let url = if state.scrape_url.is_empty() { state.config.default_launch_url.clone() } else { state.scrape_url.clone() };
                    tracing::info!("LAUNCH BROWSER pressed. Target: {}", url);
                    
                    let profile = state.config.default_profile_dir.clone();
                    let port = state.config.remote_debug_port;
                    let ts = state.session_timestamp.clone();
                    
                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::launch(&url, profile, port, ts).await {
                            Ok(child) => {
                                tracing::info!("Browser process spawned successfully.");
                                emit(AppEvent::BrowserStarted(child));
                            },
                            Err(e) => {
                                tracing::error!("Launch failed: {}", e);
                                emit(AppEvent::OperationError(format!("Could not launch browser: {}", e)));
                            }
                        }
                    });
                }
            } else {
                if ui.button(RichText::new("STOP BROWSER").color(Color32::RED)).clicked() {
                    tracing::warn!("STOP BROWSER pressed.");
                    emit(AppEvent::TerminateBrowser);
                }
            }
        });
    });

    ui.add_space(10.0);

    // Step 2: Tab Selection
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Step 2: Target Selection").strong().size(16.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("REFRESH LIST").clicked() {
                    tracing::info!("REFRESH LIST pressed.");
                    emit(AppEvent::RequestTabRefresh);
                }
            });
        });

        ui.add_space(8.0);
        
        egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
            if state.available_tabs.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("No tabs detected. Start Browser & Refresh.").italics().color(Color32::GRAY));
                });
            } else {
                let total_width = ui.available_width();
                let id_width = 80.0;
                let select_width = 100.0;
                let info_width = total_width - id_width - select_width - 40.0;

                egui::Grid::new("tab_grid_v5")
                    .num_columns(3)
                    .spacing([10.0, 12.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(RichText::new("ID").strong().color(Color32::KHAKI));
                        ui.label(RichText::new("PAGE INFO").strong().color(Color32::KHAKI));
                        ui.label(RichText::new("ACTION").strong().color(Color32::KHAKI));
                        ui.end_row();

                        let tabs = state.available_tabs.clone();
                        for tab in tabs {
                            let is_selected = Some(tab.id.clone()) == state.selected_tab_id;
                            let font_size = 12.0;
                            
                            let short_id = if tab.id.chars().count() > 8 { 
                                format!("{}...", tab.id.chars().take(8).collect::<String>()) 
                            } else { 
                                tab.id.clone() 
                            };
                            ui.label(RichText::new(short_id).monospace().size(font_size).color(Color32::DARK_GRAY));

                            ui.allocate_ui(egui::vec2(info_width, 40.0), |ui| {
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&tab.title).strong().size(font_size).color(if is_selected { Color32::LIGHT_BLUE } else { Color32::WHITE }));
                                    ui.label(RichText::new(&tab.url).small().italics().color(Color32::GRAY));
                                });
                            });

                            if ui.selectable_label(is_selected, if is_selected { "SELECTED" } else { "SELECT" }).clicked() {
                                state.selected_tab_id = Some(tab.id.clone());
                                tracing::info!("Tab selected: {} ({})", tab.title, tab.id);
                            }
                            
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
        
        ui.checkbox(&mut state.mirror_mode, "Mirror Mode (Download Images, CSS, JS)");
        ui.add_space(5.0);

        let can_capture = state.selected_tab_id.is_some();
        let btn_label = if state.mirror_mode { "📸 CAPTURE FULL MIRROR (UTF-8)" } else { "📸 CAPTURE HTML ONLY (UTF-8)" };
        
        let btn = ui.add_enabled(
            can_capture, 
            egui::Button::new(RichText::new(btn_label).size(16.0).strong())
                .min_size([ui.available_width(), 50.0].into())
        );
        
        if btn.clicked() {
            let tab_id = state.selected_tab_id.clone().unwrap();
            let mode_str = if state.mirror_mode { "MIRROR" } else { "HTML ONLY" };
            tracing::info!("CAPTURE started. Tab ID: {}, Mode: {}", tab_id, mode_str);
            
            emit(AppEvent::RequestCapture(tab_id, state.mirror_mode));
        }
    });
}
