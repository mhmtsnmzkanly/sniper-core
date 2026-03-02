use crate::state::{AppState, AutomationStep, AutomationStatus};
use egui::{Ui, Color32, RichText, Frame, Stroke, CornerRadius, vec2};
use crate::core::events::AppEvent;
use crate::ui::scrape::emit;

pub fn render_embedded(ui: &mut Ui, state: &mut AppState, tid: &str) {
    if !state.workspaces.contains_key(tid) { return; }
    
    let (mut auto_steps, mut auto_status, discovered_selectors, mut selector_search, mut variables, mut var_key, mut var_val, extracted_data) = {
        let ws = state.workspaces.get(tid).unwrap();
        (ws.auto_steps.clone(), ws.auto_status.clone(), ws.discovered_selectors.clone(), ws.selector_search.clone(), ws.variables.clone(), ws.var_edit_key.clone(), ws.var_edit_val.clone(), ws.extracted_data.clone())
    };

    // --- LEFT PANEL: PIPELINE & VARIABLES ---
    ui.columns(2, |cols| {
        // COLUMN 0: Pipeline Builder
        cols[0].group(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(":: PIPELINE").strong().color(Color32::KHAKI));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("SCAN").clicked() { emit(AppEvent::RequestPageSelectors(tid.to_string())); }
                    ui.menu_button("➕ ADD", |ui| {
                        ui.set_min_width(150.0);
                        if ui.button("🌐 Nav").clicked() { auto_steps.push(AutomationStep::Navigate("https://".into())); ui.close_menu(); }
                        if ui.button("🖱 Click").clicked() { auto_steps.push(AutomationStep::Click("".into())); ui.close_menu(); }
                        if ui.button("⌨ Type").clicked() { auto_steps.push(AutomationStep::Type { selector: "".into(), value: "".into(), use_variable: false }); ui.close_menu(); }
                        ui.separator();
                        if ui.button("⏳ Wait").clicked() { auto_steps.push(AutomationStep::Wait(1)); ui.close_menu(); }
                        if ui.button("🔍 Wait Sel").clicked() { auto_steps.push(AutomationStep::WaitSelector("".into())); ui.close_menu(); }
                        if ui.button("💤 Wait Idle").clicked() { auto_steps.push(AutomationStep::WaitUntilIdle); ui.close_menu(); }
                        ui.separator();
                        if ui.button("🧪 Ext").clicked() { auto_steps.push(AutomationStep::Extract { selector: "".into(), as_key: "data".into(), add_to_dataset: true }); ui.close_menu(); }
                        if ui.button("🔁 Loop").clicked() { auto_steps.push(AutomationStep::ForEach { selector: "".into(), body: vec![] }); ui.close_menu(); }
                        ui.separator();
                        if ui.button("📸 Screenshot").clicked() { auto_steps.push(AutomationStep::Screenshot("capture.png".into())); ui.close_menu(); }
                        if ui.button("💾 Export").clicked() { auto_steps.push(AutomationStep::Export("result.json".into())); ui.close_menu(); }
                        if ui.button("📜 Scroll").clicked() { auto_steps.push(AutomationStep::ScrollBottom); ui.close_menu(); }
                    });
                });
            });
            ui.add_space(5.0);
            
            let mut delete_idx = None;
            let mut move_from = None;
            let mut move_to = None;

            egui::ScrollArea::vertical().max_height(400.0).id_salt("auto_steps_scroll").show(ui, |ui| {
                if auto_steps.is_empty() { ui.label("No blocks added."); }
                for (idx, step) in auto_steps.iter_mut().enumerate() {
                    let item_id = egui::Id::new(("step", idx));
                    let dnd_res = ui.dnd_drag_source(item_id, idx, |ui| {
                        render_step_block(ui, step, idx, &mut delete_idx, &discovered_selectors, &mut selector_search, &variables);
                    });
                    if let Some(payload) = ui.dnd_drop_zone::<usize, _>(Frame::NONE, |ui| { }).1 {
                        move_from = Some(*payload); move_to = Some(idx);
                    }
                }
            });

            if let (Some(from), Some(to)) = (move_from, move_to) {
                if from != to { let item = auto_steps.remove(from); auto_steps.insert(to, item); }
            }
            if let Some(idx) = delete_idx { auto_steps.remove(idx); }
        });

        // COLUMN 1: Variables & Dataset
        cols[1].vertical(|ui| {
            ui.group(|ui| {
                ui.label(RichText::new(":: VARIABLES").strong().color(Color32::LIGHT_BLUE));
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut var_key).hint_text("Key").desired_width(80.0));
                    ui.add(egui::TextEdit::singleline(&mut var_val).hint_text("Val").desired_width(100.0));
                    if ui.button("+").clicked() && !var_key.is_empty() {
                        variables.insert(var_key.clone(), var_val.clone());
                        var_key.clear(); var_val.clear();
                    }
                });
                egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
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
                egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
                    if extracted_data.is_empty() {
                        ui.label("No data captured yet.");
                    } else {
                        for (i, row) in extracted_data.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(format!("{}.", i + 1));
                                for (k, v) in row {
                                    ui.label(RichText::new(format!("{}:", k)).small().color(Color32::GRAY));
                                    ui.label(RichText::new(v).small());
                                }
                            });
                        }
                    }
                });
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

        if ui.add_enabled(!is_running && !auto_steps.is_empty(), 
            egui::Button::new(RichText::new(btn_text).strong()).min_size([200.0, 40.0].into()))
            .clicked() {
            auto_status = AutomationStatus::Running(0);
            emit(AppEvent::RequestAutomationRun(tid.to_string(), auto_steps.clone()));
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("📁 LOAD DSL").clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() {
                    if let Ok(content) = std::fs::read_to_string(path) {
                        match serde_json::from_str::<crate::core::automation::dsl::AutomationDsl>(&content) {
                            Ok(dsl) => {
                                // Extract initial variables from DSL set_variable steps
                                for step in &dsl.steps {
                                    if let crate::core::automation::dsl::Step::SetVariable { key, value } = step {
                                        variables.insert(key.clone(), value.clone());
                                    }
                                }
                                auto_steps = map_dsl_to_steps(dsl.steps);
                                tracing::info!("[AUTO-UI] Successfully loaded {} steps.", auto_steps.len());
                            },
                            Err(e) => {
                                tracing::error!("[AUTO-UI] Load failed: {}", e);
                                // We could trigger a notification here
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
        AutomationStep::Navigate(_) | AutomationStep::Click(_) | AutomationStep::Type { .. } => (Color32::from_rgb(80, 130, 255), "ACT"),
        AutomationStep::Wait(_) | AutomationStep::WaitSelector(_) | AutomationStep::WaitUntilIdle => (Color32::from_rgb(255, 200, 50), "W8"),
        AutomationStep::If { .. } | AutomationStep::ForEach { .. } => (Color32::from_rgb(255, 100, 50), "CTRL"),
        _ => (Color32::from_rgb(50, 220, 120), "DATA"),
    };

    Frame::new().fill(color.gamma_multiply(0.1)).stroke(Stroke::new(1.0, color)).inner_margin(8.0).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("⣿").color(color));
            ui.label(RichText::new(title).small().strong().color(color));
            
            match step {
                AutomationStep::Navigate(url) => { ui.add(egui::TextEdit::singleline(url).desired_width(150.0)); }
                AutomationStep::Click(sel) => { selector_input(ui, sel, discovered, search); }
                AutomationStep::Type { selector, value, use_variable } => {
                    if *use_variable {
                        egui::ComboBox::from_id_salt(format!("v_{}", idx)).selected_text(value.as_str()).show_ui(ui, |ui| {
                            for k in vars.keys() { ui.selectable_value(value, k.clone(), k); }
                        });
                    } else { ui.add(egui::TextEdit::singleline(value).desired_width(80.0)); }
                    if ui.button(if *use_variable { "V" } else { "T" }).clicked() { *use_variable = !*use_variable; }
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
                    ui.add(egui::TextEdit::singleline(value).desired_width(100.0));
                }
                AutomationStep::Screenshot(f) => { ui.label("To:"); ui.add(egui::TextEdit::singleline(f).desired_width(100.0)); }
                AutomationStep::Wait(secs) => { ui.add(egui::DragValue::new(secs)); ui.label("s"); }
                AutomationStep::WaitSelector(sel) => { selector_input(ui, sel, discovered, search); }
                AutomationStep::WaitUntilIdle => { ui.label("Until Network Idle"); }
                AutomationStep::Export(f) => { ui.label("To:"); ui.add(egui::TextEdit::singleline(f).desired_width(100.0)); }
                AutomationStep::NewRow => { ui.label("START NEW ROW"); }
                AutomationStep::ScrollBottom => { ui.label("TO BOTTOM"); }
                _ => { ui.label("Other Step"); }
            }
            if ui.button("x").clicked() { *delete_idx = Some(idx); }
        });
    });
}

fn selector_input(ui: &mut Ui, value: &mut String, discovered: &[String], search: &mut String) {
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(value).desired_width(120.0));
        ui.menu_button("Q", |ui| {
            ui.add(egui::TextEdit::singleline(search));
            let filter = search.to_lowercase();
            for s in discovered {
                if filter.is_empty() || s.to_lowercase().contains(&filter) {
                    if ui.button(RichText::new(s).small()).clicked() { *value = s.clone(); ui.close_menu(); }
                }
            }
        });
    });
}

fn map_steps_to_dsl(steps: &[AutomationStep]) -> crate::core::automation::dsl::AutomationDsl {
    crate::core::automation::dsl::AutomationDsl {
        dsl_version: 1,
        steps: steps.iter().map(|s| match s {
            AutomationStep::Navigate(u) => crate::core::automation::dsl::Step::Navigate { url: u.clone() },
            AutomationStep::Click(sel) => crate::core::automation::dsl::Step::Click { selector: sel.clone() },
            AutomationStep::Type { selector, value, use_variable } => {
                let final_val = if *use_variable { format!("{{{{{}}}}}", value) } else { value.clone() };
                crate::core::automation::dsl::Step::Type { selector: selector.clone(), value: final_val }
            },
            AutomationStep::Wait(secs) => crate::core::automation::dsl::Step::WaitFor { selector: "body".into(), timeout_ms: Some(secs * 1000) },
            AutomationStep::WaitSelector(sel) => crate::core::automation::dsl::Step::WaitFor { selector: sel.clone(), timeout_ms: Some(5000) },
            AutomationStep::WaitUntilIdle => crate::core::automation::dsl::Step::WaitUntilIdle { timeout_ms: Some(2000) },
            AutomationStep::Screenshot(f) => crate::core::automation::dsl::Step::Screenshot { filename: f.clone() },
            AutomationStep::ScrollBottom => crate::core::automation::dsl::Step::ScrollBottom,
            AutomationStep::Extract { selector, as_key, add_to_dataset } => crate::core::automation::dsl::Step::Extract { selector: selector.clone(), as_key: as_key.clone(), add_to_row: Some(*add_to_dataset) },
            AutomationStep::SetVariable { key, value } => crate::core::automation::dsl::Step::SetVariable { key: key.clone(), value: value.clone() },
            AutomationStep::NewRow => crate::core::automation::dsl::Step::NewRow,
            AutomationStep::Export(f) => crate::core::automation::dsl::Step::Export { filename: f.clone() },
            AutomationStep::ForEach { selector, body } => crate::core::automation::dsl::Step::ForEach { selector: selector.clone(), body: map_steps_to_dsl(body).steps },
            AutomationStep::If { condition_selector, then_steps } => crate::core::automation::dsl::Step::If { 
                condition: crate::core::automation::dsl::Condition::Exists { selector: condition_selector.clone() },
                then_steps: map_steps_to_dsl(then_steps).steps,
                else_steps: None,
            },
            _ => crate::core::automation::dsl::Step::ScrollBottom,
        }).collect(),
    }
}

fn map_dsl_to_steps(steps: Vec<crate::core::automation::dsl::Step>) -> Vec<AutomationStep> {
    steps.into_iter().map(|s| match s {
        crate::core::automation::dsl::Step::Navigate { url } => AutomationStep::Navigate(url),
        crate::core::automation::dsl::Step::Click { selector } => AutomationStep::Click(selector),
        crate::core::automation::dsl::Step::Type { selector, value } => {
            if value.starts_with("{{") && value.ends_with("}}") {
                AutomationStep::Type { selector, value: value.trim_matches(|c| c == '{' || c == '}').to_string(), use_variable: true }
            } else {
                AutomationStep::Type { selector, value, use_variable: false }
            }
        },
        crate::core::automation::dsl::Step::WaitFor { selector, timeout_ms } => {
            if selector == "body" { AutomationStep::Wait(timeout_ms.unwrap_or(1000) / 1000) }
            else { AutomationStep::WaitSelector(selector) }
        },
        crate::core::automation::dsl::Step::WaitUntilIdle { .. } => AutomationStep::WaitUntilIdle,
        crate::core::automation::dsl::Step::Screenshot { filename } => AutomationStep::Screenshot(filename),
        crate::core::automation::dsl::Step::Extract { selector, as_key, add_to_row } => AutomationStep::Extract { selector, as_key, add_to_dataset: add_to_row.unwrap_or(true) },
        crate::core::automation::dsl::Step::SetVariable { key, value } => AutomationStep::SetVariable { key, value },
        crate::core::automation::dsl::Step::NewRow => AutomationStep::NewRow,
        crate::core::automation::dsl::Step::Export { filename } => AutomationStep::Export(filename),
        crate::core::automation::dsl::Step::If { condition, then_steps, .. } => {
            let sel = match condition { crate::core::automation::dsl::Condition::Exists { selector } => selector, _ => "".into() };
            AutomationStep::If { condition_selector: sel, then_steps: map_dsl_to_steps(then_steps) }
        },
        crate::core::automation::dsl::Step::ForEach { selector, body } => AutomationStep::ForEach { selector, body: map_dsl_to_steps(body) },
        crate::core::automation::dsl::Step::ScrollBottom => AutomationStep::ScrollBottom,
    }).collect()
}
