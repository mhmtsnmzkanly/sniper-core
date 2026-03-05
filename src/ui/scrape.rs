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
    ui.label(
        RichText::new("Live browser session controls, captures and tab tools")
            .small()
            .color(design::TEXT_MUTED),
    );
    ui.add_space(10.0);

    let frame_style = Frame::group(ui.style())
        .fill(design::BG_SURFACE)
        .stroke(Stroke::new(1.0, Color32::from_rgb(42, 64, 78)))
        .corner_radius(10.0)
        .inner_margin(12.0);

    // LIVE TAB SELECTION (Auto-refresh every 5s)
    let now = ui.input(|i| i.time);
    if state.is_browser_running && now - state.last_tab_refresh > 5.0 {
        state.last_tab_refresh = now;
        emit(AppEvent::RequestTabRefresh);
    }

    // KOD NOTU: Browser Control ve Chrome Tabs paneli %30 / %70 oranında yan yana gösterilir.
    ui.columns(2, |cols| {
        let total = cols[0].available_width() + cols[1].available_width();
        // KOD NOTU: Browser Control paneli daha da daraltıldı (0.18), Tab listesi genişletildi (0.82).
        cols[0].set_width(total * 0.18);
        cols[1].set_width(total * 0.82);

        // Browser Control
        frame_style.show(&mut cols[0], |ui| {
            ui.label(
                RichText::new("Browser Control")
                    .strong()
                    .color(design::ACCENT_ORANGE),
            );
            ui.add_space(8.0);

            egui::Grid::new("browser_config_grid")
                .spacing([8.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Chrome Path:");
                    ui.add(
                        egui::TextEdit::singleline(&mut state.config.chrome_binary_path)
                            .desired_width(ui.available_width() * 0.95),
                    );
                    ui.end_row();

                    ui.label("Profile Path:");
                    ui.add(
                        egui::TextEdit::singleline(&mut state.config.chrome_profile_path)
                            .desired_width(ui.available_width() * 0.95),
                    );
                    ui.end_row();

                    ui.label("Remote Port:");
                    ui.add(egui::DragValue::new(&mut state.config.remote_debug_port));
                    ui.end_row();

                    ui.label("Proxy:");
                    ui.add(
                        egui::TextEdit::singleline(&mut state.config.proxy_server)
                            .hint_text("http://host:port or socks5://host:port")
                            .desired_width(ui.available_width() * 0.95),
                    );
                    ui.end_row();

                    ui.label("User-Agent:");
                    ui.add(
                        egui::TextEdit::singleline(&mut state.config.user_agent)
                            .hint_text("Leave empty to use browser default")
                            .desired_width(ui.available_width() * 0.95),
                    );
                    ui.end_row();

                    ui.label("Launch Randomization:");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut state.config.randomize_user_agent, "Random UA");
                        ui.checkbox(&mut state.config.randomize_fingerprint, "Random Fingerprint");
                    });
                    ui.end_row();
                });

            ui.add_space(8.0);
            if !state.is_browser_running {
                if ui
                    .add(
                        egui::Button::new(RichText::new("LAUNCH BROWSER").strong())
                            .min_size([ui.available_width(), design::BUTTON_HEIGHT].into())
                            .fill(Color32::from_rgb(0, 128, 180)),
                    )
                    .clicked()
                {
                    tracing::info!("[UI] Click: LAUNCH BROWSER");
                    let url = state.config.default_launch_url.clone();
                    let binary = state.config.chrome_binary_path.clone();
                    let port = state.config.remote_debug_port;
                    let output_dir = state.config.output_dir.clone();
                    let tx = EVENT_SENDER.lock().unwrap().clone().unwrap();
                    let launch_opts = crate::core::browser::BrowserLaunchOptions {
                        proxy_server: Some(state.config.proxy_server.clone()),
                        user_agent: Some(state.config.user_agent.clone()),
                        randomize_user_agent: state.config.randomize_user_agent,
                        randomize_fingerprint: state.config.randomize_fingerprint,
                    };

                    let profile = if state.use_custom_profile {
                        let isolated_path = state.config.output_dir.join("profiles").join("isolated");
                        let _ = std::fs::create_dir_all(&isolated_path);
                        isolated_path.to_string_lossy().to_string()
                    } else {
                        state.config.chrome_profile_path.clone()
                    };

                    tokio::spawn(async move {
                        match crate::core::browser::BrowserManager::launch(&url, &binary, &profile, port, tx, output_dir, launch_opts).await {
                            Ok(child) => emit(AppEvent::BrowserStarted(child)),
                            Err(e) => {
                                tracing::error!("[CORE] Launch failed: {}", e);
                                emit(AppEvent::OperationError(format!("Launch Failed: {}", e)));
                            }
                        }
                    });

                }
            } else {
                if ui
                    .add(
                        egui::Button::new(RichText::new("TERMINATE INSTANCE").strong().color(Color32::BLACK))
                            .min_size([ui.available_width(), design::BUTTON_HEIGHT].into())
                            .fill(Color32::from_rgb(255, 80, 80)),
                    )
                    .clicked()
                {
                    tracing::info!("[UI] Click: TERMINATE BROWSER");
                    emit(AppEvent::TerminateBrowser);
                }
                ui.add_space(6.0);
                ui.label(
                    RichText::new("● BROWSER ACTIVE")
                        .monospace()
                        .color(design::ACCENT_GREEN),
                );
            }
        });

        // Chrome Tabs
        frame_style.show(&mut cols[1], |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Chrome Tabs")
                        .strong()
                        .color(design::ACCENT_ORANGE),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("REFRESH LIST").clicked() {
                        tracing::info!("[UI] Click: REFRESH TAB LIST");
                        emit(AppEvent::RequestTabRefresh);
                    }
                });
            });
            ui.add_space(8.0);

            egui::ScrollArea::vertical().max_height(170.0).show(ui, |ui| {
                if state.available_tabs.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            RichText::new("NO ACTIVE TABS DETECTED")
                                .italics()
                                .color(Color32::GRAY),
                        );
                    });
                } else {
                    for tab in state.available_tabs.clone() {
                        let is_selected = Some(tab.id.clone()) == state.selected_tab_id;
                        let border_col = if is_selected {
                            Color32::from_rgb(0, 255, 128)
                        } else {
                            Color32::from_gray(60)
                        };
                        let bg_col = if is_selected {
                            Color32::from_rgb(30, 50, 60)
                        } else {
                            Color32::from_gray(30)
                        };

                        let response = Frame::group(ui.style())
                            .stroke(Stroke::new(if is_selected { 2.0 } else { 1.0 }, border_col))
                            .fill(bg_col)
                            .inner_margin(10.0)
                            .corner_radius(6.0)
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.vertical(|ui| {
                                    // KOD NOTU: Başlık ve URL artık otomatik olarak truncate edilir (kesilir).
                                    // Tooltip ile tam içerik gösterilir.
                                    ui.add(egui::Label::new(
                                        RichText::new(&tab.title)
                                            .strong()
                                            .color(if is_selected {
                                                Color32::WHITE
                                            } else {
                                                Color32::GRAY
                                            })
                                    ).truncate());
                                    
                                    ui.add_space(2.0);
                                    
                                    ui.add(egui::Label::new(
                                        RichText::new(&tab.url)
                                            .size(design::FONT_SMALL)
                                            .monospace()
                                            .color(Color32::from_gray(150))
                                    ).truncate());
                                });
                            })
                            .response;

                        // Tooltip: Fare ile üzerine gelince tam adresi ve başlığı göster
                        response.clone().on_hover_ui(|ui| {
                            ui.label(RichText::new(&tab.title).strong());
                            ui.label(&tab.url);
                        });

                        if ui
                            .interact(response.rect, response.id, egui::Sense::click())
                            .clicked()
                        {
                            tracing::info!("[UI] User selected tab: {}", tab.title);
                            state.selected_tab_id = Some(tab.id.clone());
                        }
                        ui.add_space(6.0);
                    }
                }
            });
        });
    });

    ui.add_space(12.0);

    // COMMAND CENTER
    frame_style.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Command Center")
                    .strong()
                    .color(design::ACCENT_ORANGE),
            );
            let tid = state.selected_tab_id.clone().unwrap_or_default();
            if ui
                .add_enabled(!tid.is_empty(), egui::Button::new("FORCE RELOAD"))
                .clicked()
            {
                tracing::info!("[UI] Click: RELOAD TAB {}", tid);
                emit(AppEvent::RequestPageReload(tid.clone()));
            }
        });
        ui.add_space(10.0);

        let can_action = state.selected_tab_id.is_some();
        let tid = state.selected_tab_id.clone().unwrap_or_default();

        ui.vertical(|ui| {
            ui.columns(4, |cols| {
                let btn_h = design::BUTTON_HEIGHT + 10.0;
                if cols[0]
                    .add_enabled(
                        can_action,
                        egui::Button::new(RichText::new("📄 CAPTURE HTML").strong())
                            .min_size([cols[0].available_width(), btn_h].into()),
                    )
                    .clicked()
                {
                    tracing::info!("[UI] Click: CAPTURE HTML for {}", tid);
                    emit(AppEvent::RequestCapture(tid.clone(), "html".into()));
                }
                if cols[1]
                    .add_enabled(
                        can_action,
                        egui::Button::new(RichText::new("📦 COMPLETE").strong().color(Color32::LIGHT_BLUE))
                            .min_size([cols[1].available_width(), btn_h].into()),
                    )
                    .clicked()
                {
                    tracing::info!("[UI] Click: CAPTURE COMPLETE for {}", tid);
                    emit(AppEvent::RequestCapture(tid.clone(), "complete".into()));
                }
                if cols[2]
                    .add_enabled(
                        can_action,
                        egui::Button::new(RichText::new("🪞 MIRROR").strong().color(Color32::GOLD))
                            .min_size([cols[2].available_width(), btn_h].into()),
                    )
                    .clicked()
                {
                    tracing::info!("[UI] Click: CAPTURE MIRROR for {}", tid);
                    emit(AppEvent::RequestCapture(tid.clone(), "mirror".into()));
                }
                if cols[3]
                    .add_enabled(
                        can_action,
                        egui::Button::new(
                            RichText::new("🤖 AUTOMATION")
                                .strong()
                                .color(Color32::from_rgb(0, 255, 128)),
                        )
                        .min_size([cols[3].available_width(), btn_h].into()),
                    )
                    .clicked()
                {
                    tracing::info!("[UI] Click: OPEN AUTOMATION for {}", tid);
                    let title = state
                        .available_tabs
                        .iter()
                        .find(|t| t.id == tid)
                        .map(|t| t.title.clone())
                        .unwrap_or_else(|| "Tab".into());
                    let ws = state
                        .workspaces
                        .entry(tid.clone())
                        .or_insert_with(|| crate::state::TabWorkspace::new(tid.clone(), title));
                    ws.show_automation = true;
                }
            });

            ui.add_space(8.0);

            ui.columns(4, |cols| {
                let btn_h = design::BUTTON_HEIGHT + 10.0;
                if cols[0]
                    .add_enabled(
                        can_action,
                        egui::Button::new(RichText::new("🌐 NETWORK").strong())
                            .min_size([cols[0].available_width(), btn_h].into()),
                    )
                    .clicked()
                {
                    tracing::info!("[UI] Click: OPEN NETWORK for {}", tid);
                    let title = state
                        .available_tabs
                        .iter()
                        .find(|t| t.id == tid)
                        .map(|t| t.title.clone())
                        .unwrap_or_else(|| "Tab".into());
                    let ws = state
                        .workspaces
                        .entry(tid.clone())
                        .or_insert_with(|| crate::state::TabWorkspace::new(tid.clone(), title));
                    ws.show_network = true;
                    emit(AppEvent::RequestNetworkToggle(tid.clone(), true));
                }
                if cols[1]
                    .add_enabled(
                        can_action,
                        egui::Button::new(RichText::new("🖼 MEDIA").strong())
                            .min_size([cols[1].available_width(), btn_h].into()),
                    )
                    .clicked()
                {
                    tracing::info!("[UI] Click: OPEN MEDIA for {}", tid);
                    let title = state
                        .available_tabs
                        .iter()
                        .find(|t| t.id == tid)
                        .map(|t| t.title.clone())
                        .unwrap_or_else(|| "Tab".into());
                    let ws = state
                        .workspaces
                        .entry(tid.clone())
                        .or_insert_with(|| crate::state::TabWorkspace::new(tid.clone(), title));
                    ws.show_media = true;
                    emit(AppEvent::RequestNetworkToggle(tid.clone(), true));
                }
                if cols[2]
                    .add_enabled(
                        can_action,
                        egui::Button::new(
                            RichText::new("🍪 COOKIE")
                                .strong()
                                .color(Color32::from_rgb(255, 180, 0)),
                        )
                        .min_size([cols[2].available_width(), btn_h].into()),
                    )
                    .clicked()
                {
                    tracing::info!("[UI] Click: OPEN COOKIES for {}", tid);
                    let title = state
                        .available_tabs
                        .iter()
                        .find(|t| t.id == tid)
                        .map(|t| t.title.clone())
                        .unwrap_or_else(|| "Tab".into());
                    let ws = state
                        .workspaces
                        .entry(tid.clone())
                        .or_insert_with(|| crate::state::TabWorkspace::new(tid.clone(), title));
                    ws.show_storage = true;
                    emit(AppEvent::RequestCookies(tid.clone()));
                }
                if cols[3]
                    .add_enabled(
                        can_action,
                        egui::Button::new(RichText::new("💻 CONSOLE").strong().color(Color32::LIGHT_BLUE))
                            .min_size([cols[3].available_width(), btn_h].into()),
                    )
                    .clicked()
                {
                    tracing::info!("[UI] Click: OPEN CONSOLE for {}", tid);
                    let title = state
                        .available_tabs
                        .iter()
                        .find(|t| t.id == tid)
                        .map(|t| t.title.clone())
                        .unwrap_or_else(|| "Tab".into());
                    let ws = state
                        .workspaces
                        .entry(tid.clone())
                        .or_insert_with(|| crate::state::TabWorkspace::new(tid.clone(), title));
                    ws.show_console = true;
                    emit(AppEvent::RequestNetworkToggle(tid.clone(), true));
                }
            });
        });
    });
}
