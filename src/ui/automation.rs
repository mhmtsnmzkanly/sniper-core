use crate::state::{AppState, AutomationStep, AutomationStatus};
use egui::{Ui, Color32, RichText, Frame, Stroke, CornerRadius, vec2};
use crate::core::events::AppEvent;
use crate::ui::scrape::emit;

pub fn render_embedded(ui: &mut Ui, state: &mut AppState, tid: &str) {
    if !state.workspaces.contains_key(tid) { return; }
    
    let (mut auto_steps, mut auto_status, discovered_selectors, mut selector_search) = {
        let ws = state.workspaces.get(tid).unwrap();
        (ws.auto_steps.clone(), ws.auto_status.clone(), ws.discovered_selectors.clone(), ws.selector_search.clone())
    };

    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("🤖 SCRATCH AUTOMATION").strong().size(18.0).color(Color32::KHAKI));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(RichText::new("🔄 SCAN SELECTORS").strong()).on_hover_text("Discover all IDs and Classes from the active tab").clicked() {
                    emit(AppEvent::RequestPageSelectors(tid.to_string()));
                }
                ui.menu_button(RichText::new("➕ ADD BLOCK").strong(), |ui| {
                    ui.set_min_width(150.0);
                    ui.label("ACTIONS");
                    if ui.button("🌐 Navigate").clicked() { auto_steps.push(AutomationStep::Navigate("https://".into())); ui.close_menu(); }
                    if ui.button("🖱 Click").clicked() { auto_steps.push(AutomationStep::Click("".into())); ui.close_menu(); }
                    if ui.button("⌨ Type").clicked() { auto_steps.push(AutomationStep::Type { selector: "".into(), value: "".into() }); ui.close_menu(); }
                    ui.separator();
                    if ui.button("⏳ Wait (Sec)").clicked() { auto_steps.push(AutomationStep::Wait(1)); ui.close_menu(); }
                    if ui.button("🔍 Wait Selector").clicked() { auto_steps.push(AutomationStep::WaitSelector("".into())); ui.close_menu(); }
                    if ui.button("📜 Scroll Bottom").clicked() { auto_steps.push(AutomationStep::ScrollBottom); ui.close_menu(); }
                    ui.separator();
                    if ui.button("🧪 Extract").clicked() { auto_steps.push(AutomationStep::Extract { selector: "".into(), as_key: "data".into(), add_to_row: true }); ui.close_menu(); }
                    if ui.button("🔁 Loop").clicked() { auto_steps.push(AutomationStep::ForEach { selector: "".into(), body: vec![] }); ui.close_menu(); }
                    if ui.button("❓ If").clicked() { auto_steps.push(AutomationStep::If { condition_selector: "".into(), then_steps: vec![] }); ui.close_menu(); }
                });
                
                if ui.button("💾 SAVE").clicked() {
                    let dsl = map_steps_to_dsl(&auto_steps);
                    if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).set_file_name("automation.json").save_file() {
                        if let Ok(json) = serde_json::to_string_pretty(&dsl) { let _ = std::fs::write(path, json); }
                    }
                }
                if ui.button("📁 LOAD").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() {
                        if let Ok(content) = std::fs::read_to_string(path) {
                            if let Ok(dsl) = serde_json::from_str::<crate::core::automation::dsl::AutomationDsl>(&content) {
                                auto_steps = map_dsl_to_steps(dsl.steps);
                            }
                        }
                    }
                }
                if ui.button("🗑 CLEAR").clicked() { auto_steps.clear(); }
            });
        });

        ui.add_space(10.0);

        let mut move_from = None;
        let mut move_to = None;
        let mut delete_idx = None;

        egui::ScrollArea::vertical().max_height(500.0).id_salt("auto_scroll").show(ui, |ui| {
            if auto_steps.is_empty() {
                ui.centered_and_justified(|ui| { ui.label(RichText::new("Drag and add blocks to build your robot...").italics().color(Color32::GRAY)); });
            }

            let steps_len = auto_steps.len();
            for idx in 0..steps_len {
                let item_id = egui::Id::new(("step", idx));
                
                // --- DND WRAPPER ---
                // We wrap the whole block in drag source, but inside we'll use a handle
                let dnd_res = ui.dnd_drag_source(item_id, idx, |ui| {
                    render_step_block(ui, &mut auto_steps[idx], idx, &mut delete_idx, &discovered_selectors, &mut selector_search);
                });

                // Drop zone logic
                if let Some(payload) = ui.dnd_drop_zone::<usize, _>(Frame::NONE, |ui| {
                    // Optional: show a line indicator
                }).1 {
                    move_from = Some(*payload);
                    move_to = Some(idx);
                }
            }
        });

        if let (Some(from), Some(to)) = (move_from, move_to) {
            if from != to {
                let item = auto_steps.remove(from);
                auto_steps.insert(to, item);
            }
        }
        if let Some(idx) = delete_idx { auto_steps.remove(idx); }

        ui.add_space(12.0);
        let can_run = auto_status == AutomationStatus::Idle && !auto_steps.is_empty();
        let (btn_text, btn_color) = match &auto_status {
            AutomationStatus::Idle => ("▶ START AUTOMATION PIPELINE".to_string(), Color32::from_rgb(40, 120, 60)),
            AutomationStatus::Running(i) => (format!("🏃 EXECUTING BLOCK {}...", i + 1), Color32::from_rgb(180, 120, 20)),
            _ => ("▶ RE-RUN AUTOMATION".to_string(), Color32::from_rgb(40, 120, 60)),
        };

        if ui.add_enabled(can_run || !matches!(auto_status, AutomationStatus::Running(_)), 
            egui::Button::new(RichText::new(btn_text).strong().size(15.0))
                .min_size([ui.available_width(), 45.0].into())
                .fill(btn_color))
            .clicked() {
            auto_status = AutomationStatus::Running(0);
            emit(AppEvent::RequestAutomationRun(tid.to_string(), auto_steps.clone()));
        }
    });

    if let Some(ws) = state.workspaces.get_mut(tid) {
        ws.auto_steps = auto_steps;
        ws.auto_status = auto_status;
        ws.selector_search = selector_search;
    }
}

fn render_step_block(ui: &mut Ui, step: &mut AutomationStep, idx: usize, delete_idx: &mut Option<usize>, discovered: &[String], search: &mut String) {
    let (color, title) = match step {
        AutomationStep::Navigate(_) | AutomationStep::Click(_) | AutomationStep::Type { .. } => (Color32::from_rgb(80, 130, 255), "ACTION"),
        AutomationStep::Wait(_) | AutomationStep::WaitSelector(_) | AutomationStep::ScrollBottom => (Color32::from_rgb(255, 200, 50), "WAIT"),
        AutomationStep::If { .. } | AutomationStep::ForEach { .. } => (Color32::from_rgb(255, 100, 50), "CONTROL"),
        AutomationStep::Extract { .. } | AutomationStep::Export(_) | AutomationStep::NewRow | AutomationStep::SetVariable { .. } => (Color32::from_rgb(50, 220, 120), "DATA"),
        _ => (Color32::DARK_GRAY, "OTHER"),
    };

    Frame::new()
        .fill(color.gamma_multiply(0.08))
        .stroke(Stroke::new(1.5, color.gamma_multiply(0.5)))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // --- DRAG HANDLE (Big and clickable) ---
                let handle = ui.add(egui::Label::new(RichText::new("⣿").size(22.0).color(color).strong()).sense(egui::Sense::drag()));
                handle.on_hover_text("Drag to reorder");
                
                ui.add_space(5.0);
                ui.label(RichText::new(format!("{}.", idx + 1)).strong().size(14.0));
                
                Frame::new().fill(color).corner_radius(CornerRadius::same(4)).inner_margin(vec2(6.0, 2.0)).show(ui, |ui| {
                    ui.label(RichText::new(title).strong().size(11.0).color(Color32::BLACK));
                });
                
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                // UI elements now use a higher priority for events
                ui.scope(|ui| {
                    match step {
                        AutomationStep::Navigate(url) => { ui.label("Navigate to:"); ui.add(egui::TextEdit::singleline(url).desired_width(300.0)); }
                        AutomationStep::Click(sel) => { ui.label("Click on:"); selector_input(ui, sel, discovered, search); }
                        AutomationStep::Type { selector, value } => {
                            ui.label("Type"); ui.add(egui::TextEdit::singleline(value).desired_width(120.0));
                            ui.label("into"); selector_input(ui, selector, discovered, search);
                        }
                        AutomationStep::Wait(secs) => { ui.label("Wait for"); ui.add(egui::DragValue::new(secs).range(1..=300)); ui.label("seconds"); }
                        AutomationStep::WaitSelector(sel) => { ui.label("Wait for element:"); selector_input(ui, sel, discovered, search); }
                        AutomationStep::Extract { selector, as_key, .. } => {
                            ui.label("Extract text from"); selector_input(ui, selector, discovered, search);
                            ui.label("save as"); ui.add(egui::TextEdit::singleline(as_key).desired_width(100.0));
                        }
                        AutomationStep::ForEach { selector, body } => {
                            ui.label("Loop through:"); selector_input(ui, selector, discovered, search);
                            ui.label(RichText::new(format!("({} inner blocks)", body.len())).italics().color(Color32::GRAY));
                        }
                        AutomationStep::If { condition_selector, then_steps } => {
                            ui.label("If exists:"); selector_input(ui, condition_selector, discovered, search);
                            ui.label(RichText::new(format!("({} blocks)", then_steps.len())).italics().color(Color32::GRAY));
                        }
                        AutomationStep::Export(file) => { ui.label("Export data to:"); ui.add(egui::TextEdit::singleline(file).desired_width(150.0)); }
                        AutomationStep::SetVariable { key, value } => { 
                            ui.label("Set"); ui.add(egui::TextEdit::singleline(key).desired_width(80.0)); 
                            ui.label("="); ui.add(egui::TextEdit::singleline(value).desired_width(120.0)); 
                        }
                        AutomationStep::NewRow => { ui.label(RichText::new("➕ START NEW DATA ROW").strong().color(Color32::LIGHT_GREEN)); }
                        AutomationStep::ScrollBottom => { ui.label("📜 SCROLL TO PAGE BOTTOM"); }
                        _ => { ui.label("Block"); }
                    }
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new("❌").color(Color32::LIGHT_RED)).on_hover_text("Delete this block").clicked() {
                        *delete_idx = Some(idx);
                    }
                });
            });
        });
    ui.add_space(6.0);
}

fn selector_input(ui: &mut Ui, value: &mut String, discovered: &[String], search: &mut String) {
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(value).desired_width(180.0));
        let menu_btn = ui.menu_button("🔍", |ui| {
            ui.set_max_width(300.0);
            ui.label(RichText::new("SEARCH DISCOVERED SELECTORS").strong());
            ui.add(egui::TextEdit::singleline(search).hint_text("Filter (e.g. .btn or #login)"));
            ui.separator();
            egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
                if discovered.is_empty() {
                    ui.label(RichText::new("No selectors found. Click SCAN above.").italics().color(Color32::GRAY));
                } else {
                    let filter = search.to_lowercase();
                    let mut found_any = false;
                    for s in discovered {
                        if filter.is_empty() || s.to_lowercase().contains(&filter) {
                            if ui.button(RichText::new(s).small()).clicked() {
                                *value = s.clone();
                                ui.close_menu();
                            }
                            found_any = true;
                        }
                    }
                    if !found_any { ui.label("No matches."); }
                }
            });
        });
        menu_btn.response.on_hover_text("Quick select ID/Class from page");
    });
}

fn map_steps_to_dsl(steps: &[AutomationStep]) -> crate::core::automation::dsl::AutomationDsl {
    crate::core::automation::dsl::AutomationDsl {
        dsl_version: 1,
        steps: steps.iter().map(|s| match s {
            AutomationStep::Navigate(u) => crate::core::automation::dsl::Step::Navigate { url: u.clone() },
            AutomationStep::Click(sel) => crate::core::automation::dsl::Step::Click { selector: sel.clone() },
            AutomationStep::Type { selector, value } => crate::core::automation::dsl::Step::Type { selector: selector.clone(), value: value.clone() },
            AutomationStep::Wait(secs) => crate::core::automation::dsl::Step::WaitFor { selector: "body".into(), timeout_ms: Some(secs * 1000) },
            AutomationStep::WaitSelector(sel) => crate::core::automation::dsl::Step::WaitFor { selector: sel.clone(), timeout_ms: Some(5000) },
            AutomationStep::ScrollBottom => crate::core::automation::dsl::Step::ScrollBottom,
            AutomationStep::Extract { selector, as_key, add_to_row } => crate::core::automation::dsl::Step::Extract { selector: selector.clone(), as_key: as_key.clone(), add_to_row: Some(*add_to_row) },
            AutomationStep::SetVariable { key, value } => crate::core::automation::dsl::Step::SetVariable { key: key.clone(), value: value.clone() },
            AutomationStep::NewRow => crate::core::automation::dsl::Step::NewRow,
            AutomationStep::Export(f) => crate::core::automation::dsl::Step::Export { filename: f.clone() },
            AutomationStep::If { condition_selector, then_steps } => crate::core::automation::dsl::Step::If { 
                condition: crate::core::automation::dsl::Condition::Exists { selector: condition_selector.clone() },
                then_steps: map_steps_to_dsl(then_steps).steps,
                else_steps: None,
            },
            AutomationStep::ForEach { selector, body } => crate::core::automation::dsl::Step::ForEach { 
                selector: selector.clone(),
                body: map_steps_to_dsl(body).steps,
            },
            _ => crate::core::automation::dsl::Step::ScrollBottom,
        }).collect(),
    }
}

fn map_dsl_to_steps(steps: Vec<crate::core::automation::dsl::Step>) -> Vec<AutomationStep> {
    steps.into_iter().map(|s| match s {
        crate::core::automation::dsl::Step::Navigate { url } => AutomationStep::Navigate(url),
        crate::core::automation::dsl::Step::Click { selector } => AutomationStep::Click(selector),
        crate::core::automation::dsl::Step::Type { selector, value } => AutomationStep::Type { selector, value },
        crate::core::automation::dsl::Step::WaitFor { selector, timeout_ms } => {
            if selector == "body" { AutomationStep::Wait(timeout_ms.unwrap_or(1000) / 1000) }
            else { AutomationStep::WaitSelector(selector) }
        },
        crate::core::automation::dsl::Step::Extract { selector, as_key, add_to_row } => AutomationStep::Extract { selector, as_key, add_to_row: add_to_row.unwrap_or(true) },
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
