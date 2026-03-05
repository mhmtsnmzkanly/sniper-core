use crate::core::events::AppEvent;
use crate::core::scripting::types::ScriptPackage;
use crate::core::scripting::templates;
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
    let template_library = templates::library();

    ui.horizontal(|ui| {
        if ui.button("New").clicked() {
            // KOD NOTU: New ile temiz package baslatilir, eski output listesi de sifirlanir.
            state.script_package = ScriptPackage::default();
            state.script_error = None;
            state.scripting_debug_plan.clear();
            state.scripting_debug_index = 0;
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
            .add_enabled(!state.is_script_running, egui::Button::new("Debugger"))
            .clicked()
        {
            let selected = state
                .scripting_tab_binding
                .clone()
                .or_else(|| state.selected_tab_id.clone());
            emit(AppEvent::RequestScriptingDebugPlan(
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
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("Template:");
        let selected_template = template_library
            .iter()
            .find(|t| t.id == state.scripting_template_id)
            .map(|t| t.title.clone())
            .unwrap_or_else(|| "Select template".to_string());
        egui::ComboBox::from_id_salt("scripting_template_library")
            .selected_text(selected_template)
            .show_ui(ui, |ui| {
                for template in &template_library {
                    if ui
                        .selectable_label(state.scripting_template_id == template.id, &template.title)
                        .on_hover_text(&template.description)
                        .clicked()
                    {
                        state.scripting_template_id = template.id.clone();
                    }
                }
            });
        if ui.button("Apply Template").clicked() {
            if let Some(template) = template_library
                .iter()
                .find(|t| t.id == state.scripting_template_id)
            {
                state.script_package = template.package.clone();
                state.script_error = None;
                state.scripting_debug_plan.clear();
                state.scripting_debug_index = 0;
            }
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
    ui.label(RichText::new("Script Debugger").strong());
    ui.horizontal(|ui| {
        ui.label("Break Condition:");
        ui.add(
            egui::TextEdit::singleline(&mut state.scripting_break_condition)
                .hint_text("Action text contains... (e.g. Capture, RunDsl, selector)")
                .desired_width(ui.available_width() * 0.7),
        );
    });
    ui.horizontal(|ui| {
        ui.checkbox(
            &mut state.scripting_emit_step_timing,
            "Emit step timing telemetry (TIMING lines)",
        );
    });
    if state.scripting_debug_plan.is_empty() {
        ui.colored_label(Color32::from_gray(170), "No debug plan yet. Click Debugger to build step preview.");
    } else {
        let max_idx = state.scripting_debug_plan.len().saturating_sub(1);
        if state.scripting_debug_index > max_idx {
            state.scripting_debug_index = max_idx;
        }

        ui.horizontal(|ui| {
            if ui
                .add_enabled(state.scripting_debug_index > 0, egui::Button::new("Prev"))
                .clicked()
            {
                state.scripting_debug_index = state.scripting_debug_index.saturating_sub(1);
            }
            if ui
                .add_enabled(
                    state.scripting_debug_index + 1 < state.scripting_debug_plan.len(),
                    egui::Button::new("Next"),
                )
                .clicked()
            {
                state.scripting_debug_index += 1;
            }
            ui.label(format!(
                "Step {}/{}",
                state.scripting_debug_index + 1,
                state.scripting_debug_plan.len()
            ));
        });
        ui.add_space(4.0);
        let current_line = state
            .scripting_debug_plan
            .get(state.scripting_debug_index)
            .cloned()
            .unwrap_or_default();
        let break_match = !state.scripting_break_condition.trim().is_empty()
            && current_line
                .to_ascii_lowercase()
                .contains(&state.scripting_break_condition.to_ascii_lowercase());
        if break_match {
            ui.colored_label(
                Color32::from_rgb(255, 200, 120),
                "Break condition matches this step.",
            );
        }
        ui.monospace(current_line);
    }

    ui.separator();
    ui.label(RichText::new("Runtime").strong());
    if let Some(err) = &state.script_error {
        ui.colored_label(Color32::LIGHT_RED, format!("ERROR: {}", err));
    } else {
        ui.colored_label(Color32::LIGHT_GREEN, "Script outputlari System Telemetry panelinde listelenir.");
        ui.small(format!("Buffered script lines: {}", state.script_output.len()));
        if let Some(last) = state.script_output.last() {
            ui.monospace(format!("Last: {}", last));
        }
    }
}
