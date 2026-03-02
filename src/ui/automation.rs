use crate::state::{AppState, AutomationStep, AutomationStatus};
use egui::{Ui, Color32, RichText, Frame, Stroke, CornerRadius};
use crate::core::events::AppEvent;
use crate::ui::scrape::emit;

pub fn render_embedded(ui: &mut Ui, state: &mut AppState, tid: &str) {
    if !state.workspaces.contains_key(tid) { return; }
    
    let (mut auto_steps, mut auto_status, discovered_selectors) = {
        let ws = state.workspaces.get(tid).unwrap();
        (ws.auto_steps.clone(), ws.auto_status.clone(), ws.discovered_selectors.clone())
    };

    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("🤖 SCRATCH AUTOMATION").strong().color(Color32::KHAKI));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🔄 SCAN SELECTORS").on_hover_text("Discover all IDs and Classes from the active tab").clicked() {
                    emit(AppEvent::RequestPageSelectors(tid.to_string()));
                }
                ui.menu_button("➕ ADD BLOCK", |ui| {
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

        egui::ScrollArea::vertical().max_height(400.0).id_salt("auto_scroll").show(ui, |ui| {
            if auto_steps.is_empty() {
                ui.centered_and_justified(|ui| { ui.label("Add blocks to build your robot..."); });
            }

            for (idx, step) in auto_steps.iter_mut().enumerate() {
                let item_id = egui::Id::new(("step", idx));
                let dnd_res = ui.dnd_drag_source(item_id, idx, |ui| {
                    render_step_block(ui, step, idx, &mut delete_idx, &discovered_selectors);
                });

                if let Some(payload) = ui.dnd_drop_zone::<usize, _>(Frame::NONE, |ui| { }).1 {
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

        ui.add_space(8.0);
        let can_run = auto_status == AutomationStatus::Idle && !auto_steps.is_empty();
        let btn_text = match &auto_status {
            AutomationStatus::Idle => "▶ RUN AUTOMATION".to_string(),
            AutomationStatus::Running(i) => format!("🏃 STEP {}...", i + 1),
            _ => "▶ RUN AGAIN".to_string(),
        };

        if ui.add_enabled(can_run || matches!(auto_status, AutomationStatus::Finished | AutomationStatus::Error(_)), 
            egui::Button::new(RichText::new(btn_text).strong()).min_size([ui.available_width(), 40.0].into()))
            .clicked() {
            auto_status = AutomationStatus::Running(0);
            emit(AppEvent::RequestAutomationRun(tid.to_string(), auto_steps.clone()));
        }
    });

    if let Some(ws) = state.workspaces.get_mut(tid) {
        ws.auto_steps = auto_steps;
        ws.auto_status = auto_status;
    }
}

fn render_step_block(ui: &mut Ui, step: &mut AutomationStep, idx: usize, delete_idx: &mut Option<usize>, discovered: &[String]) {
    let (color, title) = match step {
        AutomationStep::Navigate(_) | AutomationStep::Click(_) | AutomationStep::Type { .. } => (Color32::from_rgb(60, 100, 200), "ACTION"),
        AutomationStep::Wait(_) | AutomationStep::WaitSelector(_) | AutomationStep::ScrollBottom => (Color32::from_rgb(180, 150, 40), "WAIT"),
        AutomationStep::If { .. } | AutomationStep::ForEach { .. } => (Color32::from_rgb(200, 80, 40), "CONTROL"),
        AutomationStep::Extract { .. } | AutomationStep::Export(_) | AutomationStep::NewRow | AutomationStep::SetVariable { .. } => (Color32::from_rgb(40, 150, 80), "DATA"),
        _ => (Color32::DARK_GRAY, "OTHER"),
    };

    Frame::new()
        .fill(color.gamma_multiply(0.1))
        .stroke(Stroke::new(1.0, color))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("☰").color(color).strong());
                ui.label(RichText::new(format!("{}. ", idx + 1)).small());
                ui.label(RichText::new(title).small().strong().color(color));
                ui.separator();

                match step {
                    AutomationStep::Navigate(url) => { ui.label("Nav:"); ui.text_edit_singleline(url); }
                    AutomationStep::Click(sel) => { ui.label("Click:"); selector_input(ui, sel, discovered); }
                    AutomationStep::Type { selector, value } => {
                        ui.label("Type"); ui.text_edit_singleline(value);
                        ui.label("into"); selector_input(ui, selector, discovered);
                    }
                    AutomationStep::Wait(secs) => { ui.label("Wait"); ui.add(egui::DragValue::new(secs)); ui.label("sec"); }
                    AutomationStep::WaitSelector(sel) => { ui.label("Wait for"); selector_input(ui, sel, discovered); }
                    AutomationStep::Extract { selector, as_key, .. } => {
                        ui.label("Extract"); selector_input(ui, selector, discovered);
                        ui.label("as"); ui.text_edit_singleline(as_key);
                    }
                    AutomationStep::ForEach { selector, body } => {
                        ui.label("Loop:"); selector_input(ui, selector, discovered);
                        ui.label(RichText::new(format!("({} steps)", body.len())).italics().small());
                    }
                    AutomationStep::If { condition_selector, then_steps } => {
                        ui.label("If exists:"); selector_input(ui, condition_selector, discovered);
                        ui.label(RichText::new(format!("({} steps)", then_steps.len())).italics().small());
                    }
                    _ => { ui.label("Block"); }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("❌").clicked() { *delete_idx = Some(idx); }
                });
            });
        });
    ui.add_space(4.0);
}

fn selector_input(ui: &mut Ui, value: &mut String, discovered: &[String]) {
    ui.horizontal(|ui| {
        ui.text_edit_singleline(value);
        ui.menu_button("🔍", |ui| {
            ui.set_max_width(250.0);
            ui.label(RichText::new("Discovered Selectors").strong().small());
            ui.separator();
            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                if discovered.is_empty() {
                    ui.label(RichText::new("No selectors found. Click SCAN above.").italics().color(Color32::GRAY));
                }
                for s in discovered {
                    if ui.button(RichText::new(s).small()).clicked() {
                        *value = s.clone();
                        ui.close_menu();
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
