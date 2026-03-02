use crate::state::{AppState, AutomationStep, AutomationStatus};
use egui::{Ui, Color32, RichText};
use crate::core::events::AppEvent;
use crate::ui::scrape::emit;

pub fn render(ui: &mut Ui, state: &mut AppState) {
    ui.heading("AUTOMATION STUDIO");
    ui.add_space(10.0);

    // LIVE CONSOLE (NEW)
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("LIVE BROWSER CONSOLE").strong().color(Color32::LIGHT_BLUE));
            if ui.button("Clear Console").clicked() { state.console_logs.clear(); }
        });
        ui.add_space(5.0);
        egui::ScrollArea::vertical().max_height(150.0).stick_to_bottom(true).show(ui, |ui| {
            if state.console_logs.is_empty() {
                ui.label(RichText::new("No console logs yet...").italics().color(Color32::GRAY));
            } else {
                for log in &state.console_logs {
                    ui.label(RichText::new(log).monospace().size(11.0));
                }
            }
        });
    });

    ui.add_space(15.0);

    // PIPELINE BUILDER
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("AUTOMATION PIPELINE").strong().size(16.0));
            ui.menu_button("➕ Add Step", |ui| {
                if ui.button("Navigate").clicked() { state.auto_steps.push(AutomationStep::Navigate(String::new())); ui.close_menu(); }
                if ui.button("Click").clicked() { state.auto_steps.push(AutomationStep::Click(String::new())); ui.close_menu(); }
                if ui.button("Wait").clicked() { state.auto_steps.push(AutomationStep::Wait(1)); ui.close_menu(); }
                if ui.button("Scroll Bottom").clicked() { state.auto_steps.push(AutomationStep::ScrollBottom); ui.close_menu(); }
                if ui.button("Extract Text").clicked() { state.auto_steps.push(AutomationStep::ExtractText(String::new())); ui.close_menu(); }
            });
            if ui.button("🗑 Clear All").clicked() { state.auto_steps.clear(); }
        });

        ui.add_space(10.0);

        let mut to_remove = None;
        egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
            for (idx, step) in state.auto_steps.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{}.", idx + 1)).strong());
                    match step {
                        AutomationStep::Navigate(url) => { ui.label("Navigate:"); ui.text_edit_singleline(url); }
                        AutomationStep::Click(sel) => { ui.label("Click:"); ui.text_edit_singleline(sel); }
                        AutomationStep::Wait(secs) => { ui.label("Wait (s):"); ui.add(egui::DragValue::new(secs).range(1..=60)); }
                        AutomationStep::ExtractText(sel) => { ui.label("Extract:"); ui.text_edit_singleline(sel); }
                        AutomationStep::ScrollBottom => { ui.label("Scroll to bottom of page."); }
                        AutomationStep::InjectJS(s) => { ui.label("Inject JS:"); ui.text_edit_singleline(s); }
                        _ => {}
                    }
                    if ui.button("❌").clicked() { to_remove = Some(idx); }
                });
            }
        });
        if let Some(idx) = to_remove { state.auto_steps.remove(idx); }

        ui.add_space(10.0);
        let can_run = state.selected_tab_id.is_some() && state.auto_status == AutomationStatus::Idle;
        let btn_text = match &state.auto_status {
            AutomationStatus::Idle => "▶ START PIPELINE",
            AutomationStatus::Running(i) => "RUNNING...",
            _ => "START PIPELINE",
        };

        if ui.add_enabled(can_run, egui::Button::new(RichText::new(btn_text).strong()).min_size([ui.available_width(), 40.0].into())).clicked() {
            if let Some(tid) = state.selected_tab_id.clone() {
                state.auto_status = AutomationStatus::Running(0);
                emit(AppEvent::RequestAutomationRun(tid, state.auto_steps.clone()));
            }
        }
    });

    ui.add_space(15.0);

    // LIVE SCRIPT
    ui.group(|ui| {
        ui.label(RichText::new("SCRIPT INJECTION").strong());
        ui.horizontal(|ui| {
            if ui.button("📁 Load JS File").clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter("JS", &["js"]).pick_file() {
                    if let Ok(c) = std::fs::read_to_string(&path) { state.js_script = c; }
                }
            }
            if ui.button("Clear").clicked() { state.js_script.clear(); }
        });
        ui.add(egui::TextEdit::multiline(&mut state.js_script).font(egui::TextStyle::Monospace).desired_rows(6).desired_width(f32::INFINITY));
        if ui.button("▶ INJECT").clicked() {
            if let Some(tid) = state.selected_tab_id.clone() { emit(AppEvent::RequestScriptExecution(tid, state.js_script.clone())); }
        }
        if !state.js_result.is_empty() {
            ui.add_space(5.0);
            ui.add(egui::Label::new(RichText::new(&state.js_result).color(Color32::GREEN).monospace()).selectable(true));
        }
    });
}
