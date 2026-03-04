use crate::state::AppState;
use egui::{Ui, Color32, RichText, Frame, Stroke};

pub fn render(ui: &mut Ui, state: &mut AppState) {
    ui.heading(RichText::new("SYSTEM SETTINGS").strong().color(Color32::WHITE));
    ui.add_space(10.0);

    let frame_style = Frame::group(ui.style()).fill(Color32::from_gray(25)).stroke(Stroke::new(1.0, Color32::from_gray(50)));

    frame_style.show(ui, |ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new("Environment & Paths").strong().color(Color32::LIGHT_BLUE));
            ui.add_space(5.0);

            ui.horizontal(|ui| {
                ui.label("Output Directory:");
                ui.label(RichText::new(format!("{:?}", state.config.output_dir)).color(Color32::KHAKI));
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

            ui.label(RichText::new("AI Configuration").strong().color(Color32::LIGHT_BLUE));
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.label("Gemini API Key:");
                ui.add(egui::TextEdit::singleline(&mut state.config.gemini_api_key).password(true).desired_width(300.0));
            });

            ui.add_space(20.0);
            if ui.button(RichText::new("💾 SAVE SETTINGS").strong().color(Color32::GREEN)).clicked() {
                // Persistent saving logic could be added here (e.g., settings.json)
                state.notify("Settings", "Configuration updated for current session.", false);
            }
        });
    });
}
