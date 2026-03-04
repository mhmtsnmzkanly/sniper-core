use crate::state::{AppState, AutomationStep, AutomationStatus};
use egui::{Ui, Color32, RichText, Frame, Stroke};
use egui_extras::{TableBuilder, Column};
use crate::core::events::AppEvent;
use crate::ui::scrape::emit;

pub fn render_embedded(ui: &mut Ui, state: &mut AppState, tid: &str) {
    if !state.workspaces.contains_key(tid) { return; }
    
    let (mut auto_steps, mut auto_status, discovered_selectors, mut selector_search, mut variables, mut var_key, mut var_val, extracted_data) = {
        let ws = state.workspaces.get(tid).unwrap();
        (ws.auto_steps.clone(), ws.auto_status.clone(), ws.discovered_selectors.clone(), ws.selector_search.clone(), ws.variables.clone(), ws.var_edit_key.clone(), ws.var_edit_val.clone(), ws.extracted_data.clone())
    };

    ui.columns(2, |cols| {
        cols[0].vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(":: PIPELINE").strong().color(Color32::KHAKI));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("SCAN").clicked() { emit(AppEvent::RequestPageSelectors(tid.to_string())); }
                    ui.menu_button("➕ ADD", |ui| {
                        if ui.button("🌐 Nav").clicked() { auto_steps.push(AutomationStep::Navigate("https://".into())); ui.close_menu(); }
                        if ui.button("🖱 Click").clicked() { auto_steps.push(AutomationStep::Click("".into())); ui.close_menu(); }
                        if ui.button("🖱 R-Click").clicked() { auto_steps.push(AutomationStep::RightClick("".into())); ui.close_menu(); }
                        if ui.button("🖐 Hover").clicked() { auto_steps.push(AutomationStep::Hover("".into())); ui.close_menu(); }
                        if ui.button("⌨ Type").clicked() { auto_steps.push(AutomationStep::Type { selector: "".into(), value: "".into(), is_variable: false }); ui.close_menu(); }
                        ui.separator();
                        if ui.button("⏳ Wait").clicked() { auto_steps.push(AutomationStep::Wait(1)); ui.close_menu(); }
                        if ui.button("🔍 Wait Sel").clicked() { auto_steps.push(AutomationStep::WaitSelector { selector: "".into(), timeout_ms: 5000 }); ui.close_menu(); }
                        if ui.button("💤 Wait Idle").clicked() { auto_steps.push(AutomationStep::WaitUntilIdle { timeout_ms: 2000 }); ui.close_menu(); }
                        if ui.button("📡 Wait Net").clicked() { auto_steps.push(AutomationStep::WaitNetworkIdle { timeout_ms: 10000, min_idle_ms: 500 }); ui.close_menu(); }
                        ui.separator();
                        if ui.button("🧪 Ext").clicked() { auto_steps.push(AutomationStep::Extract { selector: "".into(), as_key: "data".into(), add_to_dataset: true }); ui.close_menu(); }
                        if ui.button("🆕 New Row").clicked() { auto_steps.push(AutomationStep::NewRow); ui.close_menu(); }
                        if ui.button("🔁 Loop").clicked() { auto_steps.push(AutomationStep::ForEach { selector: "".into(), body: vec![] }); ui.close_menu(); }
                        if ui.button("❓ If").clicked() { auto_steps.push(AutomationStep::If { selector: "".into(), then_steps: vec![] }); ui.close_menu(); }
                        if ui.button("🖼 Frame").clicked() { auto_steps.push(AutomationStep::SwitchFrame("".into())); ui.close_menu(); }
                        ui.separator();
                        if ui.button("📸 Screen").clicked() { auto_steps.push(AutomationStep::Screenshot("capture.png".into())); ui.close_menu(); }
                        if ui.button("💾 Export").clicked() { auto_steps.push(AutomationStep::Export("result.json".into())); ui.close_menu(); }
                        if ui.button("📜 Scroll").clicked() { auto_steps.push(AutomationStep::ScrollBottom); ui.close_menu(); }
                    });
                });
            });
            ui.add_space(5.0);
            
            let mut delete_idx = None;
            let mut move_from = None;
            let mut move_to = None;

            egui::ScrollArea::vertical()
                .id_salt("auto_steps_scroll")
                .auto_shrink([false, false])
                .max_height(ui.available_height() * 0.6)
                .show(ui, |ui| {
                for (idx, step) in auto_steps.iter_mut().enumerate() {
                    let item_id = egui::Id::new(("step", idx));
                    let _dnd_res = ui.dnd_drag_source(item_id, idx, |ui| {
                        render_step_block(ui, step, idx, &mut delete_idx, &discovered_selectors, &mut selector_search, &variables);
                    });
                    if let Some(payload) = ui.dnd_drop_zone::<usize, _>(Frame::NONE, |_ui| { }).1 {
                        move_from = Some(*payload); move_to = Some(idx);
                    }
                }
            });

            if let (Some(from), Some(to)) = (move_from, move_to) {
                if from != to { let item = auto_steps.remove(from); auto_steps.insert(to, item); }
            }
            if let Some(idx) = delete_idx { auto_steps.remove(idx); }
        });

        cols[1].vertical(|ui| {
            ui.group(|ui| {
                ui.label(RichText::new(":: VARIABLES").strong().color(Color32::LIGHT_BLUE));
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut var_key).hint_text("Key").desired_width(ui.available_width() * 0.4));
                    ui.add(egui::TextEdit::singleline(&mut var_val).hint_text("Val").desired_width(ui.available_width() * 0.4));
                    if ui.button("+").clicked() && !var_key.is_empty() {
                        variables.insert(var_key.clone(), var_val.clone());
                        var_key.clear(); var_val.clear();
                    }
                });
                egui::ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
                    let mut to_remove = None;
                    for (k, v) in &variables {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(k).small().color(Color32::GOLD));
                            ui.label(RichText::new(v).small());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("x").clicked() { to_remove = Some(k.clone()); }
                            });
                        });
                    }
                    if let Some(k) = to_remove { variables.remove(&k); }
                });
            });

            ui.add_space(10.0);

            ui.group(|ui| {
                ui.label(RichText::new(":: DATASET PREVIEW").strong().color(Color32::GREEN));
                ui.add_space(5.0);
                
                if extracted_data.is_empty() {
                    ui.label("No data captured yet.");
                } else {
                    let keys: Vec<String> = extracted_data[0].keys().cloned().collect();
                    let table = TableBuilder::new(ui)
                        .striped(true)
                        .resizable(true)
                        .auto_shrink([false, false])
                        .column(Column::auto().at_least(30.0)) // ID
                        .columns(Column::remainder(), keys.len());

                    table.header(20.0, |mut header| {
                        header.col(|ui| { ui.strong("#"); });
                        for k in &keys {
                            header.col(|ui| { ui.strong(k); });
                        }
                    })
                    .body(|body| {
                        body.rows(20.0, extracted_data.len(), |mut row| {
                            let idx = row.index();
                            let data_row = &extracted_data[idx];
                            row.col(|ui| { ui.label(format!("{}", idx + 1)); });
                            for k in &keys {
                                row.col(|ui| {
                                    let val = data_row.get(k).cloned().unwrap_or_default();
                                    ui.label(RichText::new(val).small());
                                });
                            }
                        });
                    });
                }
            });
        });
    });

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        let is_running = matches!(auto_status, AutomationStatus::Running(_));
        let btn_text = match &auto_status {
            AutomationStatus::Running(i) => format!("🏃 RUNNING STEP {}...", i + 1),
            AutomationStatus::Finished => "✅ FINISHED (RUN AGAIN)".into(),
            AutomationStatus::Error(_) => "❌ ERROR (RETRY)".into(),
            _ => "▶ START EXECUTION".into(),
        };

        if let Some(ws) = state.workspaces.get_mut(tid) {
            if ui.add_enabled(!is_running && !auto_steps.is_empty(), 
                egui::Button::new(RichText::new(btn_text).strong()).min_size([200.0, 40.0].into()))
                .clicked() {
                ws.auto_status = AutomationStatus::Running(0);
                emit(AppEvent::RequestAutomationRun(tid.to_string(), auto_steps.clone(), ws.auto_config.clone()));
            }
        }

        ui.separator();
        
        if let Some(ws) = state.workspaces.get_mut(tid) {
            ui.menu_button("⚙ SETTINGS", |ui| {
                ui.checkbox(&mut ws.auto_config.screenshot_on_error, "📸 Screenshot on Error");
                ui.horizontal(|ui| {
                    ui.label("Retry Attempts:");
                    ui.add(egui::DragValue::new(&mut ws.auto_config.retry_attempts).range(0..=10));
                });
                ui.horizontal(|ui| {
                    ui.label("Step Timeout (ms):");
                    ui.add(egui::DragValue::new(&mut ws.auto_config.step_timeout_ms).range(1000..=60000).speed(100));
                });
            });
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(RichText::new("🗑 CLEAR").color(Color32::LIGHT_RED)).clicked() {
                auto_steps.clear();
            }
            if ui.button("📁 LOAD DSL").clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() {
                    if let Ok(content) = std::fs::read_to_string(path) {
                        match serde_json::from_str::<crate::core::automation::dsl::AutomationDsl>(&content) {
                            Ok(dsl) => {
                                auto_steps = map_dsl_to_steps(dsl.steps);
                                for step in &auto_steps {
                                    if let AutomationStep::SetVariable { key, value } = step {
                                        variables.insert(key.clone(), value.clone());
                                    }
                                }
                                tracing::info!("[AUTO-UI] DSL loaded: {} steps", auto_steps.len());
                            }
                            Err(e) => {
                                tracing::error!("[AUTO-UI] DSL Parse Error: {}", e);
                            }
                        }
                    }
                }
            }
            if ui.button("💾 SAVE DSL").clicked() {
                let dsl = map_steps_to_dsl(&auto_steps);
                if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).save_file() {
                    if let Ok(json) = serde_json::to_string_pretty(&dsl) { let _ = std::fs::write(path, json); }
                }
            }
        });
    });

    if let Some(ws) = state.workspaces.get_mut(tid) {
        ws.auto_steps = auto_steps;
        ws.auto_status = auto_status;
        ws.selector_search = selector_search;
        ws.variables = variables;
        ws.var_edit_key = var_key;
        ws.var_edit_val = var_val;
    }
}

fn render_step_block(ui: &mut Ui, step: &mut AutomationStep, idx: usize, delete_idx: &mut Option<usize>, discovered: &[String], search: &mut String, vars: &std::collections::HashMap<String, String>) {
    let (color, title) = match step {
        AutomationStep::Navigate(_) | AutomationStep::Click(_) | AutomationStep::RightClick(_) | AutomationStep::Hover(_) | AutomationStep::Type { .. } => (Color32::from_rgb(80, 130, 255), "ACT"),
        AutomationStep::Wait(_) | AutomationStep::WaitSelector { .. } | AutomationStep::WaitUntilIdle { .. } | AutomationStep::WaitNetworkIdle { .. } => (Color32::from_rgb(255, 200, 50), "W8"),
        AutomationStep::If { .. } | AutomationStep::ForEach { .. } | AutomationStep::SwitchFrame(_) => (Color32::from_rgb(255, 100, 50), "CTRL"),
        _ => (Color32::from_rgb(50, 220, 120), "DATA"),
    };

    Frame::new().fill(color.gamma_multiply(0.1)).stroke(Stroke::new(1.0, color)).inner_margin(8.0).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("⣿").color(color));
            ui.label(RichText::new(title).small().strong().color(color));
            
            match step {
                AutomationStep::Navigate(url) => { ui.add(egui::TextEdit::singleline(url).desired_width(ui.available_width() * 0.7)); }
                AutomationStep::Click(sel) => { selector_input(ui, sel, discovered, search); }
                AutomationStep::RightClick(sel) => { selector_input(ui, sel, discovered, search); }
                AutomationStep::Hover(sel) => { selector_input(ui, sel, discovered, search); }
                AutomationStep::Type { selector, value, is_variable } => {
                    if *is_variable {
                        egui::ComboBox::from_id_salt(format!("v_{}", idx)).selected_text(value.as_str()).show_ui(ui, |ui| {
                            for k in vars.keys() { ui.selectable_value(value, k.clone(), k); }
                        });
                    } else { ui.add(egui::TextEdit::singleline(value).desired_width(80.0)); }
                    if ui.button(if *is_variable { "V" } else { "T" }).clicked() { *is_variable = !*is_variable; }
                    selector_input(ui, selector, discovered, search);
                }
                AutomationStep::Extract { selector, as_key, add_to_dataset } => {
                    selector_input(ui, selector, discovered, search);
                    ui.add(egui::TextEdit::singleline(as_key).desired_width(60.0));
                    ui.checkbox(add_to_dataset, "DB");
                }
                AutomationStep::SetVariable { key, value } => {
                    ui.add(egui::TextEdit::singleline(key).desired_width(60.0));
                    ui.label("=");
                    ui.add(egui::TextEdit::singleline(value).desired_width(80.0));
                }
                AutomationStep::Wait(secs) => { ui.add(egui::DragValue::new(secs)); ui.label("s"); }
                AutomationStep::WaitSelector { selector, .. } => { selector_input(ui, selector, discovered, search); }
                AutomationStep::WaitUntilIdle { .. } => { ui.label("Idle"); }
                AutomationStep::WaitNetworkIdle { timeout_ms, min_idle_ms } => {
                    ui.label("NetIdle");
                    ui.add(egui::DragValue::new(timeout_ms)).on_hover_text("Max timeout ms");
                    ui.label("/");
                    ui.add(egui::DragValue::new(min_idle_ms)).on_hover_text("Min quiet period ms");
                }
                AutomationStep::Export(f) => { ui.label("To:"); ui.add(egui::TextEdit::singleline(f).desired_width(80.0)); }
                AutomationStep::Screenshot(f) => { ui.label("To:"); ui.add(egui::TextEdit::singleline(f).desired_width(80.0)); }
                AutomationStep::NewRow => { ui.label("NEW ROW"); }
                AutomationStep::ScrollBottom => { ui.label("BOTTOM"); }
                AutomationStep::SwitchFrame(sel) => { ui.label("Frame"); selector_input(ui, sel, discovered, search); }
                AutomationStep::If { selector, .. } => { ui.label("If"); selector_input(ui, selector, discovered, search); }
                AutomationStep::ForEach { selector, .. } => { ui.label("Each"); selector_input(ui, selector, discovered, search); }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("x").clicked() { *delete_idx = Some(idx); }
            });
        });
    });
}

fn selector_input(ui: &mut Ui, value: &mut String, discovered: &[String], search: &mut String) {
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(value).desired_width(ui.available_width() * 0.4));
        ui.menu_button("Q", |ui| {
            ui.add(egui::TextEdit::singleline(search));
            let filter = search.to_lowercase();
            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                for s in discovered {
                    if filter.is_empty() || s.to_lowercase().contains(&filter) {
                        if ui.button(RichText::new(s).small()).clicked() { *value = s.clone(); ui.close_menu(); }
                    }
                }
            });
        });
    });
}

fn map_steps_to_dsl(steps: &[AutomationStep]) -> crate::core::automation::dsl::AutomationDsl {
    crate::core::automation::dsl::AutomationDsl {
        dsl_version: 1,
        steps: steps.iter().map(|s| match s {
            AutomationStep::Navigate(u) => crate::core::automation::dsl::Step::Navigate { url: u.clone() },
            AutomationStep::Click(sel) => crate::core::automation::dsl::Step::Click { selector: sel.clone() },
            AutomationStep::RightClick(sel) => crate::core::automation::dsl::Step::RightClick { selector: sel.clone() },
            AutomationStep::Hover(sel) => crate::core::automation::dsl::Step::Hover { selector: sel.clone() },
            AutomationStep::Type { selector, value, is_variable } => crate::core::automation::dsl::Step::Type { selector: selector.clone(), value: value.clone(), is_variable: *is_variable },
            AutomationStep::Wait(secs) => crate::core::automation::dsl::Step::Wait { seconds: *secs },
            AutomationStep::WaitSelector { selector, timeout_ms } => crate::core::automation::dsl::Step::WaitSelector { selector: selector.clone(), timeout_ms: *timeout_ms },
            AutomationStep::WaitUntilIdle { timeout_ms } => crate::core::automation::dsl::Step::WaitUntilIdle { timeout_ms: *timeout_ms },
            AutomationStep::WaitNetworkIdle { timeout_ms, min_idle_ms } => crate::core::automation::dsl::Step::WaitNetworkIdle { timeout_ms: *timeout_ms, min_idle_ms: *min_idle_ms },
            AutomationStep::ScrollBottom => crate::core::automation::dsl::Step::ScrollBottom,
            AutomationStep::Extract { selector, as_key, add_to_dataset } => crate::core::automation::dsl::Step::Extract { selector: selector.clone(), as_key: as_key.clone(), add_to_row: *add_to_dataset },
            AutomationStep::SetVariable { key, value } => crate::core::automation::dsl::Step::SetVariable { key: key.clone(), value: value.clone() },
            AutomationStep::NewRow => crate::core::automation::dsl::Step::NewRow,
            AutomationStep::Export(f) => crate::core::automation::dsl::Step::Export { filename: f.clone() },
            AutomationStep::Screenshot(f) => crate::core::automation::dsl::Step::Screenshot { filename: f.clone() },
            AutomationStep::SwitchFrame(sel) => crate::core::automation::dsl::Step::SwitchFrame { selector: sel.clone() },
            AutomationStep::If { selector, then_steps } => crate::core::automation::dsl::Step::If { selector: selector.clone(), then_steps: map_steps_to_dsl(then_steps).steps },
            AutomationStep::ForEach { selector, body } => crate::core::automation::dsl::Step::ForEach { selector: selector.clone(), body: map_steps_to_dsl(body).steps },
        }).collect(),
    }
}

fn map_dsl_to_steps(steps: Vec<crate::core::automation::dsl::Step>) -> Vec<AutomationStep> {
    steps.into_iter().map(|s| match s {
        crate::core::automation::dsl::Step::Navigate { url } => AutomationStep::Navigate(url),
        crate::core::automation::dsl::Step::Click { selector } => AutomationStep::Click(selector),
        crate::core::automation::dsl::Step::RightClick { selector } => AutomationStep::RightClick(selector),
        crate::core::automation::dsl::Step::Hover { selector } => AutomationStep::Hover(selector),
        crate::core::automation::dsl::Step::Type { selector, value, is_variable } => AutomationStep::Type { selector, value, is_variable },
        crate::core::automation::dsl::Step::Wait { seconds } => AutomationStep::Wait(seconds),
        crate::core::automation::dsl::Step::WaitSelector { selector, timeout_ms } => AutomationStep::WaitSelector { selector, timeout_ms },
        crate::core::automation::dsl::Step::WaitUntilIdle { timeout_ms } => AutomationStep::WaitUntilIdle { timeout_ms },
        crate::core::automation::dsl::Step::WaitNetworkIdle { timeout_ms, min_idle_ms } => AutomationStep::WaitNetworkIdle { timeout_ms, min_idle_ms },
        crate::core::automation::dsl::Step::Extract { selector, as_key, add_to_row } => AutomationStep::Extract { selector, as_key, add_to_dataset: add_to_row },
        crate::core::automation::dsl::Step::SetVariable { key, value } => AutomationStep::SetVariable { key, value },
        crate::core::automation::dsl::Step::NewRow => AutomationStep::NewRow,
        crate::core::automation::dsl::Step::Export { filename } => AutomationStep::Export(filename),
        crate::core::automation::dsl::Step::Screenshot { filename } => AutomationStep::Screenshot(filename),
        crate::core::automation::dsl::Step::SwitchFrame { selector } => AutomationStep::SwitchFrame(selector),
        crate::core::automation::dsl::Step::If { selector, then_steps } => AutomationStep::If { selector, then_steps: map_dsl_to_steps(then_steps) },
        crate::core::automation::dsl::Step::ForEach { selector, body } => AutomationStep::ForEach { selector, body: map_dsl_to_steps(body) },
        crate::core::automation::dsl::Step::ScrollBottom => AutomationStep::ScrollBottom,
    }).collect()
}
