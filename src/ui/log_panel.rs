use crate::state::AppState;
use egui::{Ui, Color32, RichText};

pub fn render(ui: &mut Ui, state: &mut AppState) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("SESSION LOGS").strong().size(16.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("COPY ALL LOGS").clicked() {
                    let full_log: String = state.logs.iter()
                        .map(|l| format!("[{}] {}", l.timestamp, l.message))
                        .collect::<Vec<_>>()
                        .join("\n");
                    ui.ctx().copy_text(full_log);
                }
            });
        });
        ui.add_space(5.0);
        ui.separator();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.set_width(ui.available_width()); // Tam genişlik
                ui.vertical(|ui| {
                    for log in &state.logs {
                        let color = match log.level {
                            tracing::Level::ERROR => Color32::RED,
                            tracing::Level::WARN => Color32::YELLOW,
                            tracing::Level::INFO => Color32::LIGHT_GRAY,
                            tracing::Level::DEBUG => Color32::GRAY,
                            tracing::Level::TRACE => Color32::DARK_GRAY,
                        };
                        
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("[{}]", log.timestamp)).color(Color32::DARK_GRAY).monospace().size(12.0));
                            ui.add(egui::Label::new(RichText::new(&log.message).color(color).monospace().size(13.0)).selectable(true));
                        });
                        ui.add_space(3.0);
                    }
                });
            });
    });
}
