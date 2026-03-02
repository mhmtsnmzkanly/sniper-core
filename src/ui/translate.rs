use crate::state::AppState;
use egui::Ui;
use rfd::FileDialog;

pub fn render(ui: &mut Ui, state: &mut AppState) {
    ui.heading("AI TRANSLATION (Gemini)");
    ui.add_space(10.0);

    // Raw Folder Selection
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label("RAW FOLDER:");
            if ui.button("Browse").clicked() {
                if let Some(path) = FileDialog::new().pick_folder() {
                    state.config.raw_output_dir = path;
                }
            }
            ui.label(state.config.raw_output_dir.to_string_lossy());
        });
    });

    ui.add_space(5.0);

    // Translated Folder Selection
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label("OUTPUT FOLDER:");
            if ui.button("Browse").clicked() {
                if let Some(path) = FileDialog::new().pick_folder() {
                    state.config.translator_output_dir = path;
                }
            }
            ui.label(state.config.translator_output_dir.to_string_lossy());
        });
    });

    ui.add_space(10.0);

    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label("GEMINI API KEY:");
            ui.add(egui::TextEdit::singleline(&mut state.config.gemini_api_key).password(true));
        });
    });

    ui.add_space(15.0);

    if state.is_translating {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Translating...");
        });
    } else {
        let can_translate = !state.config.gemini_api_key.is_empty();
        let btn = ui.add_enabled(can_translate, egui::Button::new("START TRANSLATION").min_size([ui.available_width(), 40.0].into()));
        
        if btn.clicked() {
            let raw = state.config.raw_output_dir.clone();
            let trans = state.config.translator_output_dir.clone();
            let api_key = state.config.gemini_api_key.clone();
            state.is_translating = true;
            
            // TODO: Arka planda translator workflow başlat (core/translator üzerinden)
            tracing::info!("Translation requested from {:?} to {:?}", raw, trans);
        }
    }
}
