use crate::state::AppState;
use egui::{Ui, RichText, Color32};

pub fn render(ui: &mut Ui, state: &mut AppState) {
    ui.heading("STUDIO CONFIGURATION 1.1.0");
    ui.add_space(10.0);

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.group(|ui| {
            ui.label(RichText::new("Version Control").strong());
            ui.horizontal(|ui| {
                ui.label("Config Schema Version:");
                ui.label(RichText::new(state.config.config_version.to_string()).color(Color32::YELLOW));
            });
        });

        ui.add_space(10.0);

        ui.group(|ui| {
            ui.label(RichText::new("Engine Settings").strong());
            
            ui.label("Remote Debug Port:");
            ui.add(egui::DragValue::new(&mut state.config.remote_debug_port).range(1024..=65535));

            ui.add_space(5.0);
            
            ui.label("Unified Output Directory:");
            ui.horizontal(|ui| {
                ui.label(state.config.output_dir.to_string_lossy());
                if ui.button("Change").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        state.config.output_dir = path;
                    }
                }
            });
        });

        ui.add_space(10.0);

        ui.group(|ui| {
            ui.label(RichText::new("Gemini AI API Settings").strong());
            ui.label("API KEY:");
            ui.add(egui::TextEdit::singleline(&mut state.config.gemini_api_key).password(true));
        });

        ui.add_space(20.0);

        ui.horizontal(|ui| {
            if ui.button(RichText::new("💾 SAVE ALL TO .env").strong().size(18.0)).clicked() {
                match crate::config::loader::save_config(&state.config) {
                    Ok(_) => {
                        state.notify("Success", "Configuration persisted to .env", false);
                    },
                    Err(e) => {
                        state.notify("Error", &format!("Save failed: {}", e), true);
                    }
                }
            }
        });
    });
}
