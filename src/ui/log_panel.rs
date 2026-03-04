use crate::state::AppState;
use crate::ui::design;
use egui::{Color32, RichText, Ui};

pub fn render(ui: &mut Ui, state: &mut AppState) {
    ui.vertical(|ui| {
        design::title(ui, "System Telemetry", design::ACCENT_CYAN);
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🗑 CLEAR").clicked() { state.logs.clear(); }
                
                // KOD NOTU: Mevcut logları kullanıcı tarafından seçilen bir dosyaya kaydeder.
                if ui.button("💾 EXPORT").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Export System Logs")
                        .add_filter("Log Files", &["txt", "log"])
                        .set_file_name("sniper_logs.txt")
                        .save_file() {
                        let content = state.logs.iter()
                            .map(|l| format!("[{}] [{}] {}", l.timestamp, l.level, l.message))
                            .collect::<Vec<_>>().join("\n");
                        if let Err(e) = std::fs::write(&path, content) {
                            tracing::error!("[UI -> LOG] Failed to export logs to {:?}: {}", path, e);
                        } else {
                            tracing::info!("[UI -> LOG] Logs successfully exported to {:?}", path);
                        }
                    }
                }

                if ui.button("📋 COPY ALL").clicked() {
                    let all = state.logs.iter()
                        .map(|l| format!("[{}] [{}] {}", l.timestamp, l.level, l.message))
                        .collect::<Vec<_>>().join("\n");
                    ui.ctx().copy_text(all);
                }
            });
        });

        ui.add_space(5.0);

        design::section_frame().show(ui, |ui| {
            egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                
                for log in &state.logs {
                    let color = match log.level.as_str() {
                        "ERROR" => Color32::from_rgb(255, 120, 120),
                        "WARN" => design::ACCENT_ORANGE,
                        "DEBUG" => design::TEXT_MUTED,
                        "TRACE" => Color32::from_rgb(99, 116, 130),
                        _ => design::TEXT_PRIMARY,
                    };

                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("[{}]", log.timestamp)).color(Color32::from_rgb(108, 129, 146)).monospace().size(12.0));
                        ui.add(egui::Label::new(RichText::new(&log.message).color(color).monospace().size(13.0)).selectable(true));
                    });
                }
            });
        });
    });
}
