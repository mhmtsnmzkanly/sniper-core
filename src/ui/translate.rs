use crate::state::AppState;
use egui::{Ui, Color32, RichText};

pub fn render(ui: &mut Ui, state: &mut AppState) {
    ui.heading("AI TRANSLATION STUDIO");
    ui.add_space(10.0);

    ui.group(|ui| {
        ui.label(RichText::new("1. Folder Configuration").strong());
        
        ui.horizontal(|ui| {
            ui.label("Work Directory:");
            if ui.button("Select").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    state.config.output_dir = path;
                }
            }
            ui.label(state.config.output_dir.to_string_lossy());
        });
    });

    ui.add_space(10.0);

    ui.group(|ui| {
        ui.label(RichText::new("2. Translation Status").strong());
        ui.add_space(5.0);
        
        if state.is_translating {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new());
                ui.label("Processing files with Gemini AI...");
            });
        } else {
            ui.label("System Idle. Ready to translate local HTML files.");
        }
    });

    ui.add_space(20.0);

    let can_start = !state.is_translating && !state.config.gemini_api_key.is_empty();
    
    if ui.add_enabled(can_start, egui::Button::new(RichText::new("🚀 START BATCH TRANSLATION").strong().size(18.0))
        .min_size([ui.available_width(), 50.0].into())).clicked() {
            state.is_translating = true;
            // Background task logic would go here
    }

    if state.config.gemini_api_key.is_empty() {
        ui.add_space(5.0);
        ui.label(RichText::new("⚠ API Key missing in SETTINGS!").color(Color32::RED).small());
    }
}
