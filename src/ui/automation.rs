use crate::state::{AppState, AutomationStep, AutomationStatus};
use egui::{Ui, Color32, RichText};
use crate::core::events::AppEvent;
use crate::ui::scrape::emit;

pub fn render(ui: &mut Ui, state: &mut AppState) {
    ui.heading("AUTOMATION STUDIO");
    ui.add_space(10.0);

    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Automation Pipeline").strong().size(16.0));
            if ui.button("➕ Add Step").clicked() {
                state.auto_steps.push(AutomationStep::Wait(1));
                tracing::info!("Automation step added.");
            }
            if ui.button("🗑 Clear All").clicked() {
                state.auto_steps.clear();
                tracing::warn!("All automation steps cleared.");
            }
        });

        ui.add_space(10.0);

        let mut to_remove = None;
        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
            for (idx, step) in state.auto_steps.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("{}.", idx + 1));
                    
                    match step {
                        AutomationStep::Navigate(url) => {
                            ui.label("Navigate to:");
                            ui.text_edit_singleline(url);
                        }
                        AutomationStep::Click(sel) => {
                            ui.label("Click Selector:");
                            ui.text_edit_singleline(sel);
                        }
                        AutomationStep::Wait(secs) => {
                            ui.label("Wait (secs):");
                            ui.add(egui::DragValue::new(secs).range(1..=60));
                        }
                        AutomationStep::ExtractText(sel) => {
                            ui.label("Extract Text:");
                            ui.text_edit_singleline(sel);
                        }
                        _ => { ui.label(format!("{:?}", step)); }
                    }

                    if ui.button("❌").clicked() {
                        to_remove = Some(idx);
                    }
                });
                ui.add_space(5.0);
            }
        });

        if let Some(idx) = to_remove {
            state.auto_steps.remove(idx);
            tracing::info!("Step {} removed.", idx + 1);
        }

        ui.add_space(10.0);
        
        let can_run = state.selected_tab_id.is_some() && state.auto_status == AutomationStatus::Idle;
        let btn_text = match &state.auto_status {
            AutomationStatus::Idle => "▶ START PIPELINE",
            AutomationStatus::Running(_i) => "RUNNING...",
            AutomationStatus::Finished => "PIPELINE FINISHED",
            AutomationStatus::Error(_) => "PIPELINE FAILED",
        };

        if ui.add_enabled(can_run, egui::Button::new(RichText::new(btn_text).strong()).min_size([ui.available_width(), 40.0].into())).clicked() {
            if let Some(tab_id) = state.selected_tab_id.clone() {
                tracing::info!("Automation PIPELINE started with {} steps.", state.auto_steps.len());
                state.auto_status = AutomationStatus::Running(0);
                emit(AppEvent::RequestAutomationRun(tab_id, state.auto_steps.clone()));
            }
        }
    });

    ui.add_space(20.0);

    ui.group(|ui| {
        ui.label(RichText::new("Live Script Injection").strong());
        ui.horizontal(|ui| {
            if ui.button("📁 Load JS File").clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter("JavaScript", &["js"]).pick_file() {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        state.js_script = content;
                        tracing::info!("JS Script loaded from: {:?}", path);
                    }
                }
            }
            if ui.button("Clear Editor").clicked() {
                state.js_script.clear();
            }
        });
        
        ui.add_space(5.0);
        ui.add(egui::TextEdit::multiline(&mut state.js_script)
            .font(egui::TextStyle::Monospace)
            .desired_rows(8)
            .desired_width(f32::INFINITY));
        
        if ui.button("▶ INJECT SCRIPT").clicked() {
            if let Some(tab_id) = state.selected_tab_id.clone() {
                tracing::info!("Injecting live script (length: {} chars).", state.js_script.len());
                emit(AppEvent::RequestScriptExecution(tab_id, state.js_script.clone()));
            } else {
                tracing::error!("No tab selected for script injection!");
            }
        }
        
        if !state.js_result.is_empty() {
            ui.add_space(10.0);
            ui.label("Result:");
            ui.add(egui::Label::new(RichText::new(&state.js_result).color(Color32::GREEN).monospace()).selectable(true));
        }
    });
}
