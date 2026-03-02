use crate::state::{AppState, AutomationStep, AutomationStatus};
use egui::{Ui, Color32, RichText, Frame, Stroke, CornerRadius};
use crate::core::events::AppEvent;
use crate::ui::scrape::emit;

pub fn render_embedded(ui: &mut Ui, state: &mut AppState, tid: &str) {
    if !state.workspaces.contains_key(tid) { return; }
    
    let mut auto_steps = {
        let ws = state.workspaces.get(tid).unwrap();
        ws.auto_steps.clone()
    };
    let mut auto_status = state.workspaces.get(tid).unwrap().auto_status.clone();

    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("🤖 SCRATCH AUTOMATION").strong().color(Color32::KHAKI));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
                
                // --- DRAG SOURCE ---
                let dnd_res = ui.dnd_drag_source(item_id, idx, |ui| {
                    render_step_block(ui, step, idx, &mut delete_idx);
                });

                // --- DROP ZONE ---
                if let Some(payload) = ui.dnd_drop_zone::<usize, _>(Frame::NONE, |ui| {
                    // Visual feedback could go here if we wanted to show a line where it drops
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

fn render_step_block(ui: &mut Ui, step: &mut AutomationStep, idx: usize, delete_idx: &mut Option<usize>) {
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
                    AutomationStep::Click(sel) => { ui.label("Click:"); ui.text_edit_singleline(sel); }
                    AutomationStep::Type { selector, value } => {
                        ui.label("Type"); ui.text_edit_singleline(value);
                        ui.label("into"); ui.text_edit_singleline(selector);
                    }
                    AutomationStep::Wait(secs) => { ui.label("Wait"); ui.add(egui::DragValue::new(secs)); ui.label("sec"); }
                    AutomationStep::WaitSelector(sel) => { ui.label("Wait for"); ui.text_edit_singleline(sel); }
                    AutomationStep::Extract { selector, as_key, .. } => {
                        ui.label("Extract"); ui.text_edit_singleline(selector);
                        ui.label("as"); ui.text_edit_singleline(as_key);
                    }
                    AutomationStep::ForEach { selector, body } => {
                        ui.label("Loop:"); ui.text_edit_singleline(selector);
                        ui.label(RichText::new(format!("({} steps)", body.len())).italics().small());
                    }
                    AutomationStep::If { condition_selector, then_steps } => {
                        ui.label("If exists:"); ui.text_edit_singleline(condition_selector);
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
