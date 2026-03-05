use crate::core::events::AppEvent;
use crate::core::scripting::types::ScriptPackage;
use crate::state::AppState;
use crate::ui::design;
use crate::ui::scrape::emit;
use egui::{Color32, RichText, Ui};

pub fn render(ui: &mut Ui, state: &mut AppState) {
    design::title(ui, "Scripting Studio", design::ACCENT_CYAN);
    ui.label(
        RichText::new("Rhai tabanli script editoru. Automation runtime ile ortak calisir.")
            .small()
            .color(design::TEXT_MUTED),
    );
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        if ui.button("New").clicked() {
            // KOD NOTU: New ile temiz package baslatilir, eski output listesi de sifirlanir.
            state.script_package = ScriptPackage::default();
            state.script_error = None;
        }

        if ui.button("Import .json").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .set_directory(state.config.output_dir.join("script"))
                .add_filter("Script Package", &["json"])
                .pick_file()
            {
                emit(AppEvent::RequestScriptingImport(path));
            }
        }

        if ui.button("Export .json").clicked() {
            let default_name = format!("{}.json", state.script_package.name);
            if let Some(path) = rfd::FileDialog::new()
                .set_directory(state.config.output_dir.join("script"))
                .set_file_name(default_name)
                .add_filter("Script Package", &["json"])
                .save_file()
            {
                emit(AppEvent::RequestScriptingExport(path, state.script_package.clone()));
            }
        }

        if ui
            .add_enabled(!state.is_script_running, egui::Button::new("Execute"))
            .clicked()
        {
            state.script_error = None;
            let selected = state
                .scripting_tab_binding
                .clone()
                .or_else(|| state.selected_tab_id.clone());
            emit(AppEvent::RequestScriptingRun(
                state.script_package.clone(),
                selected,
            ));
        }
        if ui
            .add_enabled(!state.is_script_running, egui::Button::new("Check"))
            .clicked()
        {
            let selected = state
                .scripting_tab_binding
                .clone()
                .or_else(|| state.selected_tab_id.clone());
            emit(AppEvent::RequestScriptingCheck(
                state.script_package.clone(),
                selected,
            ));
        }
        if ui
            .add_enabled(!state.is_script_running, egui::Button::new("Dry-Run"))
            .clicked()
        {
            let selected = state
                .scripting_tab_binding
                .clone()
                .or_else(|| state.selected_tab_id.clone());
            emit(AppEvent::RequestScriptingDryRun(
                state.script_package.clone(),
                selected,
            ));
        }

        if ui
            .add_enabled(state.is_script_running, egui::Button::new("Stop"))
            .clicked()
        {
            emit(AppEvent::RequestScriptingStop);
        }
    });

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label("Script Name:");
        ui.text_edit_singleline(&mut state.script_package.name);
    });
    ui.horizontal(|ui| {
        ui.label("Description:");
        ui.text_edit_singleline(&mut state.script_package.description);
    });
    ui.horizontal(|ui| {
        ui.label("Entry:");
        ui.text_edit_singleline(&mut state.script_package.entry);
        if state.is_script_running {
            ui.label(RichText::new("RUNNING").color(Color32::LIGHT_GREEN).strong());
        }
    });
    ui.horizontal(|ui| {
        ui.label("Execution Target:");
        let selected_text = state
            .scripting_tab_binding
            .as_ref()
            .and_then(|id| state.available_tabs.iter().find(|t| &t.id == id).map(|t| t.title.clone()))
            .unwrap_or_else(|| "Use current selection".to_string());
        ui.small("(TabCatch() bu secimi kullanir)");
        egui::ComboBox::from_id_salt("script_bound_tab")
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(state.scripting_tab_binding.is_none(), "Use current selection")
                    .clicked()
                {
                    state.scripting_tab_binding = None;
                }
                for tab in &state.available_tabs {
                    if ui
                        .selectable_label(
                            state.scripting_tab_binding.as_ref() == Some(&tab.id),
                            format!("{} ({})", tab.title, tab.id),
                        )
                        .clicked()
                    {
                        state.scripting_tab_binding = Some(tab.id.clone());
                    }
                }
            });
    });

    ui.separator();
    ui.label(RichText::new("Code").strong());
    ui.add(
        egui::TextEdit::multiline(&mut state.script_package.code)
            .desired_rows(20)
            .font(egui::TextStyle::Monospace)
            .desired_width(f32::INFINITY),
    );

    ui.separator();
    ui.label(RichText::new("Runtime").strong());
    if let Some(err) = &state.script_error {
        ui.colored_label(Color32::LIGHT_RED, format!("ERROR: {}", err));
    } else {
        ui.colored_label(Color32::LIGHT_GREEN, "Script outputlari System Telemetry panelinde listelenir.");
    }
}
