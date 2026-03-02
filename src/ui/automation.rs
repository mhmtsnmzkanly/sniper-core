use crate::state::{AppState, AutomationStep, AutomationStatus};
use egui::{Ui, Color32, RichText};
use crate::core::events::AppEvent;
use crate::ui::scrape::emit;

pub fn render(ui: &mut Ui, state: &mut AppState) {
    let tid = match &state.selected_tab_id {
        Some(id) => id.clone(),
        None => {
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);
                ui.label(RichText::new("⚠ SELECT A TAB IN THE SCRAPE PANEL TO START AUTOMATION").strong().color(Color32::YELLOW));
            });
            return;
        }
    };

    if !state.workspaces.contains_key(&tid) { return; }
    let ws = state.workspaces.get_mut(&tid).unwrap();

    ui.heading(format!("AUTOMATION STUDIO: {}", ws.title));
    ui.add_space(10.0);

    // LIVE CONSOLE
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("LIVE BROWSER CONSOLE").strong().color(Color32::LIGHT_BLUE));
            if ui.button("Clear Console").clicked() { ws.console_logs.clear(); }
        });
        ui.add_space(5.0);
        egui::ScrollArea::vertical().max_height(150.0).stick_to_bottom(true).show(ui, |ui| {
            if ws.console_logs.is_empty() {
                ui.label(RichText::new("No console logs yet...").italics().color(Color32::GRAY));
            } else {
                for log in &ws.console_logs {
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
                if ui.button("Navigate").clicked() { ws.auto_steps.push(AutomationStep::Navigate(String::new())); ui.close_menu(); }
                if ui.button("Click").clicked() { ws.auto_steps.push(AutomationStep::Click(String::new())); ui.close_menu(); }
                if ui.button("Wait").clicked() { ws.auto_steps.push(AutomationStep::Wait(1)); ui.close_menu(); }
                if ui.button("Scroll Bottom").clicked() { ws.auto_steps.push(AutomationStep::ScrollBottom); ui.close_menu(); }
                if ui.button("Extract Text").clicked() { ws.auto_steps.push(AutomationStep::ExtractText(String::new())); ui.close_menu(); }
            });
            if ui.button("🗑 Clear All").clicked() { ws.auto_steps.clear(); }
        });

        ui.add_space(10.0);

        let mut to_remove = None;
        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
            for (idx, step) in ws.auto_steps.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{}.", idx + 1)).strong());
                    match step {
                        AutomationStep::Navigate(url) => { ui.label("Navigate:"); ui.text_edit_singleline(url); }
                        AutomationStep::Click(sel) => { ui.label("Click:"); ui.text_edit_singleline(sel); }
                        AutomationStep::Wait(secs) => { ui.label("Wait (s):"); ui.add(egui::DragValue::new(secs).range(1..=60)); }
                        AutomationStep::ExtractText(sel) => { ui.label("Extract:"); ui.text_edit_singleline(sel); }
                        AutomationStep::ScrollBottom => { ui.label("Scroll to bottom of page."); }
                        AutomationStep::InjectJS(s) => { ui.label("Inject JS:"); ui.text_edit_singleline(s); }
                        AutomationStep::WaitSelector(sel) => { ui.label("Wait for:"); ui.text_edit_singleline(sel); }
                    }
                    if ui.button("❌").clicked() { to_remove = Some(idx); }
                });
            }
        });
        if let Some(idx) = to_remove { ws.auto_steps.remove(idx); }

        ui.add_space(10.0);
        let can_run = ws.auto_status == AutomationStatus::Idle;
        let btn_text = match &ws.auto_status {
            AutomationStatus::Idle => "▶ START PIPELINE".to_string(),
            AutomationStatus::Running(i) => format!("RUNNING STEP {}...", i + 1),
            _ => "START PIPELINE".to_string(),
        };

        if ui.add_enabled(can_run, egui::Button::new(RichText::new(btn_text).strong()).min_size([ui.available_width(), 40.0].into())).clicked() {
            ws.auto_status = AutomationStatus::Running(0);
            emit(AppEvent::RequestAutomationRun(tid.clone(), ws.auto_steps.clone()));
        }
    });

    ui.add_space(15.0);

    // LIVE SCRIPT
    ui.group(|ui| {
        ui.label(RichText::new("SCRIPT INJECTION").strong());
        ui.horizontal(|ui| {
            if ui.button("📁 Load JS File").clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter("JS", &["js"]).pick_file() {
                    if let Ok(c) = std::fs::read_to_string(&path) { ws.js_script = c; }
                }
            }
            if ui.button("Clear").clicked() { ws.js_script.clear(); }
        });
        ui.add(egui::TextEdit::multiline(&mut ws.js_script).font(egui::TextStyle::Monospace).desired_rows(6).desired_width(f32::INFINITY));
        if ui.button("▶ INJECT").clicked() {
            emit(AppEvent::RequestScriptExecution(tid.clone(), ws.js_script.clone()));
        }
        if !ws.js_result.is_empty() {
            ui.add_space(5.0);
            ui.add(egui::Label::new(RichText::new(&ws.js_result).color(Color32::GREEN).monospace()).selectable(true));
        }
    });
}
