use crate::state::AppState;
use egui::{Ui, Color32, RichText};
use crate::core::events::AppEvent;
use crate::ui::scrape::emit;

pub fn render(ui: &mut Ui, state: &mut AppState) {
    ui.heading("SCRIPT INJECTION STUDIO");
    ui.add_space(10.0);

    ui.group(|ui| {
        ui.label(RichText::new("Step 1: Write JavaScript").strong());
        ui.add_space(5.0);
        
        let editor = egui::TextEdit::multiline(&mut state.js_script)
            .font(egui::TextStyle::Monospace)
            .desired_rows(10)
            .lock_focus(true)
            .desired_width(f32::INFINITY);
        
        ui.add(editor);
        
        ui.add_space(10.0);
        
        let can_run = state.selected_tab_id.is_some() && !state.js_execution_active;
        let btn_text = if state.js_execution_active { "EXECUTING..." } else { "▶ RUN SCRIPT ON TARGET TAB" };
        
        let btn = ui.add_enabled(can_run, egui::Button::new(RichText::new(btn_text).strong()).min_size([ui.available_width(), 40.0].into()));
        
        if btn.clicked() {
            if let Some(tab_id) = state.selected_tab_id.clone() {
                state.js_execution_active = true;
                state.js_result = "Running...".to_string();
                emit(AppEvent::RequestScriptExecution(tab_id, state.js_script.clone()));
            }
        }
    });

    ui.add_space(15.0);

    ui.group(|ui| {
        ui.label(RichText::new("Execution Result").strong());
        ui.add_space(5.0);
        
        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
            ui.add(egui::Label::new(
                RichText::new(&state.js_result)
                    .monospace()
                    .color(Color32::GREEN)
            ).selectable(true));
        });
        
        if ui.button("Clear Result").clicked() {
            state.js_result.clear();
        }
    });

    ui.add_space(10.0);
    ui.separator();
    ui.label(RichText::new("Note: Return a value in JS to see it here (e.g., 'document.title').").small().italics());
}
