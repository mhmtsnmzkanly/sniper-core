use crate::state::AppState;
use egui::{Ui, Color32, RichText};

pub fn render(ui: &mut Ui, state: &mut AppState) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.heading(RichText::new("SYSTEM LOGS").color(Color32::LIGHT_BLUE));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🗑 CLEAR").clicked() { state.logs.clear(); }
                if ui.button("📋 COPY ALL").clicked() {
                    let all = state.logs.iter()
                        .map(|l| format!("[{}] [{}] {}", l.timestamp, l.level, l.message))
                        .collect::<Vec<_>>().join("\n");
                    ui.ctx().copy_text(all);
                }
            });
        });

        ui.add_space(5.0);

        egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            
            for log in &state.logs {
                let color = match log.level.as_str() {
                    "ERROR" => Color32::RED,
                    "WARN" => Color32::YELLOW,
                    "DEBUG" => Color32::GRAY,
                    "TRACE" => Color32::DARK_GRAY,
                    _ => Color32::LIGHT_GRAY,
                };

                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("[{}]", log.timestamp)).color(Color32::DARK_GRAY).monospace().size(12.0));
                    ui.add(egui::Label::new(RichText::new(&log.message).color(color).monospace().size(13.0)).selectable(true));
                });
            }
        });
    });
}
