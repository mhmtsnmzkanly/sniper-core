use crate::state::AppState;
use crate::ui::design;
use egui::{Color32, RichText, Ui};

pub fn render(ui: &mut Ui, state: &mut AppState) {
    design::title(ui, "Control Room Settings", design::ACCENT_CYAN);
    ui.label(RichText::new("Runtime paths, browser routing and API credentials").color(design::TEXT_MUTED));
    ui.add_space(12.0);

    design::section_frame().show(ui, |ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new("Environment & Paths").strong().color(design::ACCENT_ORANGE));
            ui.add_space(5.0);

            ui.horizontal(|ui| {
                ui.label("Output Directory:");
                ui.label(RichText::new(format!("{:?}", state.config.output_dir)).color(design::TEXT_MUTED));
                if ui.button("Change...").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        state.config.output_dir = path;
                    }
                }
            });

            ui.horizontal(|ui| {
                ui.label("Chrome Binary:");
                ui.add(egui::TextEdit::singleline(&mut state.config.chrome_binary_path).desired_width(400.0));
            });

            ui.horizontal(|ui| {
                ui.label("Chrome Profile:");
                ui.add(egui::TextEdit::singleline(&mut state.config.chrome_profile_path).desired_width(400.0));
            });

            ui.separator();

            ui.label(RichText::new("AI Configuration").strong().color(design::ACCENT_ORANGE));
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.label("Gemini API Key:");
                ui.add(egui::TextEdit::singleline(&mut state.config.gemini_api_key).password(true).desired_width(300.0));
            });

            ui.add_space(20.0);
            if ui.add(egui::Button::new(RichText::new("Save Runtime Config").strong().color(Color32::BLACK)).fill(design::ACCENT_GREEN)).clicked() {
                // Persistent saving logic could be added here (e.g., settings.json)
                state.notify(crate::state::NotificationLevel::Ok, "Settings", "Configuration updated for current session.");
            }
        });
    });
}
