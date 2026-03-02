use crate::state::AppState;
use egui::Ui;

pub fn render(ui: &mut Ui, state: &mut AppState) {
    ui.heading("Global Settings");
    ui.add_space(10.0);

    ui.group(|ui| {
        ui.label(egui::RichText::new("Application Context").strong());
        ui.label(format!("Config Version: {}", state.config.config_version));
        ui.label(format!("Session ID: {}", state.session_timestamp));
    });

    ui.add_space(10.0);

    ui.group(|ui| {
        ui.label(egui::RichText::new("Engine Defaults").strong());
        ui.horizontal(|ui| {
            ui.label("Default URL:");
            ui.text_edit_singleline(&mut state.config.default_launch_url);
        });
        ui.checkbox(&mut state.config.headless, "Run Headless (Not recommended for bypass)");
    });
}
