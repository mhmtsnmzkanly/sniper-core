use crate::state::AppState;
use crate::ui::design;
use egui::{Color32, RichText, Ui};

pub fn render(ui: &mut Ui, state: &mut AppState) {
    design::title(ui, "Control Room Settings", design::ACCENT_CYAN);
    ui.label(
        RichText::new("Runtime paths, browser routing and API credentials")
            .color(design::TEXT_MUTED),
    );
    ui.add_space(10.0);

    // KOD NOTU: Grid layout ile label-input çiftleri hizalanır.
    // desired_width(f32::INFINITY) yerine available_width oranı kullanılır —
    // böylece dar pencerede input taşmaz.
    design::section_frame().show(ui, |ui| {
        ui.vertical(|ui| {
            ui.label(
                RichText::new("Environment & Paths")
                    .strong()
                    .color(design::ACCENT_ORANGE),
            );
            ui.add_space(6.0);

            egui::Grid::new("settings_grid")
                .num_columns(3)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    let input_w = (ui.available_width() * 0.6).clamp(200.0, 500.0);

                    // Output dir
                    ui.label(RichText::new("Output Dir:").color(design::TEXT_MUTED));
                    ui.add(
                        egui::Label::new(
                            RichText::new(
                                state.config.output_dir.to_string_lossy().as_ref(),
                            )
                            .small()
                            .color(design::TEXT_MUTED),
                        )
                        .truncate(),
                    );
                    if ui.button("Browse…").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            state.config.output_dir = path;
                        }
                    }
                    ui.end_row();

                    // Chrome Binary
                    ui.label(RichText::new("Chrome Binary:").color(design::TEXT_MUTED));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.config.chrome_binary_path)
                            .desired_width(input_w),
                    );
                    ui.label(""); // placeholder
                    ui.end_row();

                    // Chrome Profile
                    ui.label(RichText::new("Chrome Profile:").color(design::TEXT_MUTED));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.config.chrome_profile_path)
                            .desired_width(input_w),
                    );
                    ui.label("");
                    ui.end_row();
                });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);

            ui.label(
                RichText::new("AI Configuration")
                    .strong()
                    .color(design::ACCENT_ORANGE),
            );
            ui.add_space(6.0);

            egui::Grid::new("ai_settings_grid")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    let input_w = (ui.available_width() * 0.65).clamp(200.0, 450.0);
                    ui.label(RichText::new("Gemini API Key:").color(design::TEXT_MUTED));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.config.gemini_api_key)
                            .password(true)
                            .desired_width(input_w),
                    );
                    ui.end_row();
                });

            ui.add_space(16.0);
            if ui
                .add(
                    egui::Button::new(
                        RichText::new("💾  Save Runtime Config")
                            .strong()
                            .color(Color32::BLACK),
                    )
                    .fill(design::ACCENT_GREEN)
                    .min_size([180.0, design::BUTTON_HEIGHT].into()),
                )
                .clicked()
            {
                state.notify(
                    crate::state::NotificationLevel::Ok,
                    "Settings",
                    "Configuration updated for current session.",
                );
            }
        });
    });
}
