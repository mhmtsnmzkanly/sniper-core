use crate::state::AppState;
use crate::ui::design;
use egui::{Color32, RichText, Ui};

pub fn render(ui: &mut Ui, state: &mut AppState) {
    ui.vertical(|ui| {
        design::title(ui, "System Telemetry", design::ACCENT_CYAN);
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{} entries", state.logs.len()))
                    .small()
                    .color(design::TEXT_MUTED),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🗑 CLEAR").clicked() {
                    state.logs.clear();
                }
                if ui.button("💾 EXPORT").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Export System Logs")
                        .add_filter("Log Files", &["txt", "log"])
                        .set_file_name("sniper_logs.txt")
                        .save_file()
                    {
                        let content = state
                            .logs
                            .iter()
                            .map(|l| format!("[{}] [{}] {}", l.timestamp, l.level, l.message))
                            .collect::<Vec<_>>()
                            .join("\n");
                        if let Err(e) = std::fs::write(&path, content) {
                            tracing::error!("[UI] Log export failed: {}", e);
                        } else {
                            tracing::info!("[UI] Logs exported to {:?}", path);
                        }
                    }
                }
                if ui.button("📋 COPY ALL").clicked() {
                    let all = state
                        .logs
                        .iter()
                        .map(|l| format!("[{}] [{}] {}", l.timestamp, l.level, l.message))
                        .collect::<Vec<_>>()
                        .join("\n");
                    ui.ctx().copy_text(all);
                }
            });
        });

        ui.add_space(4.0);

        // KOD NOTU: max_height kullanılmaz; available_height ile panel her zaman
        // kalan alanı doldurur ve scroll overflow olmaz.
        let avail_h = ui.available_height() - 4.0;
        design::section_frame().show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("log_scroll")
                .max_height(avail_h)
                .stick_to_bottom(true)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());

                    for log in &state.logs {
                        let color = match log.level.as_str() {
                            "ERROR" => Color32::from_rgb(255, 110, 110),
                            "WARN" => design::ACCENT_ORANGE,
                            "DEBUG" => design::TEXT_MUTED,
                            "TRACE" => Color32::from_rgb(90, 108, 122),
                            _ => design::TEXT_PRIMARY,
                        };

                        ui.horizontal(|ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!("[{}]", log.timestamp))
                                        .color(Color32::from_rgb(90, 112, 130))
                                        .monospace()
                                        .size(11.0),
                                )
                                .selectable(false),
                            );
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&log.message)
                                        .color(color)
                                        .monospace()
                                        .size(12.0),
                                )
                                .selectable(true)
                                .wrap(),
                            );
                        });
                    }
                });
        });
    });
}
