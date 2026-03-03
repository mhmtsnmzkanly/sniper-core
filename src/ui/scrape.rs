use crate::state::AppState;
use egui::{Ui, Color32, RichText, Frame, Stroke};
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
        let _ = tx.send(event);
    }
}

pub fn render(ui: &mut Ui, state: &mut AppState) {
    ui.heading(RichText::new("SNIPER STUDIO // V1.2.1").strong().size(22.0).color(Color32::WHITE));
    ui.add_space(15.0);

    // PHASE 1: BROWSER ENVIRONMENT
    let frame_style = Frame::group(ui.style()).fill(Color32::from_gray(20)).stroke(Stroke::new(1.0, Color32::from_gray(50)));
    
    frame_style.show(ui, |ui| {
        ui.label(RichText::new(":: PHASE 1 - ENVIRONMENT").strong().color(Color32::LIGHT_BLUE));
        ui.add_space(8.0);
        
        ui.horizontal(|ui| {
            if !state.is_browser_running {
                if ui.add(egui::Button::new(RichText::new("LAUNCH BROWSER INSTANCE").strong().size(14.0))
                    .min_size([250.0, 40.0].into())
                    .fill(Color32::from_rgb(0, 100, 200))).clicked() {
                    let url = state.config.default_launch_url.clone();
                    let profile = if state.use_custom_profile { state.config.default_profile_dir.clone() } else { std::env::current_dir().unwrap().join("temp_profile") };
                    let port = state.config.remote_debug_port;
                    let ts = state.session_timestamp.clone();
                    let log_dir = state.config.output_dir.clone();
                    tokio::spawn(async move {
                        if let Ok(child) = crate::core::browser::BrowserManager::launch(&url, profile, port, log_dir, ts).await {
                            emit(AppEvent::BrowserStarted(child));
                        }
                    });
                }
            } else {
                if ui.add(egui::Button::new(RichText::new("TERMINATE INSTANCE").strong().color(Color32::BLACK))
                    .min_size([180.0, 40.0].into())
                    .fill(Color32::from_rgb(255, 80, 80))).clicked() {
                    emit(AppEvent::TerminateBrowser);
                }
                ui.add_space(10.0);
                ui.label(RichText::new(format!("PORT: {}", state.config.remote_debug_port)).monospace().color(Color32::GREEN));
            }
        });
    });

    ui.add_space(15.0);

    // PHASE 2: LIVE TAB SELECTION
    frame_style.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(":: PHASE 2 - TARGET SELECTION").strong().color(Color32::LIGHT_BLUE));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("REFRESH LIST").clicked() { emit(AppEvent::RequestTabRefresh); }
            });
        });
        ui.add_space(8.0);
        
        egui::ScrollArea::horizontal().max_height(100.0).show(ui, |ui| {
            if state.available_tabs.is_empty() {
                ui.centered_and_justified(|ui| { ui.label(RichText::new("NO ACTIVE TABS DETECTED").italics().color(Color32::GRAY)); });
            } else {
                ui.horizontal(|ui| {
                    let tabs = state.available_tabs.clone();
                    for tab in tabs {
                        let is_selected = Some(tab.id.clone()) == state.selected_tab_id;
                        let border_col = if is_selected { Color32::from_rgb(0, 255, 128) } else { Color32::from_gray(60) };
                        let bg_col = if is_selected { Color32::from_rgb(30, 50, 60) } else { Color32::from_gray(30) };

                        let response = Frame::group(ui.style())
                            .stroke(Stroke::new(if is_selected { 2.0 } else { 1.0 }, border_col))
                            .fill(bg_col)
                            .inner_margin(10.0)
                            .corner_radius(4.0)
                            .show(ui, |ui| {
                            ui.set_min_width(200.0);
                            ui.vertical(|ui| {
                                let title = if tab.title.chars().count() > 25 { tab.title.chars().take(22).collect::<String>() + "..." } else { tab.title.clone() };
                                ui.label(RichText::new(title).strong().color(if is_selected { Color32::WHITE } else { Color32::GRAY }));
                                ui.add_space(4.0);
                                ui.label(RichText::new(&tab.url).size(10.0).monospace().color(Color32::from_gray(150)));
                            });
                        }).response;

                        if ui.interact(response.rect, response.id, egui::Sense::click()).clicked() {
                            state.selected_tab_id = Some(tab.id.clone());
                            tracing::info!("[SCRAPER <-> UI] Target focused: {}", tab.title);
                        }
                        ui.add_space(8.0);
                    }
                });
            }
        });
    });

    ui.add_space(15.0);

    // PHASE 3: INTEGRATED COMMAND CENTER
    frame_style.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(":: PHASE 3 - COMMAND CENTER").strong().color(Color32::LIGHT_BLUE));
            let tid = state.selected_tab_id.clone().unwrap_or_default();
            if ui.add_enabled(!tid.is_empty(), egui::Button::new("FORCE RELOAD").small()).clicked() {
                emit(AppEvent::RequestPageReload(tid.clone()));
            }
        });
        ui.add_space(12.0);
        
        let can_action = state.selected_tab_id.is_some();
        let tid = state.selected_tab_id.clone().unwrap_or_default();

        ui.vertical(|ui| {
            ui.columns(3, |cols| {
                let btn_h = 45.0;
                if cols[0].add_enabled(can_action, egui::Button::new(RichText::new("CAPTURE HTML").strong())
                    .min_size([cols[0].available_width(), btn_h].into())).clicked() {
                    emit(AppEvent::RequestCapture(tid.clone(), false));
                }
                if cols[1].add_enabled(can_action, egui::Button::new(RichText::new("CAPTURE MIRROR").strong().color(Color32::GOLD))
                    .min_size([cols[1].available_width(), btn_h].into())).clicked() {
                    emit(AppEvent::RequestCapture(tid.clone(), true));
                }
                if cols[2].add_enabled(can_action, egui::Button::new(RichText::new("AUTOMATION").strong().color(Color32::from_rgb(0, 255, 128)))
                    .min_size([cols[2].available_width(), btn_h].into())).clicked() {
                    let title = state.available_tabs.iter().find(|t| t.id == tid).map(|t| t.title.clone()).unwrap_or_else(|| "Tab".into());
                    let ws = state.workspaces.entry(tid.clone()).or_insert_with(|| crate::state::TabWorkspace::new(tid.clone(), title));
                    ws.show_automation = true;
                }
            });

            ui.add_space(10.0);

            ui.columns(4, |cols| {
                let btn_h = 45.0;
                if cols[0].add_enabled(can_action, egui::Button::new(RichText::new("NETWORK").strong()).min_size([cols[0].available_width(), btn_h].into())).clicked() {
                    let title = state.available_tabs.iter().find(|t| t.id == tid).map(|t| t.title.clone()).unwrap_or_else(|| "Tab".into());
                    let ws = state.workspaces.entry(tid.clone()).or_insert_with(|| crate::state::TabWorkspace::new(tid.clone(), title));
                    ws.show_network = true;
                    emit(AppEvent::RequestNetworkToggle(tid.clone(), true));
                }
                if cols[1].add_enabled(can_action, egui::Button::new(RichText::new("MEDIA").strong()).min_size([cols[1].available_width(), btn_h].into())).clicked() {
                    let title = state.available_tabs.iter().find(|t| t.id == tid).map(|t| t.title.clone()).unwrap_or_else(|| "Tab".into());
                    let ws = state.workspaces.entry(tid.clone()).or_insert_with(|| crate::state::TabWorkspace::new(tid.clone(), title));
                    ws.show_media = true;
                    emit(AppEvent::RequestNetworkToggle(tid.clone(), true));
                }
                if cols[2].add_enabled(can_action, egui::Button::new(RichText::new("COOKIE").strong().color(Color32::from_rgb(255, 180, 0))).min_size([cols[2].available_width(), btn_h].into())).clicked() {
                    let title = state.available_tabs.iter().find(|t| t.id == tid).map(|t| t.title.clone()).unwrap_or_else(|| "Tab".into());
                    let ws = state.workspaces.entry(tid.clone()).or_insert_with(|| crate::state::TabWorkspace::new(tid.clone(), title));
                    ws.show_storage = true;
                    emit(AppEvent::RequestCookies(tid.clone()));
                }
                if cols[3].add_enabled(can_action, egui::Button::new(RichText::new("CONSOLE").strong().color(Color32::LIGHT_BLUE)).min_size([cols[3].available_width(), btn_h].into())).clicked() {
                    let title = state.available_tabs.iter().find(|t| t.id == tid).map(|t| t.title.clone()).unwrap_or_else(|| "Tab".into());
                    let ws = state.workspaces.entry(tid.clone()).or_insert_with(|| crate::state::TabWorkspace::new(tid.clone(), title));
                    ws.sniffer_active = true; 
                    emit(AppEvent::RequestNetworkToggle(tid.clone(), true));
                }
            });
        });
    });
}
