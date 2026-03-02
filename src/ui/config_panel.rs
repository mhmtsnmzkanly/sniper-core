use crate::state::AppState;
use egui::{Ui, RichText, Color32};

pub fn render(ui: &mut Ui, state: &mut AppState) {
    ui.heading("STUDIO CONFIGURATION (.env)");
    ui.add_space(10.0);

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.group(|ui| {
            ui.label(RichText::new("Version Control").strong());
            ui.horizontal(|ui| {
                ui.label("Config Version:");
                ui.label(RichText::new(state.config.config_version.to_string()).color(Color32::YELLOW));
                ui.label("(Internal schema version used for migrations)");
            });
        });

        ui.add_space(10.0);

        ui.group(|ui| {
            ui.label(RichText::new("Engine Settings").strong());
            
            ui.label("Default Launch URL:");
            ui.text_edit_singleline(&mut state.config.default_launch_url);
            ui.label(RichText::new("The URL opened when 'Launch Browser' is clicked without input.").small().italics());

            ui.add_space(5.0);
            
            ui.label("Default Profile Directory:");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut state.config.default_profile_dir.to_string_lossy().into_owned());
                if ui.button("Browse").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        state.config.default_profile_dir = path;
                    }
                }
            });
            ui.label(RichText::new("Path where browser cookies and history are stored.").small().italics());

            ui.add_space(5.0);

            ui.label("Remote Debug Port:");
            ui.add(egui::DragValue::new(&mut state.config.remote_debug_port).range(1024..=65535));
            ui.label(RichText::new("Port used for CDP communication (Default: 9222).").small().italics());
        });

        ui.add_space(10.0);

        ui.group(|ui| {
            ui.label(RichText::new("Downloader & Mirror Settings").strong());
            
            ui.horizontal(|ui| {
                ui.label("Download Timeout (sec):");
                ui.add(egui::DragValue::new(&mut state.config.download_timeout));
            });

            ui.horizontal(|ui| {
                ui.label("Max Parallel Downloads:");
                ui.add(egui::DragValue::new(&mut state.config.max_concurrent_download).range(1..=32));
            });

            ui.add_space(5.0);

            ui.label("Raw HTML Output Dir:");
            ui.horizontal(|ui| {
                ui.label(state.config.raw_output_dir.to_string_lossy());
                if ui.button("Change").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        state.config.raw_output_dir = path;
                    }
                }
            });
        });

        ui.add_space(10.0);

        ui.group(|ui| {
            ui.label(RichText::new("Gemini AI API Settings").strong());
            
            ui.label("API URL:");
            ui.text_edit_singleline(&mut state.config.gemini_api_url);

            ui.add_space(5.0);

            ui.label("API KEY:");
            ui.add(egui::TextEdit::singleline(&mut state.config.gemini_api_key).password(true));
            ui.label(RichText::new("Required for the TRANSLATE tab operations.").small().italics());
        });

        ui.add_space(20.0);

        ui.horizontal(|ui| {
            if ui.button(RichText::new("💾 SAVE ALL TO .env").strong().size(18.0)).clicked() {
                match crate::config::loader::save_config(&state.config) {
                    Ok(_) => {
                        state.notify("Success", "Configuration saved to .env file.", false);
                        tracing::info!("Configuration manually saved to .env.");
                    },
                    Err(e) => {
                        state.notify("Error", &format!("Failed to save: {}", e), true);
                        tracing::error!("Failed to save config: {}", e);
                    }
                }
            }
            
            if ui.button("Reset to Defaults").clicked() {
                state.config = crate::config::AppConfig::default();
                tracing::warn!("Configuration reset to default values.");
            }
        });
    });
}
