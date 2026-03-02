use crate::state::{AppState, AutomationStep, AutomationStatus};
use egui::{Ui, Color32, RichText};
use crate::core::events::AppEvent;
use crate::ui::scrape::emit;

pub fn render_embedded(ui: &mut Ui, state: &mut AppState, tid: &str) {
    if !state.workspaces.contains_key(tid) { return; }
    let ws = state.workspaces.get_mut(tid).unwrap();

    // PIPELINE BUILDER
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("🤖 AUTOMATION PIPELINE").strong().color(Color32::KHAKI));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button("➕ Add Step", |ui| {
                    if ui.button("Navigate").clicked() { ws.auto_steps.push(AutomationStep::Navigate(String::new())); ui.close_menu(); }
                    if ui.button("Click").clicked() { ws.auto_steps.push(AutomationStep::Click(String::new())); ui.close_menu(); }
                    if ui.button("Wait").clicked() { ws.auto_steps.push(AutomationStep::Wait(1)); ui.close_menu(); }
                    if ui.button("Scroll Bottom").clicked() { ws.auto_steps.push(AutomationStep::ScrollBottom); ui.close_menu(); }
                    if ui.button("Extract Text").clicked() { ws.auto_steps.push(AutomationStep::ExtractText(String::new())); ui.close_menu(); }
                });
                
                if ui.button("💾 Save DSL").on_hover_text("Save this automation pipeline as JSON").clicked() {
                    let dsl = crate::core::automation::dsl::AutomationDsl {
                        dsl_version: 1,
                        steps: ws.auto_steps.iter().map(|s| match s {
                            AutomationStep::Navigate(u) => crate::core::automation::dsl::Step::Navigate { url: u.clone() },
                            AutomationStep::Click(sel) => crate::core::automation::dsl::Step::Click { selector: sel.clone() },
                            AutomationStep::Wait(secs) => crate::core::automation::dsl::Step::WaitFor { selector: "body".into(), timeout_ms: Some(secs * 1000) },
                            AutomationStep::ScrollBottom => crate::core::automation::dsl::Step::ScrollBottom,
                            AutomationStep::ExtractText(sel) => crate::core::automation::dsl::Step::Extract { selector: sel.clone(), as_key: "data".into() },
                            _ => crate::core::automation::dsl::Step::ScrollBottom, // Default fallback
                        }).collect(),
                    };
                    if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).set_file_name("automation.json").save_file() {
                        if let Ok(json) = serde_json::to_string_pretty(&dsl) {
                            let _ = std::fs::write(path, json);
                        }
                    }
                }

                if ui.button("📁 Load DSL").on_hover_text("Load an automation pipeline from JSON").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() {
                        if let Ok(content) = std::fs::read_to_string(path) {
                            if let Ok(dsl) = serde_json::from_str::<crate::core::automation::dsl::AutomationDsl>(&content) {
                                ws.auto_steps = dsl.steps.into_iter().map(|s| match s {
                                    crate::core::automation::dsl::Step::Navigate { url } => AutomationStep::Navigate(url),
                                    crate::core::automation::dsl::Step::Click { selector } => AutomationStep::Click(selector),
                                    crate::core::automation::dsl::Step::ScrollBottom => AutomationStep::ScrollBottom,
                                    crate::core::automation::dsl::Step::Extract { selector, .. } => AutomationStep::ExtractText(selector),
                                    crate::core::automation::dsl::Step::WaitFor { timeout_ms, .. } => AutomationStep::Wait(timeout_ms.unwrap_or(1000) / 1000),
                                    _ => AutomationStep::Wait(1),
                                }).collect();
                            }
                        }
                    }
                }

                if ui.button("🗑 Clear").clicked() { ws.auto_steps.clear(); }
            });
        });

        ui.add_space(5.0);

        let mut to_remove = None;
        egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
            if ws.auto_steps.is_empty() {
                ui.centered_and_justified(|ui| { ui.label(RichText::new("No steps added yet.").italics().color(Color32::DARK_GRAY)); });
            }
            for (idx, step) in ws.auto_steps.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{}.", idx + 1)).strong());
                    match step {
                        AutomationStep::Navigate(url) => { ui.label("Nav:"); ui.text_edit_singleline(url); }
                        AutomationStep::Click(sel) => { ui.label("Clk:"); ui.text_edit_singleline(sel); }
                        AutomationStep::Wait(secs) => { ui.label("W8:"); ui.add(egui::DragValue::new(secs).range(1..=60)); ui.label("s"); }
                        AutomationStep::ExtractText(sel) => { ui.label("Ext:"); ui.text_edit_singleline(sel); }
                        AutomationStep::ScrollBottom => { ui.label("Scroll to bottom"); }
                        _ => {}
                    }
                    if ui.button("❌").clicked() { to_remove = Some(idx); }
                });
            }
        });
        if let Some(idx) = to_remove { ws.auto_steps.remove(idx); }

        ui.add_space(8.0);
        let can_run = ws.auto_status == AutomationStatus::Idle && !ws.auto_steps.is_empty();
        let btn_text = match &ws.auto_status {
            AutomationStatus::Idle => "▶ RUN AUTOMATION".to_string(),
            AutomationStatus::Running(i) => format!("🏃 STEP {} IN PROGRESS...", i + 1),
            AutomationStatus::Finished => "✅ FINISHED (Run Again)".to_string(),
            AutomationStatus::Error(e) => format!("❌ ERROR: {}", e),
        };

        if ui.add_enabled(can_run || ws.auto_status == AutomationStatus::Finished, 
            egui::Button::new(RichText::new(btn_text).strong())
                .min_size([ui.available_width(), 35.0].into())
                .fill(if can_run { Color32::from_rgb(40, 80, 40) } else { Color32::from_rgb(60, 60, 60) }))
            .clicked() {
            ws.auto_status = AutomationStatus::Running(0);
            emit(AppEvent::RequestAutomationRun(tid.to_string(), ws.auto_steps.clone()));
        }
    });

    ui.add_space(10.0);

    // LIVE SCRIPT INJECTION (Embedded)
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("🚀 SCRIPT INJECTION").strong().color(Color32::KHAKI));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("📁 Load File").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("JavaScript", &["js"]).pick_file() {
                        if let Ok(content) = std::fs::read_to_string(path) {
                            ws.js_script = content;
                        }
                    }
                }
                if ui.button("🗑 Clear").clicked() { ws.js_script.clear(); }
            });
        });
        
        ui.add_space(5.0);
        ui.add(egui::TextEdit::multiline(&mut ws.js_script)
            .font(egui::FontId::monospace(12.0))
            .desired_rows(6)
            .desired_width(f32::INFINITY));
            
        if ui.button("▶ EXECUTE IN TAB").clicked() {
            emit(AppEvent::RequestScriptExecution(tid.to_string(), ws.js_script.clone()));
        }
        
        if !ws.js_result.is_empty() {
            ui.add_space(5.0);
            ui.label(RichText::new(format!("Last Result: {}", ws.js_result)).color(Color32::GREEN).small());
        }
    });
}
