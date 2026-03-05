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
    ui.add_space(8.0);

    let panel_stroke = Stroke::new(1.0, Color32::from_rgb(42, 64, 78));

    // LIVE TAB SELECTION (Auto-refresh every 5s)
    let now = ui.input(|i| i.time);
    if state.is_browser_running && now - state.last_tab_refresh > 5.0 {
        state.last_tab_refresh = now;
        emit(AppEvent::RequestTabRefresh);
    }

    // ── Browser Control + Chrome Tabs ── responsive 20/80 split
    let available = ui.available_width();
    // sütun genişliklerini kullanılabilir alana göre orantılı göster
    let ctrl_w = (available * 0.22).clamp(180.0, 260.0);
    let tabs_w = available - ctrl_w - ui.spacing().item_spacing.x;

    ui.horizontal(|ui| {
        // ── Browser Control ──────────────────────────────────────────
        Frame::group(ui.style())
            .fill(design::BG_SURFACE)
            .stroke(panel_stroke)
            .corner_radius(10.0)
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.set_width(ctrl_w);
                ui.label(
                    RichText::new("Browser Control")
                        .strong()
                        .size(12.0)
                        .color(design::ACCENT_ORANGE),
                );
                ui.add_space(4.0);

                egui::ScrollArea::vertical()
                    .id_salt("browser_settings_scroll")
                    .max_height(160.0)
                    .show(ui, |ui| {
                        ui.style_mut().override_text_style = Some(egui::TextStyle::Small);
                        egui::Grid::new("browser_config_grid")
                            .spacing([4.0, 3.0])
                            .show(ui, |ui| {
                                ui.label("Bin:");
                                ui.add(
                                    egui::TextEdit::singleline(
                                        &mut state.config.chrome_binary_path,
                                    )
                                    .desired_width(f32::INFINITY),
                                );
                                ui.end_row();

                                ui.label("Prof:");
                                ui.add(
                                    egui::TextEdit::singleline(
                                        &mut state.config.chrome_profile_path,
                                    )
                                    .desired_width(f32::INFINITY),
                                );
                                ui.end_row();

                                ui.label("Port:");
                                ui.add(egui::DragValue::new(
                                    &mut state.config.remote_debug_port,
                                ));
                                ui.end_row();

                                ui.label("Proxy:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut state.config.proxy_server)
                                        .hint_text("host:port")
                                        .desired_width(f32::INFINITY),
                                );
                                ui.end_row();

                                ui.label("UA:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut state.config.user_agent)
                                        .hint_text("Default")
                                        .desired_width(f32::INFINITY),
                                );
                                ui.end_row();

                                ui.label("Rand:");
                                ui.vertical(|ui| {
                                    ui.checkbox(&mut state.config.randomize_user_agent, "UA");
                                    ui.checkbox(&mut state.config.randomize_fingerprint, "FP");
                                });
                                ui.end_row();
                            });
                    });

                ui.add_space(6.0);
                let btn_h = design::BUTTON_HEIGHT;
                let btn_w = ui.available_width();
                if !state.is_browser_running {
                    if ui
                        .add(
                            egui::Button::new(RichText::new("LAUNCH BROWSER").strong())
                                .min_size([btn_w, btn_h].into())
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
                            let isolated_path = state
                                .config
                                .output_dir
                                .join("profiles")
                                .join("isolated");
                            let _ = std::fs::create_dir_all(&isolated_path);
                            isolated_path.to_string_lossy().to_string()
                        } else {
                            state.config.chrome_profile_path.clone()
                        };

                        tokio::spawn(async move {
                            match crate::core::browser::BrowserManager::launch(
                                &url,
                                &binary,
                                &profile,
                                port,
                                tx,
                                output_dir,
                                launch_opts,
                            )
                            .await
                            {
                                Ok(child) => emit(AppEvent::BrowserStarted(child)),
                                Err(e) => {
                                    tracing::error!("[CORE] Launch failed: {}", e);
                                    emit(AppEvent::OperationError(format!(
                                        "Launch Failed: {}",
                                        e
                                    )));
                                }
                            }
                        });
                    }
                } else {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("TERMINATE").strong().color(Color32::BLACK),
                            )
                            .min_size([btn_w, btn_h].into())
                            .fill(Color32::from_rgb(255, 80, 80)),
                        )
                        .clicked()
                    {
                        tracing::info!("[UI] Click: TERMINATE BROWSER");
                        emit(AppEvent::TerminateBrowser);
                    }
                    ui.add_space(3.0);
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("RELAUNCH + PROFILE").strong().size(11.0),
                            )
                            .min_size([btn_w, btn_h - 4.0].into())
                            .fill(Color32::from_rgb(80, 126, 190)),
                        )
                        .on_hover_text(
                            "Apply updated Proxy / UA / Fingerprint by restarting browser.",
                        )
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
                            let isolated_path = state
                                .config
                                .output_dir
                                .join("profiles")
                                .join("isolated");
                            let _ = std::fs::create_dir_all(&isolated_path);
                            isolated_path.to_string_lossy().to_string()
                        } else {
                            state.config.chrome_profile_path.clone()
                        };
                        emit(AppEvent::TerminateBrowser);
                        tokio::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_millis(1300)).await;
                            match crate::core::browser::BrowserManager::launch(
                                &url,
                                &binary,
                                &profile,
                                port,
                                tx,
                                output_dir,
                                launch_opts,
                            )
                            .await
                            {
                                Ok(child) => emit(AppEvent::BrowserStarted(child)),
                                Err(e) => {
                                    emit(AppEvent::OperationError(format!("Relaunch failed: {}", e)))
                                }
                            }
                        });
                    }
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("● BROWSER ACTIVE")
                            .monospace()
                            .size(11.0)
                            .color(design::ACCENT_GREEN),
                    );
                }
            });

        // ── Chrome Tabs ───────────────────────────────────────────────
        Frame::group(ui.style())
            .fill(design::BG_SURFACE)
            .stroke(panel_stroke)
            .corner_radius(10.0)
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.set_width(tabs_w);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Chrome Tabs")
                            .strong()
                            .color(design::ACCENT_ORANGE),
                    );
                    ui.add_space(6.0);
                    ui.add(
                        egui::Slider::new(&mut state.tabs_per_row, 1..=4)
                            .text("cols")
                            .trailing_fill(true),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("REFRESH").clicked() {
                            tracing::info!("[UI] Click: REFRESH TAB LIST");
                            emit(AppEvent::RequestTabRefresh);
                        }
                    });
                });
                ui.add_space(6.0);

                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .id_salt("tab_tiles_scroll")
                    .show(ui, |ui| {
                        if state.available_tabs.is_empty() {
                            ui.centered_and_justified(|ui| {
                                ui.label(
                                    RichText::new("NO ACTIVE TABS DETECTED")
                                        .italics()
                                        .color(Color32::GRAY),
                                );
                            });
                        } else {
                            let per_row = state.tabs_per_row.clamp(1, 4);
                            let spacing = 5.0;
                            let col_w = ((ui.available_width()
                                - spacing * (per_row as f32 - 1.0))
                                / per_row as f32)
                                .max(120.0);
                            let mut idx = 0;
                            egui::Grid::new("tabs_grid")
                                .num_columns(per_row)
                                .spacing([spacing, spacing])
                                .show(ui, |grid| {
                                    for tab in state.available_tabs.clone() {
                                        let is_selected =
                                            Some(tab.id.clone()) == state.selected_tab_id;
                                        let border_col = if is_selected {
                                            Color32::from_rgb(0, 200, 120)
                                        } else {
                                            Color32::from_gray(55)
                                        };
                                        let bg_col = if is_selected {
                                            Color32::from_rgb(22, 38, 48)
                                        } else {
                                            Color32::from_gray(22)
                                        };

                                        grid.vertical(|ui| {
                                            let response = Frame::group(ui.style())
                                                .stroke(Stroke::new(
                                                    if is_selected { 2.0 } else { 1.0 },
                                                    border_col,
                                                ))
                                                .fill(bg_col)
                                                .inner_margin(6.0)
                                                .corner_radius(6.0)
                                                .show(ui, |ui| {
                                                    ui.set_width(col_w);
                                                    ui.set_height(64.0);
                                                    ui.vertical(|ui| {
                                                        ui.add(
                                                            egui::Label::new(
                                                                RichText::new(&tab.title)
                                                                    .strong()
                                                                    .size(11.0)
                                                                    .color(if is_selected {
                                                                        Color32::WHITE
                                                                    } else {
                                                                        Color32::from_gray(200)
                                                                    }),
                                                            )
                                                            .truncate(),
                                                        );
                                                        ui.add_space(2.0);
                                                        ui.add(
                                                            egui::Label::new(
                                                                RichText::new(&tab.url)
                                                                    .size(9.0)
                                                                    .monospace()
                                                                    .color(Color32::from_gray(
                                                                        120,
                                                                    )),
                                                            )
                                                            .truncate(),
                                                        );
                                                    });
                                                })
                                                .response;

                                            response.clone().on_hover_ui(|ui| {
                                                ui.label(
                                                    RichText::new(&tab.title).strong(),
                                                );
                                                ui.label(&tab.url);
                                                if let Some(ws) =
                                                    state.workspaces.get(&tab.id)
                                                {
                                                    let age = (ui.input(|i| i.time)
                                                        - ws.open_time)
                                                        .max(0.0);
                                                    ui.small(format!(
                                                        "Age: {:.0}s",
                                                        age
                                                    ));
                                                }
                                            });

                                            if ui
                                                .interact(
                                                    response.rect,
                                                    response.id,
                                                    egui::Sense::click(),
                                                )
                                                .clicked()
                                            {
                                                tracing::info!(
                                                    "[UI] User selected tab: {}",
                                                    tab.title
                                                );
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

    ui.add_space(10.0);

    // ── Command Center ────────────────────────────────────────────────
    Frame::group(ui.style())
        .fill(design::BG_SURFACE)
        .stroke(panel_stroke)
        .corner_radius(10.0)
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Command Center")
                        .strong()
                        .color(design::ACCENT_ORANGE),
                );
                let tid = state.selected_tab_id.clone().unwrap_or_default();
                if ui
                    .add_enabled(!tid.is_empty(), egui::Button::new("⟳ RELOAD"))
                    .clicked()
                {
                    tracing::info!("[UI] Click: RELOAD TAB {}", tid);
                    emit(AppEvent::RequestPageReload(tid.clone()));
                }
                if let Some(ws) = state
                    .selected_tab_id
                    .as_ref()
                    .and_then(|id| state.workspaces.get(id))
                {
                    if ws.auto_reload_triggered {
                        ui.colored_label(design::ACCENT_ORANGE, "Reload requested...");
                    }
                }
            });

            ui.add_space(8.0);

            let can_action = state.selected_tab_id.is_some();
            let tid = state.selected_tab_id.clone().unwrap_or_default();

            // KOD NOTU: Her buton için min genişlik hesaplanarak responsiveness sağlanır.
            // Sabit sütun sayısı yerine wrap_ui ile otomatik satır geçişi yapılır.
            ui.horizontal_wrapped(|ui| {
                let btn_h = design::BUTTON_HEIGHT;
                let min_w = 110.0_f32;


                // Row 1: Capture group
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("CAPTURE")
                            .size(10.0)
                            .color(design::TEXT_MUTED),
                    );
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add_enabled(
                                can_action,
                                egui::Button::new(
                                    RichText::new("📄 HTML").strong(),
                                )
                                .min_size([min_w, btn_h].into()),
                            )
                            .clicked()
                        {
                            tracing::info!("[UI] Click: CAPTURE HTML for {}", tid);
                            emit(AppEvent::RequestCapture(tid.clone(), "html".into()));
                        }
                        if ui
                            .add_enabled(
                                can_action,
                                egui::Button::new(
                                    RichText::new("📦 COMPLETE")
                                        .strong()
                                        .color(Color32::LIGHT_BLUE),
                                )
                                .min_size([min_w, btn_h].into()),
                            )
                            .clicked()
                        {
                            tracing::info!("[UI] Click: CAPTURE COMPLETE for {}", tid);
                            emit(AppEvent::RequestCapture(tid.clone(), "complete".into()));
                        }
                        if ui
                            .add_enabled(
                                can_action,
                                egui::Button::new(
                                    RichText::new("🪞 MIRROR")
                                        .strong()
                                        .color(Color32::GOLD),
                                )
                                .min_size([min_w, btn_h].into()),
                            )
                            .clicked()
                        {
                            tracing::info!("[UI] Click: CAPTURE MIRROR for {}", tid);
                            emit(AppEvent::RequestCapture(tid.clone(), "mirror".into()));
                        }
                    });
                });

                ui.separator();

                // Row 2: Panel open group
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("PANELS")
                            .size(10.0)
                            .color(design::TEXT_MUTED),
                    );
                    ui.horizontal_wrapped(|ui| {
                        let panels = [
                            ("🌐 NETWORK", Color32::WHITE, "network"),
                            ("🖼 MEDIA", Color32::WHITE, "media"),
                            ("🍪 COOKIE", Color32::from_rgb(255, 180, 0), "cookie"),
                            ("💻 CONSOLE", Color32::LIGHT_BLUE, "console"),
                            ("🤖 AUTOMATION", Color32::from_rgb(0, 255, 128), "automation"),
                        ];
                        for (label, color, panel) in panels {
                            if ui
                                .add_enabled(
                                    can_action,
                                    egui::Button::new(
                                        RichText::new(label).strong().color(color),
                                    )
                                    .min_size([min_w, btn_h].into()),
                                )
                                .clicked()
                            {
                                tracing::info!("[UI] Click: OPEN {} for {}", panel, tid);
                                let title = state
                                    .available_tabs
                                    .iter()
                                    .find(|t| t.id == tid)
                                    .map(|t| t.title.clone())
                                    .unwrap_or_else(|| "Tab".into());
                                let ws = state
                                    .workspaces
                                    .entry(tid.clone())
                                    .or_insert_with(|| {
                                        crate::state::TabWorkspace::new(
                                            tid.clone(),
                                            title,
                                        )
                                    });
                                match panel {
                                    "network" => {
                                        ws.show_network = true;
                                        emit(AppEvent::RequestNetworkToggle(
                                            tid.clone(),
                                            true,
                                        ));
                                    }
                                    "media" => {
                                        ws.show_media = true;
                                        emit(AppEvent::RequestNetworkToggle(
                                            tid.clone(),
                                            true,
                                        ));
                                    }
                                    "cookie" => {
                                        ws.show_storage = true;
                                        emit(AppEvent::RequestCookies(tid.clone()));
                                    }
                                    "console" => {
                                        ws.show_console = true;
                                        emit(AppEvent::RequestNetworkToggle(
                                            tid.clone(),
                                            true,
                                        ));
                                    }
                                    "automation" => {
                                        ws.show_automation = true;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    });
                });
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // ── Blob De-Mask ──────────────────────────────────────────
            // KOD NOTU: Blob De-Masker butonu — blob: URL'lerini gerçek medya URL'lerine dönüştürür.
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        can_action,
                        egui::Button::new(
                            RichText::new("🔍 BLOB DE-MASK")
                                .strong()
                                .color(Color32::from_rgb(255, 200, 50)),
                        )
                        .min_size([0.0, design::BUTTON_HEIGHT].into()),
                    )
                    .on_hover_text(
                        "Resolve blob: media URLs by scanning page network activity",
                    )
                    .clicked()
                {
                    tracing::info!("[UI] Click: BLOB DE-MASK for {}", tid);
                    emit(AppEvent::RequestBlobDemask(tid.clone()));
                }
            });

            ui.add_space(6.0);

            // ── Selector Inspector ────────────────────────────────────
            let inspector_armed = state
                .selected_tab_id
                .as_ref()
                .and_then(|id| state.workspaces.get(id))
                .map(|ws| ws.selector_inspector_armed)
                .unwrap_or(false);

            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(if inspector_armed {
                        "Selector Inspector: ARMED"
                    } else {
                        "Selector Inspector: IDLE"
                    })
                    .color(if inspector_armed {
                        design::ACCENT_GREEN
                    } else {
                        design::TEXT_MUTED
                    })
                    .strong(),
                );
                if ui
                    .add_enabled(can_action, egui::Button::new("ARM"))
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
                    emit(AppEvent::RequestScriptExecution(
                        tid.clone(),
                        script.to_string(),
                    ));
                }
                if ui
                    .add_enabled(can_action, egui::Button::new("FETCH"))
                    .clicked()
                {
                    let script = r#"(() => {
                        const picked = window.__sniperPickedSelector || 'NONE';
                        return 'SNIPER_SELECTOR_VALUE:' + picked;
                    })()"#;
                    emit(AppEvent::RequestScriptExecution(
                        tid.clone(),
                        script.to_string(),
                    ));
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
                    emit(AppEvent::RequestScriptExecution(
                        tid.clone(),
                        script.to_string(),
                    ));
                }
            });
        });
}
