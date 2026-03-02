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
                    state.raw_path = Some(path);
                }
            }
            if let Some(path) = &state.raw_path {
                ui.label(egui::RichText::new(path.to_string_lossy()).small());
            }
        });
    });

    ui.add_space(5.0);

    // Translated Folder Selection
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label("OUTPUT FOLDER:");
            if ui.button("Browse").clicked() {
                if let Some(path) = FileDialog::new().pick_folder() {
                    state.trans_path = Some(path);
                }
            }
            if let Some(path) = &state.trans_path {
                ui.label(egui::RichText::new(path.to_string_lossy()).small());
            }
        });
    });

    ui.add_space(10.0);

    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label("GEMINI API KEY:");
            ui.add(egui::TextEdit::singleline(&mut state.gemini_api_key).password(true));
        });
    });

    ui.add_space(15.0);

    if state.is_translating {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Translating...");
        });
    } else {
        let can_translate = state.raw_path.is_some() && state.trans_path.is_some() && !state.gemini_api_key.is_empty();
        let btn = ui.add_enabled(can_translate, egui::Button::new("START TRANSLATION").min_size([ui.available_width(), 40.0].into()));
        
        if btn.clicked() {
            let raw = state.raw_path.clone().unwrap();
            let trans = state.trans_path.clone().unwrap();
            let api_key = state.gemini_api_key.clone();
            state.is_translating = true;
            
            tokio::spawn(async move {
                match crate::backend::GeminiClient::new(Some(api_key)) {
                    Ok(client) => {
                        if let Err(e) = client.run_translate_workflow(raw, trans, 3).await {
                            tracing::error!("Translation workflow error: {}", e);
                        }
                    }
                    Err(e) => tracing::error!("Gemini client error: {}", e),
                }
            });
        }
    }
}
