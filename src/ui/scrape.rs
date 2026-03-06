use crate::state::AppState;
use crate::ui::design;
use egui::{Color32, Frame, RichText, Stroke, Ui};
use lazy_static::lazy_static;
use std::sync::Mutex;
use tokio::sync::mpsc;

use crate::core::events::AppEvent;

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
    design::title(ui, "Operations Deck", design::ACCENT_CYAN);
    ui.add_space(4.0);

    let panel_stroke = Stroke::new(1.0, Color32::from_rgb(42, 64, 78));
    let full_w = ui.available_width();

    // ── ROW 1: BROWSER LAUNCHER ───────────────────────────────────────
    Frame::group(ui.style())
        .fill(design::BG_SURFACE)
        .stroke(panel_stroke)
        .corner_radius(8.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.set_width(full_w);
            ui.horizontal(|ui| {
                ui.label(RichText::new("🚀 BROWSER CONTROL").strong().color(design::ACCENT_ORANGE));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if state.is_browser_running {
                        ui.label(RichText::new("● ONLINE").monospace().strong().color(design::ACCENT_GREEN));
                    } else {
                        ui.label(RichText::new("○ OFFLINE").monospace().color(Color32::GRAY));
                    }
                });
            });
            ui.separator();
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                // Settings Grid
                ui.vertical(|ui| {
                    egui::Grid::new("browser_mini_grid").spacing([8.0, 4.0]).show(ui, |ui| {
                        ui.label("Path:"); 
                        ui.add(egui::TextEdit::singleline(&mut state.config.chrome_binary_path).desired_width(200.0));
                        ui.label("Port:"); 
                        ui.add(egui::DragValue::new(&mut state.config.remote_debug_port));
                        ui.end_row();

                        ui.label("Proxy:"); 
                        ui.add(egui::TextEdit::singleline(&mut state.config.proxy_server).desired_width(200.0));
                        ui.checkbox(&mut state.config.randomize_fingerprint, "Stealth Mode");
                        ui.end_row();

                        ui.label("UA:");
                        ui.add(egui::TextEdit::singleline(&mut state.config.user_agent).desired_width(200.0));
                        ui.checkbox(&mut state.config.randomize_user_agent, "Random UA");
                        ui.end_row();
                    });
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let btn_h = 40.0;
                    if !state.is_browser_running {
                        if ui.add(egui::Button::new(RichText::new("LAUNCH BROWSER").strong().size(14.0))
                            .min_size([180.0, btn_h].into())
                            .fill(Color32::from_rgb(0, 110, 170))).clicked() {
                            launch_browser(state);
                        }
                    } else {
                        if ui.add(egui::Button::new(RichText::new("TERMINATE").strong().size(14.0))
                            .min_size([180.0, btn_h].into())
                            .fill(Color32::from_rgb(255, 80, 80))).clicked() {
                            emit(AppEvent::TerminateBrowser);
                        }
                    }
                });
            });
        });

    ui.add_space(12.0);

    // ── ROW 2: CHROME TABS ────────────────────────────────────────────
    Frame::group(ui.style())
        .fill(design::BG_SURFACE)
        .stroke(panel_stroke)
        .corner_radius(8.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.set_width(full_w);
            ui.horizontal(|ui| {
                ui.label(RichText::new("📑 CHROME TABS").strong().color(design::ACCENT_ORANGE));
                ui.add_space(10.0);
                if state.is_browser_running {
                    ui.add(egui::Slider::new(&mut state.tabs_per_row, 1..=6).text("Columns"));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add_enabled(state.is_browser_running, egui::Button::new("⟳ SYNC TABS")).clicked() {
                        emit(AppEvent::RequestTabRefresh);
                    }
                });
            });
            ui.separator();
            ui.add_space(4.0);

            let scroll_h = 240.0;
            egui::ScrollArea::vertical()
                .max_height(scroll_h)
                .id_salt("tab_scroll_row")
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if !state.is_browser_running {
                        ui.centered_and_justified(|ui| {
                            ui.label(RichText::new("BROWSER NOT CONNECTED\nTabs will appear here after launch").italics().color(Color32::GRAY));
                        });
                    } else if state.available_tabs.is_empty() {
                        ui.centered_and_justified(|ui| {
                            ui.vertical_centered(|ui| {
                                ui.spinner();
                                ui.add_space(8.0);
                                ui.label(RichText::new(format!("Searching for active targets on port {}", state.config.remote_debug_port)).color(Color32::KHAKI));
                                if ui.button("Retry Force Sync").clicked() { emit(AppEvent::RequestTabRefresh); }
                            });
                        });
                    } else {
                        let per_row = state.tabs_per_row.clamp(1, 6);
                        let spacing = 8.0;
                        let avail_w = ui.available_width();
                        let col_w = ((avail_w - (per_row as f32 - 1.0) * spacing) / per_row as f32).max(100.0);
                        
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(spacing, spacing);
                            for tab in state.available_tabs.iter() {
                                let is_selected = Some(tab.id.clone()) == state.selected_tab_id;
                                let (border_col, bg_col) = if is_selected { 
                                    (design::ACCENT_GREEN, Color32::from_rgb(30, 50, 60)) 
                                } else { 
                                    (Color32::from_gray(50), design::BG_PRIMARY) 
                                };
                                
                                let res = Frame::group(ui.style())
                                    .stroke(Stroke::new(if is_selected { 2.0 } else { 1.0 }, border_col))
                                    .fill(bg_col)
                                    .inner_margin(8.0)
                                    .corner_radius(6.0)
                                    .show(ui, |ui| {
                                        ui.set_width(col_w);
                                        ui.set_height(64.0);
                                        ui.vertical(|ui| {
                                            ui.add(egui::Label::new(RichText::new(&tab.title).strong().size(11.0).color(Color32::WHITE)).truncate());
                                            ui.add(egui::Label::new(RichText::new(&tab.url).size(9.0).color(Color32::from_gray(140))).truncate());
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Max), |ui| {
                                                ui.label(RichText::new(&tab.id[..8.min(tab.id.len())]).size(7.0).color(Color32::from_gray(80)).monospace());
                                            });
                                        });
                                    }).response;

                                let click_res = ui.interact(res.rect, res.id, egui::Sense::click());
                                if click_res.clicked() {
                                    state.selected_tab_id = Some(tab.id.clone());
                                    tracing::info!("[UI] Selected tab: {}", tab.id);
                                }
                                res.on_hover_text(format!("{}\n{}", tab.title, tab.url));
                            }
                        });
                    }
                });
        });

    ui.add_space(12.0);

    // ── ROW 3: COMMAND CENTER ──────────────────────────────────────────
    render_command_center(ui, state, panel_stroke);
}

fn launch_browser(state: &AppState) {
    let tx = EVENT_SENDER.lock().unwrap().clone().expect("Event sender not initialized");
    let url = state.config.default_launch_url.clone();
    let binary = state.config.chrome_binary_path.clone();
    let profile = if state.use_custom_profile { 
        state.config.output_dir.join("profiles").join("isolated").to_string_lossy().to_string() 
    } else { 
        state.config.chrome_profile_path.clone() 
    };
    let port = state.config.remote_debug_port;
    let output_dir = state.config.output_dir.clone();

    let launch_opts = crate::core::browser::BrowserLaunchOptions {
        proxy_server: Some(state.config.proxy_server.clone()),
        user_agent: Some(state.config.user_agent.clone()),
        randomize_user_agent: state.config.randomize_user_agent,
        randomize_fingerprint: state.config.randomize_fingerprint,
    };

    tokio::spawn(async move {
         let r = crate::core::browser::BrowserManager::launch(
            &url, &binary, &profile, port, tx, output_dir, launch_opts
         ).await;
         match r {
             Ok(child) => emit(AppEvent::BrowserStarted(child)),
             Err(e) => emit(AppEvent::OperationError(e.to_string()))
         }
    });
}

fn render_command_center(ui: &mut Ui, state: &mut AppState, panel_stroke: Stroke) {
    Frame::group(ui.style())
        .fill(design::BG_SURFACE)
        .stroke(panel_stroke)
        .corner_radius(10.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Command Center").strong().color(design::ACCENT_ORANGE));
                let tid = state.selected_tab_id.clone().unwrap_or_default();
                if ui.add_enabled(!tid.is_empty(), egui::Button::new("⟳ RELOAD")).clicked() {
                    emit(AppEvent::RequestPageReload(tid.clone()));
                }
            });
            ui.add_space(8.0);

            let can_action = state.selected_tab_id.is_some();
            let tid = state.selected_tab_id.clone().unwrap_or_default();

            ui.horizontal_wrapped(|ui| {
                let btn_h = design::BUTTON_HEIGHT;
                let min_w = 120.0_f32;

                // CAPTURE Group
                ui.vertical(|ui| {
                    ui.label(RichText::new("CAPTURE").size(10.0).color(design::TEXT_MUTED));
                    ui.horizontal(|ui| {
                        if ui.add_enabled(can_action, egui::Button::new(RichText::new("📄 HTML").strong()).min_size([min_w, btn_h].into())).clicked() {
                            emit(AppEvent::RequestCapture(tid.clone(), "html".into()));
                        }
                        if ui.add_enabled(can_action, egui::Button::new(RichText::new("📦 COMPLETE").strong().color(Color32::LIGHT_BLUE)).min_size([min_w, btn_h].into())).clicked() {
                            emit(AppEvent::RequestCapture(tid.clone(), "complete".into()));
                        }
                        if ui.add_enabled(can_action, egui::Button::new(RichText::new("🪞 MIRROR").strong().color(Color32::KHAKI)).min_size([min_w, btn_h].into())).clicked() {
                            emit(AppEvent::RequestCapture(tid.clone(), "mirror".into()));
                        }
                    });
                });

                ui.separator();

                // PANELS Group
                ui.vertical(|ui| {
                    ui.label(RichText::new("PANELS").size(10.0).color(design::TEXT_MUTED));
                    ui.horizontal_wrapped(|ui| {
                        let panels = [
                            ("🌐 NETWORK", Color32::WHITE, "network"),
                            ("🖼 MEDIA", Color32::WHITE, "media"),
                            ("🍪 COOKIE", Color32::from_rgb(255, 180, 0), "cookie"),
                            ("💻 CONSOLE", Color32::LIGHT_BLUE, "console"),
                        ];
                        for (label, color, panel) in panels {
                            if ui.add_enabled(can_action, egui::Button::new(RichText::new(label).strong().color(color)).min_size([min_w, btn_h].into())).clicked() {
                                open_panel(state, tid.clone(), panel);
                            }
                        }
                    });
                });
            });
        });
}

fn open_panel(state: &mut AppState, tid: String, panel: &str) {
    let title = state.available_tabs.iter().find(|t| t.id == tid).map(|t| t.title.clone()).unwrap_or_else(|| "Tab".into());
    let ws = state.workspaces.entry(tid.clone()).or_insert_with(|| crate::state::TabWorkspace::new(tid.clone(), title));
    
    match panel {
        "network" => { ws.show_network = true; emit(AppEvent::RequestNetworkToggle(tid, true)); }
        "media" => { ws.show_media = true; emit(AppEvent::RequestNetworkToggle(tid, true)); }
        "cookie" => { ws.show_storage = true; emit(AppEvent::RequestCookies(tid)); }
        "console" => { ws.show_console = true; emit(AppEvent::RequestNetworkToggle(tid, true)); }
        _ => {}
    }
}
