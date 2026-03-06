use crate::core::events::AppEvent;
use crate::core::scripting::types::ScriptPackage;
use crate::core::scripting::templates;
use crate::state::AppState;
use crate::ui::design;
use crate::ui::scrape::emit;
use egui::{Color32, Frame, RichText, Stroke, Ui, text::LayoutJob};

/// KOD NOTU: Rhai sözdizimi için basit bir renklendirici (Syntax Highlighter).
fn rhai_highlighter(ui: &Ui, code: &str) -> LayoutJob {
    let mut job = LayoutJob::default();
    let font_id = egui::TextStyle::Monospace.resolve(ui.style());

    // Basit tokenlaştırma (Regex yerine daha hızlı manuel tarama)
    let keywords = [
        "fn", "let", "const", "if", "else", "for", "loop", "while", "return", "break", "continue", "import", "as", "export",
    ];
    let browser_apis = [
        "Tab", "TabNew", "TabCatch", "TabCurrent", "navigate", "click", "type", "wait_for_ms", "screenshot", "find_el",
        "capture", "html", "mirror", "complete", "console", "inject", "network", "cookies", "log", "fs_write_text", "fs_append_text",
    ];

    let mut it = code.chars().peekable();
    while let Some(c) = it.next() {
        if c == '/' && it.peek() == Some(&'/') { // Comment
            let mut s = format!("{}", c);
            while let Some(&nc) = it.peek() {
                if nc == '\n' { break; }
                s.push(it.next().unwrap());
            }
            job.append(&s, 0.0, egui::TextFormat { color: Color32::from_rgb(100, 150, 100), font_id: font_id.clone(), ..Default::default() });
        } else if c == '"' || c == '`' || c == '\'' { // String
            let quote = c;
            let mut s = format!("{}", c);
            while let Some(&nc) = it.peek() {
                s.push(it.next().unwrap());
                if nc == quote { break; }
            }
            job.append(&s, 0.0, egui::TextFormat { color: Color32::from_rgb(200, 150, 100), font_id: font_id.clone(), ..Default::default() });
        } else if c.is_alphabetic() || c == '_' { // Word
            let mut s = format!("{}", c);
            while let Some(&nc) = it.peek() {
                if nc.is_alphanumeric() || nc == '_' { s.push(it.next().unwrap()); } else { break; }
            }
            let color = if keywords.contains(&s.as_str()) {
                Color32::from_rgb(80, 150, 255) // Keyword
            } else if browser_apis.contains(&s.as_str()) {
                Color32::from_rgb(255, 180, 100) // API
            } else {
                Color32::from_rgb(200, 200, 200) // Default
            };
            job.append(&s, 0.0, egui::TextFormat { color, font_id: font_id.clone(), ..Default::default() });
        } else if c.is_numeric() { // Number
            let mut s = format!("{}", c);
            while let Some(&nc) = it.peek() {
                if nc.is_numeric() || nc == '.' { s.push(it.next().unwrap()); } else { break; }
            }
            job.append(&s, 0.0, egui::TextFormat { color: Color32::from_rgb(180, 130, 220), font_id: font_id.clone(), ..Default::default() });
        } else { // Symbol/Punctuation
            let color = if "(){}[].,;".contains(c) { Color32::from_gray(140) } else { Color32::from_gray(200) };
            job.append(&c.to_string(), 0.0, egui::TextFormat { color, font_id: font_id.clone(), ..Default::default() });
        }
    }
    job
}

pub fn render(ui: &mut Ui, state: &mut AppState) {
    design::title(ui, "Scripting Studio", design::ACCENT_CYAN);
    ui.label(
        RichText::new("Rhai tabanlı script editörü. Browser komutları (click, type vb.) arka planda Automation Runtime (DSL) ile ortak çalışır.")
            .small()
            .color(design::TEXT_MUTED),
    );
    ui.add_space(8.0);

    let template_library = templates::library();
    let panel_stroke = Stroke::new(1.0, Color32::from_rgb(42, 64, 78));

    // ── Araç Çubuğu ──────────────────────────────────────────────────
    Frame::group(ui.style())
        .fill(design::BG_SURFACE)
        .stroke(panel_stroke)
        .corner_radius(8.0)
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("🆕 New").clicked() {
                    state.script_package = ScriptPackage::default();
                    state.script_error = None;
                    state.scripting_debug_plan.clear();
                    state.scripting_debug_index = 0;
                }
                if ui.button("📂 Import").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_directory(state.config.output_dir.join("script"))
                        .add_filter("Script Package", &["json"])
                        .pick_file()
                    {
                        emit(AppEvent::RequestScriptingImport(path));
                    }
                }
                if ui.button("💾 Export").clicked() {
                    let default_name = format!("{}.json", state.script_package.name);
                    if let Some(path) = rfd::FileDialog::new()
                        .set_directory(state.config.output_dir.join("script"))
                        .set_file_name(default_name)
                        .add_filter("Script Package", &["json"])
                        .save_file()
                    {
                        emit(AppEvent::RequestScriptingExport(
                            path,
                            state.script_package.clone(),
                        ));
                    }
                }

                ui.separator();

                if ui
                    .add_enabled(!state.is_script_running, egui::Button::new("▶ Execute"))
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
                    .add_enabled(!state.is_script_running, egui::Button::new("✓ Check"))
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
                    .add_enabled(!state.is_script_running, egui::Button::new("🧪 Dry-Run"))
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
                    .add_enabled(!state.is_script_running, egui::Button::new("🔎 Debugger"))
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
                    .add_enabled(state.is_script_running, egui::Button::new("⏹ Stop"))
                    .clicked()
                {
                    emit(AppEvent::RequestScriptingStop);
                }

                if state.is_script_running {
                    ui.add(egui::Spinner::new().size(14.0));
                    ui.label(
                        RichText::new("RUNNING")
                            .color(Color32::LIGHT_GREEN)
                            .strong(),
                    );
                }
            });
        });

    ui.add_space(6.0);

    // ── Meta Bilgi ────────────────────────────────────────────────────
    Frame::group(ui.style())
        .fill(design::BG_SURFACE)
        .stroke(panel_stroke)
        .corner_radius(8.0)
        .inner_margin(8.0)
        .show(ui, |ui| {
            egui::Grid::new("scripting_meta_grid")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    // Template
                    ui.label(RichText::new("Template:").color(design::TEXT_MUTED));
                    ui.horizontal(|ui| {
                        let selected_template = template_library
                            .iter()
                            .find(|t| t.id == state.scripting_template_id)
                            .map(|t| t.title.clone())
                            .unwrap_or_else(|| "Select template".to_string());
                        egui::ComboBox::from_id_salt("scripting_template_library")
                            .width(180.0)
                            .selected_text(selected_template)
                            .show_ui(ui, |ui| {
                                for template in &template_library {
                                    if ui
                                        .selectable_label(
                                            state.scripting_template_id == template.id,
                                            &template.title,
                                        )
                                        .on_hover_text(&template.description)
                                        .clicked()
                                    {
                                        state.scripting_template_id = template.id.clone();
                                    }
                                }
                            });
                        if ui.button("Apply").clicked() {
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
                    ui.end_row();

                    // Name
                    ui.label(RichText::new("Name:").color(design::TEXT_MUTED));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.script_package.name)
                            .desired_width(f32::INFINITY),
                    );
                    ui.end_row();

                    // Entry
                    ui.label(RichText::new("Entry fn:").color(design::TEXT_MUTED));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.script_package.entry)
                            .desired_width(120.0),
                    );
                    ui.end_row();

                    // Tab binding
                    ui.label(RichText::new("Target Tab:").color(design::TEXT_MUTED));
                    let selected_text = state
                        .scripting_tab_binding
                        .as_ref()
                        .and_then(|id| {
                            state
                                .available_tabs
                                .iter()
                                .find(|t| &t.id == id)
                                .map(|t| t.title.clone())
                        })
                        .unwrap_or_else(|| "Use current selection".to_string());
                    egui::ComboBox::from_id_salt("script_bound_tab")
                        .width(220.0)
                        .selected_text(selected_text)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(
                                    state.scripting_tab_binding.is_none(),
                                    "Use current selection",
                                )
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
                    ui.end_row();

                    // Preflight
                    ui.label("");
                    ui.checkbox(
                        &mut state.scripting_check_preflight,
                        "Preflight: validate selectors on selected tab",
                    );
                    ui.end_row();
                });
        });

    ui.add_space(6.0);

    // ── Kod Editörü ───────────────────────────────────────────────────
    ui.label(RichText::new("Code Editor").strong().color(design::ACCENT_ORANGE));
    let editor_h = (ui.available_height() * 0.55).clamp(200.0, 600.0);
    
    Frame::canvas(ui.style())
        .fill(design::BG_PRIMARY)
        .stroke(panel_stroke)
        .corner_radius(4.0)
        .show(ui, |ui| {
            let avail_w = ui.available_width();
            
            egui::ScrollArea::vertical()
                .id_salt("script_editor_scroll")
                .max_height(editor_h)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // 1. Satır Numaraları
                        let line_count = state.script_package.code.lines().count().max(1);
                        let mut line_numbers = String::new();
                        for i in 1..=line_count {
                            line_numbers.push_str(&format!("{:>3}\n", i));
                        }
                        
                        ui.add_space(4.0);
                        ui.vertical(|ui| {
                            ui.add_space(2.0);
                            ui.label(RichText::new(line_numbers).monospace().color(Color32::from_gray(80)).line_height(Some(14.5)));
                        });
                        
                        ui.add_space(4.0);
                        ui.separator();
                        
                        // 2. Kod Alanı
                        let mut layouter = |ui: &Ui, string: &str, _wrap_width: f32| {
                            let mut job = rhai_highlighter(ui, string);
                            job.wrap.max_width = f32::INFINITY; 
                            ui.fonts(|f| f.layout_job(job))
                        };

                        ui.add(
                            egui::TextEdit::multiline(&mut state.script_package.code)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(avail_w)
                                .desired_rows(20)
                                .lock_focus(true)
                                .layouter(&mut layouter)
                                .frame(false)
                        );
                    });
                });
        });

    ui.add_space(6.0);

    // ── Debugger ──────────────────────────────────────────────────────
    ui.collapsing(
        RichText::new("Script Debugger").strong(),
        |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Break Condition:").color(design::TEXT_MUTED));
                ui.add(
                    egui::TextEdit::singleline(&mut state.scripting_break_condition)
                        .hint_text("Action text contains... (e.g. Capture, RunDsl)")
                        .desired_width(ui.available_width() * 0.7),
                );
            });
            ui.checkbox(&mut state.scripting_emit_step_timing, "Emit step timing telemetry");
            ui.add_space(4.0);

            if state.scripting_debug_plan.is_empty() {
                ui.colored_label(
                    Color32::from_gray(150),
                    "No debug plan yet. Click Debugger to build step preview.",
                );
            } else {
                let max_idx = state.scripting_debug_plan.len().saturating_sub(1);
                if state.scripting_debug_index > max_idx {
                    state.scripting_debug_index = max_idx;
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            state.scripting_debug_index > 0,
                            egui::Button::new("◀ Prev"),
                        )
                        .clicked()
                    {
                        state.scripting_debug_index =
                            state.scripting_debug_index.saturating_sub(1);
                    }
                    if ui
                        .add_enabled(
                            state.scripting_debug_index + 1 < state.scripting_debug_plan.len(),
                            egui::Button::new("Next ▶"),
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
                    ui.colored_label(Color32::from_rgb(255, 200, 120), "⚡ Break condition matches this step.");
                }
                Frame::new()
                    .fill(design::BG_PRIMARY)
                    .corner_radius(6.0)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.monospace(&current_line);
                    });
            }
        },
    );

    ui.add_space(4.0);

    // ── Runtime Output ────────────────────────────────────────────────
    ui.collapsing(RichText::new("Runtime Output").strong(), |ui| {
        if let Some(err) = &state.script_error {
            ui.colored_label(
                Color32::from_rgb(255, 100, 100),
                format!("❌ ERROR: {}", err),
            );
        } else {
            ui.colored_label(design::ACCENT_GREEN, "✓ Output'lar System Telemetry panelinde listelenir.");
            ui.add_space(2.0);
            if let Some(last) = state.script_output.last() {
                Frame::new()
                    .fill(design::BG_PRIMARY)
                    .corner_radius(6.0)
                    .inner_margin(6.0)
                    .show(ui, |ui| {
                        ui.monospace(format!("Last: {}", last));
                    });
            }
        }
    });
}
