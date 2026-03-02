use crate::state::{AppState, Tab, TabWorkspace};
use egui::{Ui, Color32, RichText, Frame, Vec2, Stroke};
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
    ui.heading(RichText::new("PRECISION EXTRACTION STUDIO 1.1.0").strong().size(20.0));
    ui.add_space(10.0);

    // PHASE 1: BROWSER ENVIRONMENT
    ui.group(|ui| {
        ui.label(RichText::new("PHASE 1: BROWSER ENVIRONMENT").strong().color(Color32::KHAKI));
        ui.add_space(5.0);
        
        ui.horizontal(|ui| {
            if !state.is_browser_running {
                if ui.add(egui::Button::new(RichText::new("🚀 LAUNCH BROWSER INSTANCE").strong()).min_size([250.0, 40.0].into())).clicked() {
                    let url = state.config.default_launch_url.clone();
                    let profile = if state.use_custom_profile { state.config.default_profile_dir.clone() } else { std::env::current_dir().unwrap().join("temp_profile") };
                    let port = state.config.remote_debug_port;
                    let ts = state.session_timestamp.clone();
                    tokio::spawn(async move {
                        if let Ok(child) = crate::core::browser::BrowserManager::launch(&url, profile, port, ts).await {
                            emit(AppEvent::BrowserStarted(child));
                        }
                    });
                }
            } else {
                if ui.add(egui::Button::new(RichText::new("🛑 TERMINATE").color(Color32::RED).strong()).min_size([120.0, 40.0].into())).clicked() {
                    emit(AppEvent::TerminateBrowser);
                }
                
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    let exec_path = crate::core::browser::BrowserManager::find_executable().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| "Unknown".to_string());
                    let profile_path = if state.use_custom_profile { state.config.default_profile_dir.to_string_lossy().into_owned() } else { "Temporary Isolated Profile".to_string() };
                    
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("EXE:").small().color(Color32::GRAY));
                        ui.label(RichText::new(exec_path).small().color(Color32::LIGHT_BLUE));
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("PROFILE:").small().color(Color32::GRAY));
                        ui.label(RichText::new(profile_path).small().color(Color32::LIGHT_BLUE));
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("STATUS:").small().color(Color32::GRAY));
                        ui.label(RichText::new(format!("ONLINE (Port: {})", state.config.remote_debug_port)).small().strong().color(Color32::GREEN));
                    });
                });
            }
        });
    });

    ui.add_space(10.0);

    // PHASE 2: LIVE TAB SELECTION (HORIZONTAL)
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("PHASE 2: LIVE TAB SELECTION").strong().color(Color32::KHAKI));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🔄 REFRESH LIST").clicked() { emit(AppEvent::RequestTabRefresh); }
            });
        });
        ui.add_space(8.0);

        egui::ScrollArea::horizontal().max_height(100.0).show(ui, |ui| {
            if state.available_tabs.is_empty() {
                ui.centered_and_justified(|ui| { ui.label(RichText::new("No active tabs. Launch browser first.").italics().color(Color32::GRAY)); });
            } else {
                ui.horizontal(|ui| {
                    let tabs = state.available_tabs.clone();
                    for tab in tabs {
                        let is_selected = Some(tab.id.clone()) == state.selected_tab_id;
                        
                        let stroke = if is_selected { Stroke::new(2.0, Color32::LIGHT_BLUE) } else { Stroke::new(1.0, Color32::DARK_GRAY) };
                        let bg = if is_selected { Color32::from_rgb(30, 50, 80) } else { Color32::from_rgb(40, 40, 40) };

                        let response = Frame::group(ui.style())
                            .stroke(stroke)
                            .fill(bg)
                            .inner_margin(8.0)
                            .show(ui, |ui| {
                                ui.set_min_width(180.0);
                                ui.vertical(|ui| {
                                    let title = if tab.title.chars().count() > 20 { tab.title.chars().take(17).collect::<String>() + "..." } else { tab.title.clone() };
                                    ui.label(RichText::new(title).strong().color(if is_selected { Color32::WHITE } else { Color32::GRAY }));
                                    
                                    let url_trunc = if tab.url.chars().count() > 25 { tab.url.chars().take(22).collect::<String>() + "..." } else { tab.url.clone() };
                                    ui.label(RichText::new(url_trunc).small().italics().color(Color32::DARK_GRAY));
                                });
                            }).response;

                        let response = ui.interact(response.rect, response.id, egui::Sense::click());
                        if response.clicked() {
                            state.selected_tab_id = Some(tab.id.clone());
                            tracing::info!("[SCRAPER <-> UI] Tab selected: {}", tab.title);
                        }
                        
                        if is_selected {
                            response.highlight();
                        }
                        
                        ui.add_space(5.0);
                    }
                });
            }
        });
    });

    ui.add_space(10.0);

    // PHASE 3: INTEGRATED COMMAND CENTER
    ui.group(|ui| {
        ui.label(RichText::new("PHASE 3: TARGET COMMAND CENTER").strong().color(Color32::KHAKI));
        ui.add_space(10.0);
        
        let can_action = state.selected_tab_id.is_some();
        let tid = state.selected_tab_id.clone().unwrap_or_default();

        ui.vertical(|ui| {
            ui.columns(2, |cols| {
                if cols[0].add_enabled(can_action, egui::Button::new(RichText::new("📄 CAPTURE HTML ONLY").strong()).min_size([cols[0].available_width(), 45.0].into())).clicked() {
                    emit(AppEvent::RequestCapture(tid.clone(), false));
                }
                if cols[1].add_enabled(can_action, egui::Button::new(RichText::new("🖼 CAPTURE FULL MIRROR").strong()).min_size([cols[1].available_width(), 45.0].into())).clicked() {
                    emit(AppEvent::RequestCapture(tid.clone(), true));
                }
            });

            ui.add_space(8.0);

            ui.columns(3, |cols| {
                if cols[0].add_enabled(can_action, egui::Button::new(RichText::new("🌐 NETWORK").strong().color(Color32::LIGHT_GRAY)).min_size([cols[0].available_width(), 40.0].into())).clicked() {
                    let title = state.available_tabs.iter().find(|t| t.id == tid).map(|t| t.title.clone()).unwrap_or_else(|| "Tab".into());
                    let ws = state.workspaces.entry(tid.clone()).or_insert_with(|| TabWorkspace::new(tid.clone(), title));
                    ws.show_network = true;
                }
                if cols[1].add_enabled(can_action, egui::Button::new(RichText::new("🖼 MEDIA").strong().color(Color32::LIGHT_GRAY)).min_size([cols[1].available_width(), 40.0].into())).clicked() {
                    let title = state.available_tabs.iter().find(|t| t.id == tid).map(|t| t.title.clone()).unwrap_or_else(|| "Tab".into());
                    let ws = state.workspaces.entry(tid.clone()).or_insert_with(|| TabWorkspace::new(tid.clone(), title));
                    ws.show_media = true;
                }
                if cols[2].add_enabled(can_action, egui::Button::new(RichText::new("📦 STORAGE").strong().color(Color32::LIGHT_GRAY)).min_size([cols[2].available_width(), 40.0].into())).clicked() {
                    let title = state.available_tabs.iter().find(|t| t.id == tid).map(|t| t.title.clone()).unwrap_or_else(|| "Tab".into());
                    let ws = state.workspaces.entry(tid.clone()).or_insert_with(|| TabWorkspace::new(tid.clone(), title));
                    ws.show_storage = true;
                }
            });
        });

        if !can_action {
            ui.add_space(5.0);
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new("⚠ Select a tab above to enable Command Center.").small().color(Color32::YELLOW));
            });
        }
    });
}
