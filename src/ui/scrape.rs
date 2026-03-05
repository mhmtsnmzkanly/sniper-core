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
        // KOD NOTU: Browser Control paneli iyice daraltıldı (≈%8).
        cols[0].set_width(total * 0.08);
        cols[1].set_width(total * 0.92);

        // Browser Control
        frame_style.show(&mut cols[0], |ui| {
            ui.label(
                RichText::new("Browser Control")
                    .strong()
                    .size(13.0)
                    .color(design::ACCENT_ORANGE),
            );
            ui.add_space(2.0);

            // KOD NOTU: Ayarlar dikeyde de minimize edildi (100px) ve metinler kısaltıldı.
            egui::ScrollArea::vertical().id_salt("browser_settings_scroll").max_height(100.0).show(ui, |ui| {
                ui.style_mut().override_text_style = Some(egui::TextStyle::Small);
                egui::Grid::new("browser_config_grid")
                    .spacing([4.0, 4.0])
                    .show(ui, |ui| {
                        ui.label("Bin:");
                        ui.add(egui::TextEdit::singleline(&mut state.config.chrome_binary_path));
                        ui.end_row();

                        ui.label("Prof:");
                        ui.add(egui::TextEdit::singleline(&mut state.config.chrome_profile_path));
                        ui.end_row();

                        ui.label("Port:");
                        ui.add(egui::DragValue::new(&mut state.config.remote_debug_port));
                        ui.end_row();

                        ui.label("Proxy:");
                        ui.add(egui::TextEdit::singleline(&mut state.config.proxy_server).hint_text("host:port"));
                        ui.end_row();

                        ui.label("UA:");
                        ui.add(egui::TextEdit::singleline(&mut state.config.user_agent).hint_text("Default"));
                        ui.end_row();

                        ui.label("Rand:");
                        ui.vertical(|ui| {
                            ui.checkbox(&mut state.config.randomize_user_agent, "UA");
                            ui.checkbox(&mut state.config.randomize_fingerprint, "FP");
                        });
                        ui.end_row();
                    });
            });

            ui.add_space(4.0);
            let compact_btn_h = design::BUTTON_HEIGHT - 10.0;
            if !state.is_browser_running {
                if ui
                    .add(
                        egui::Button::new(RichText::new("LAUNCH BROWSER").strong())
                            .min_size([ui.available_width(), compact_btn_h].into())
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
                            .min_size([ui.available_width(), compact_btn_h].into())
                            .fill(Color32::from_rgb(255, 80, 80)),
                    )
                    .clicked()
                {
                    tracing::info!("[UI] Click: TERMINATE BROWSER");
                    emit(AppEvent::TerminateBrowser);
                }
                ui.add_space(4.0);
                if ui
                    .add(
                        egui::Button::new(RichText::new("RELAUNCH APPLY NETWORK PROFILE").strong())
                            .min_size([ui.available_width(), compact_btn_h].into())
                            .fill(Color32::from_rgb(80, 126, 190)),
                    )
                    .on_hover_text("Apply updated Proxy / UA / Fingerprint settings by restarting browser automatically.")
                    .clicked()
                {
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
                    emit(AppEvent::TerminateBrowser);
                    tokio::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(1300)).await;
                        match crate::core::browser::BrowserManager::launch(&url, &binary, &profile, port, tx, output_dir, launch_opts).await {
                            Ok(child) => emit(AppEvent::BrowserStarted(child)),
                            Err(e) => emit(AppEvent::OperationError(format!("Relaunch failed: {}", e))),
                        }
                    });
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
                ui.add_space(8.0);
                ui.add(
                    egui::Slider::new(&mut state.tabs_per_row, 1..=3)
                        .text("tabs/row")
                        .trailing_fill(true),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("REFRESH LIST").clicked() {
                        tracing::info!("[UI] Click: REFRESH TAB LIST");
                        emit(AppEvent::RequestTabRefresh);
                    }
                });
            });
            ui.add_space(8.0);

            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                if state.available_tabs.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            RichText::new("NO ACTIVE TABS DETECTED")
                                .italics()
                                .color(Color32::GRAY),
                        );
                    });
                } else {
                    let per_row = state.tabs_per_row.clamp(1, 3);
                    let mut idx = 0;
                    let spacing = 6.0;
                    egui::Grid::new("tabs_grid")
                        .num_columns(per_row)
                        .spacing([spacing, spacing])
                        .show(ui, |grid| {
                            for tab in state.available_tabs.clone() {
                                let is_selected = Some(tab.id.clone()) == state.selected_tab_id;
                                let border_col = if is_selected {
                                    Color32::from_rgb(0, 200, 120)
                                } else {
                                    Color32::from_gray(60)
                                };
                                let bg_col = if is_selected {
                                    Color32::from_rgb(26, 42, 52)
                                } else {
                                    Color32::from_gray(24)
                                };

                                grid.vertical(|ui| {
                                    let tile_width = ui.available_width();
                                    let response = Frame::group(ui.style())
                                        .stroke(Stroke::new(if is_selected { 2.0 } else { 1.0 }, border_col))
                                        .fill(bg_col)
                                        .inner_margin(8.0)
                                        .corner_radius(6.0)
                                        .show(ui, |ui| {
                                            ui.set_min_width(tile_width);
                                            ui.set_max_width(tile_width);
                                            ui.set_height(72.0);
                                            ui.vertical(|ui| {
                                                ui.add(egui::Label::new(
                                                    RichText::new(&tab.title)
                                                        .strong()
                                                        .size(12.0)
                                                        .color(if is_selected { Color32::WHITE } else { Color32::from_gray(200) })
                                                ).truncate());
                                                ui.add_space(2.0);
                                                ui.add(egui::Label::new(
                                                    RichText::new(&tab.url)
                                                        .size(design::FONT_SMALL)
                                                        .monospace()
                                                        .color(Color32::from_gray(140))
                                                ).truncate());
                                            });
                                        })
                                        .response;

                                    response.clone().on_hover_ui(|ui| {
                                        ui.label(RichText::new(&tab.title).strong());
                                        ui.label(&tab.url);
                                        if let Some(ws) = state.workspaces.get(&tab.id) {
                                            let age = (ui.input(|i| i.time) - ws.open_time).max(0.0);
                                            ui.small(format!("Workspace age: {:.0}s", age));
                                        }
                                    });

                                    if ui
                                        .interact(response.rect, response.id, egui::Sense::click())
                                        .clicked()
                                    {
                                        tracing::info!("[UI] User selected tab: {}", tab.title);
                                        state.selected_tab_id = Some(tab.id.clone());
                                    }
                                });

                                idx += 1;
                                if idx % per_row == 0 {
                                    grid.end_row();
                                }
                            }
                        });
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
            if let Some(ws) = state.selected_tab_id.as_ref().and_then(|id| state.workspaces.get(id)) {
                if ws.auto_reload_triggered {
                    ui.colored_label(design::ACCENT_ORANGE, "Reload requested...");
                }
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

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let inspector_armed = state
                    .selected_tab_id
                    .as_ref()
                    .and_then(|id| state.workspaces.get(id))
                    .map(|ws| ws.selector_inspector_armed)
                    .unwrap_or(false);
                ui.label(
                    RichText::new(if inspector_armed { "Selector Inspector: ARMED" } else { "Selector Inspector: IDLE" })
                        .color(if inspector_armed { design::ACCENT_GREEN } else { design::TEXT_MUTED })
                        .strong(),
                );
                if ui
                    .add_enabled(can_action, egui::Button::new("YAKALA (ARM)"))
                    .clicked()
                {
                    let script = r#"(() => {
                        const toSelector = (el) => {
                            if (!el || !el.tagName) return '';
                            const parts = [];
                            while (el && el.nodeType === 1 && parts.length < 6) {
                                let part = el.tagName.toLowerCase();
                                if (el.id) { part += '#' + el.id; parts.unshift(part); break; }
                                if (el.classList && el.classList.length > 0) {
                                    part += '.' + Array.from(el.classList).slice(0, 2).join('.');
                                }
                                parts.unshift(part);
                                el = el.parentElement;
                            }
                            return parts.join(' > ');
                        };
                        if (window.__sniperInspectorArmed) return 'SNIPER_SELECTOR_ARMED';
                        window.__sniperInspectorListener = (ev) => {
                            ev.preventDefault();
                            ev.stopPropagation();
                            window.__sniperPickedSelector = toSelector(ev.target);
                            window.__sniperInspectorArmed = false;
                            document.documentElement.style.cursor = '';
                            document.removeEventListener('click', window.__sniperInspectorListener, true);
                        };
                        window.__sniperPickedSelector = '';
                        window.__sniperInspectorArmed = true;
                        document.documentElement.style.cursor = 'crosshair';
                        document.addEventListener('click', window.__sniperInspectorListener, true);
                        return 'SNIPER_SELECTOR_ARMED';
                    })()"#;
                    emit(AppEvent::RequestScriptExecution(tid.clone(), script.to_string()));
                }
                if ui
                    .add_enabled(can_action, egui::Button::new("FETCH SELECTOR"))
                    .clicked()
                {
                    let script = r#"(() => {
                        const picked = window.__sniperPickedSelector || 'NONE';
                        return 'SNIPER_SELECTOR_VALUE:' + picked;
                    })()"#;
                    emit(AppEvent::RequestScriptExecution(tid.clone(), script.to_string()));
                }
                if ui
                    .add_enabled(can_action, egui::Button::new("CLEAR"))
                    .clicked()
                {
                    let script = r#"(() => {
                        if (window.__sniperInspectorListener) {
                            document.removeEventListener('click', window.__sniperInspectorListener, true);
                        }
                        window.__sniperInspectorArmed = false;
                        window.__sniperPickedSelector = '';
                        document.documentElement.style.cursor = '';
                        return 'SNIPER_SELECTOR_CLEARED';
                    })()"#;
                    emit(AppEvent::RequestScriptExecution(tid.clone(), script.to_string()));
                }
            });
        });
    });
}
