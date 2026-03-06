use crate::core::events::AppEvent;
use crate::core::scripting::types::ScriptPackage;
use crate::core::scripting::templates;
use crate::state::AppState;
use crate::ui::design;
use crate::ui::scrape::emit;
use egui::{Color32, Frame, RichText, Stroke, Ui, text::LayoutJob, Id};

const RHAI_KEYWORDS: &[&str] = &[
    "fn", "let", "const", "if", "else", "for", "loop", "while", "return", "break", "continue", "import", "as", "export",
];

const BROWSER_APIS: &[&str] = &[
    "Tab", "TabNew", "TabCatch", "TabCurrent", "navigate", "click", "type", "wait_for_ms", "screenshot", "find_el",
    "capture", "html", "mirror", "complete", "console", "inject", "network", "cookies", "log", "fs_write_text", "fs_append_text",
];

/// KOD NOTU: Otomatik tamamlama mantığını işler.
fn handle_autocomplete(ui: &mut Ui, state: &mut AppState, cursor_pos: usize) {
    let code = &state.script_package.code;
    if cursor_pos == 0 { return; }

    let last_char = code.chars().nth(cursor_pos.saturating_sub(1));
    let mut trigger = false;

    // Trigger on '.' or Ctrl+Space (handled via input check)
    if last_char == Some('.') {
        trigger = true;
    }

    if trigger || (ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Space))) {
        state.ide_autocomplete_open = true;
        state.ide_autocomplete_cursor = cursor_pos;
        state.ide_autocomplete_index = 0;
        
        // Basit öneri listesi (Gelecekte bağlama göre filtrelenebilir)
        state.ide_autocomplete_suggestions = BROWSER_APIS.iter().map(|s| s.to_string()).collect();
    }

    // Enter or Tab to apply
    if state.ide_autocomplete_open {
        if ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Tab)) {
            if let Some(suggestion) = state.ide_autocomplete_suggestions.get(state.ide_autocomplete_index) {
                let mut new_code = code.clone();
                new_code.insert_str(cursor_pos, suggestion);
                state.script_package.code = new_code;
                state.ide_autocomplete_open = false;
            }
        }
        if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
            state.ide_autocomplete_index = (state.ide_autocomplete_index + 1) % state.ide_autocomplete_suggestions.len().max(1);
        }
        if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
            state.ide_autocomplete_index = (state.ide_autocomplete_index + state.ide_autocomplete_suggestions.len().saturating_sub(1)) % state.ide_autocomplete_suggestions.len().max(1);
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            state.ide_autocomplete_open = false;
        }
    }
}

/// KOD NOTU: Parantez eşleştirme mantığı.
fn find_matching_brace(code: &str, cursor_idx: usize) -> Option<(usize, usize)> {
    if cursor_idx == 0 || code.is_empty() { return None; }
    
    let chars: Vec<char> = code.chars().collect();
    let idx = cursor_idx.saturating_sub(1);
    if idx >= chars.len() { return None; }
    
    let c = chars[idx];
    let (open, close, forward) = match c {
        '(' => ('(', ')', true),
        '{' => ('{', '}', true),
        '[' => ('[', ']', true),
        ')' => ('(', ')', false),
        '}' => ('{', '}', false),
        ']' => ('[', ']', false),
        _ => return None,
    };

    if forward {
        let mut depth = 0;
        for i in idx..chars.len() {
            if chars[i] == open { depth += 1; }
            else if chars[i] == close {
                depth -= 1;
                if depth == 0 { return Some((idx, i)); }
            }
        }
    } else {
        let mut depth = 0;
        for i in (0..=idx).rev() {
            if chars[i] == close { depth += 1; }
            else if chars[i] == open {
                depth -= 1;
                if depth == 0 { return Some((i, idx)); }
            }
        }
    }
    None
}

fn get_api_doc(api: &str) -> &'static str {
    match api {
        "Tab" => "Creates a new tab or attaches to one. Usage: let t = Tab(\"url\");",
        "TabNew" => "Force opens a new browser tab. Usage: let t = TabNew(\"url\");",
        "TabCatch" => "Attaches to the tab currently selected in the Ops UI.",
        "navigate" => "Navigates the tab to a new URL. Usage: tab.navigate(\"url\");",
        "click" => "Clicks an element identified by CSS selector. Usage: tab.click(\"#btn\");",
        "type" => "Types text into an input field. Usage: tab.type(\"input\", \"text\");",
        "wait_for_ms" => "Pauses script execution. Usage: tab.wait_for_ms(1000);",
        "screenshot" => "Captures a screenshot of the current page.",
        "capture" => "Sub-module for page capture (html, complete, mirror).",
        "html" => "Captures raw HTML of the page. Usage: tab.capture.html();",
        "mirror" => "Captures a mirror (MHTML/WebBundle) of the page.",
        "complete" => "Captures a full page archive with all assets.",
        "console" => "Access to browser console. Usage: tab.console.log(\"msg\");",
        "inject" => "Injects and executes custom JavaScript. Usage: tab.console.inject(\"code\");",
        "network" => "Network monitoring and interception APIs.",
        "cookies" => "Cookie management. Usage: tab.cookies.get_all();",
        "fs_write_text" => "Writes text to a file in output_dir. Usage: fs_write_text(\"path\", \"data\");",
        _ => "Sniper Browser API function.",
    }
}

/// KOD NOTU: Rhai sözdizimi için gelişmiş renklendirici.
fn rhai_highlighter(
    ui: &Ui, 
    code: &str, 
    highlight_braces: Option<(usize, usize)>,
    diagnostics: &[crate::core::scripting::types::ScriptDiagnostic]
) -> LayoutJob {
    let mut job = LayoutJob::default();
    let font_id = egui::TextStyle::Monospace.resolve(ui.style());

    // Satır başlangıç indekslerini hesapla (Hata vurgulama için)
    let mut line_starts = vec![0];
    for (i, c) in code.chars().enumerate() {
        if c == '\n' { line_starts.push(i + 1); }
    }

    let mut it = code.chars().enumerate().peekable();
    while let Some((idx, c)) = it.next() {
        // 1. Parantez eşleşme kontrolü
        let is_brace_match = highlight_braces.map_or(false, |(a, b)| idx == a || idx == b);
        
        // 2. Hata kontrolü (Bu karakter bir hata bölgesinde mi?)
        let mut error_color = None;
        for d in diagnostics {
            if let Some(line_idx) = d.line.map(|l| l.saturating_sub(1)) {
                if let Some(&start_pos) = line_starts.get(line_idx) {
                    let end_pos = line_starts.get(line_idx + 1).cloned().unwrap_or(code.len());
                    // Basitlik için tüm satırı vurgula veya kolonu kullan
                    if idx >= start_pos && idx < end_pos {
                        error_color = Some(Color32::from_rgb(180, 50, 50));
                    }
                }
            }
        }

        let mut base_format = egui::TextFormat { 
            font_id: font_id.clone(), 
            ..Default::default() 
        };

        if is_brace_match {
            base_format.background = Color32::from_rgb(60, 80, 100);
            base_format.color = Color32::WHITE;
        }
        
        if let Some(ec) = error_color {
            base_format.underline = Stroke::new(1.5, ec);
        }

        if c == '/' && it.peek().map_or(false, |(_, nc)| *nc == '/') { // Comment
            let mut s = format!("{}", c);
            while let Some((_, nc)) = it.peek() {
                if *nc == '\n' { break; }
                let next_c = it.next().unwrap().1;
                s.push(next_c);
            }
            job.append(&s, 0.0, egui::TextFormat { color: Color32::from_rgb(100, 150, 100), ..base_format.clone() });
        } else if c == '"' || c == '`' || c == '\'' { // String
            let quote = c;
            let mut s = format!("{}", c);
            while let Some((_, nc)) = it.peek() {
                let nc_val = *nc;
                let next_c = it.next().unwrap().1;
                s.push(next_c);
                if nc_val == quote { break; }
            }
            job.append(&s, 0.0, egui::TextFormat { color: Color32::from_rgb(200, 150, 100), ..base_format.clone() });
        } else if c.is_alphabetic() || c == '_' { // Word
            let mut s = format!("{}", c);
            while let Some((_, nc)) = it.peek() {
                if nc.is_alphanumeric() || *nc == '_' { 
                    let next_c = it.next().unwrap().1;
                    s.push(next_c); 
                } else { break; }
            }
            let color = if RHAI_KEYWORDS.contains(&s.as_str()) {
                Color32::from_rgb(80, 150, 255) // Keyword
            } else if BROWSER_APIS.contains(&s.as_str()) {
                Color32::from_rgb(255, 180, 100) // API
            } else {
                Color32::from_rgb(200, 200, 200) // Default
            };
            job.append(&s, 0.0, egui::TextFormat { color, ..base_format.clone() });
        } else if c.is_numeric() { // Number
            let mut s = format!("{}", c);
            while let Some((_, nc)) = it.peek() {
                if nc.is_numeric() || *nc == '.' { 
                    let next_c = it.next().unwrap().1;
                    s.push(next_c); 
                } else { break; }
            }
            job.append(&s, 0.0, egui::TextFormat { color: Color32::from_rgb(180, 130, 220), ..base_format.clone() });
        } else { // Symbol/Punctuation
            let color = if "(){}[].,;".contains(c) { Color32::from_gray(140) } else { Color32::from_gray(200) };
            job.append(&c.to_string(), 0.0, egui::TextFormat { color, ..base_format.clone() });
        }
    }
    job
}

/// KOD NOTU: Kod içinde arama ve değiştirme arayüzü.
fn render_search_bar(ui: &mut Ui, state: &mut AppState) {
    if !state.ide_search_open { return; }

    Frame::none()
        .fill(design::BG_SURFACE)
        .stroke(Stroke::new(1.0, design::ACCENT_CYAN))
        .inner_margin(4.0)
        .corner_radius(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🔍 Find:").small().color(design::TEXT_MUTED));
                ui.add(egui::TextEdit::singleline(&mut state.ide_search_text).desired_width(120.0));

                ui.label(RichText::new("Replace:").small().color(design::TEXT_MUTED));
                ui.add(egui::TextEdit::singleline(&mut state.ide_replace_text).desired_width(120.0));

                if ui.button("Next").clicked() {
                    let code = &state.script_package.code;
                    if !state.ide_search_text.is_empty() {
                        let start = state.ide_autocomplete_cursor;
                        if let Some(pos) = code[start..].find(&state.ide_search_text) {
                            state.ide_autocomplete_cursor = start + pos + state.ide_search_text.len();
                        } else if let Some(pos) = code.find(&state.ide_search_text) {
                            state.ide_autocomplete_cursor = pos + state.ide_search_text.len();
                        }
                    }
                }

                if ui.button("Replace").clicked() {
                    if !state.ide_search_text.is_empty() {
                        let code = state.script_package.code.clone();
                        let start = state.ide_autocomplete_cursor.saturating_sub(state.ide_search_text.len());
                        if code.get(start..state.ide_autocomplete_cursor) == Some(&state.ide_search_text) {
                            let mut new_code = code[..start].to_string();
                            new_code.push_str(&state.ide_replace_text);
                            new_code.push_str(&code[state.ide_autocomplete_cursor..]);
                            state.script_package.code = new_code;
                        }
                    }
                }

                if ui.button("All").clicked() {
                    if !state.ide_search_text.is_empty() {
                        state.script_package.code = state.script_package.code.replace(&state.ide_search_text, &state.ide_replace_text);
                    }
                }

                if ui.button("❌").clicked() {
                    state.ide_search_open = false;
                }
            });
        });
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
    ui.horizontal(|ui| {
        ui.label(RichText::new("Code Editor").strong().color(design::ACCENT_ORANGE));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("🔍 Find (Ctrl+F)").clicked() {
                state.ide_search_open = !state.ide_search_open;
            }
        });
    });

    if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F)) {
        state.ide_search_open = true;
    }

    render_search_bar(ui, state);
    ui.add_space(2.0);

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
                        let cursor_pos = state.ide_autocomplete_cursor; 
                        let highlight_braces = find_matching_brace(&state.script_package.code, cursor_pos);
                        let diagnostics = &state.ide_diagnostics;

                        let mut layouter = |ui: &Ui, string: &str, _wrap_width: f32| {
                            let mut job = rhai_highlighter(ui, string, highlight_braces, diagnostics);
                            job.wrap.max_width = f32::INFINITY; 
                            ui.ctx().fonts(|f| f.layout_job(job))
                        };

                        let output = egui::TextEdit::multiline(&mut state.script_package.code)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(avail_w)
                                .desired_rows(20)
                                .lock_focus(true)
                                .layouter(&mut layouter)
                                .frame(false)
                                .show(ui);

                        // Hover Tooltips Logic
                        output.response.on_hover_ui(|ui| {
                            if let Some(hover_pos) = ui.ctx().pointer_hover_pos() {
                                let relative_pos = hover_pos - output.galley_pos;
                                let cursor_at_hover = output.galley.cursor_from_pos(relative_pos);
                                let char_idx = cursor_at_hover.ccursor.index;
                                let code = &state.script_package.code;
                                
                                // 1. Check for Errors at this position
                                let mut hover_text = None;
                                let mut line_starts = vec![0];
                                for (i, c) in code.chars().enumerate() {
                                    if c == '\n' { line_starts.push(i + 1); }
                                }
                                
                                for d in diagnostics {
                                    if let Some(line_idx) = d.line.map(|l| l.saturating_sub(1)) {
                                        if let Some(&start) = line_starts.get(line_idx) {
                                            let end = line_starts.get(line_idx + 1).cloned().unwrap_or(code.len());
                                            if char_idx >= start && char_idx < end {
                                                hover_text = Some(format!("❌ Error: {}", d.message));
                                                break;
                                            }
                                        }
                                    }
                                }
                                
                                // 2. Check for API Docs if no error
                                if hover_text.is_none() {
                                    let start_idx = code[..char_idx.min(code.len())].rfind(|c: char| !c.is_alphanumeric() && c != '_').map(|i| i + 1).unwrap_or(0);
                                    let end_idx = code[char_idx..].find(|c: char| !c.is_alphanumeric() && c != '_').map(|i| i + char_idx).unwrap_or(code.len());
                                    if start_idx < end_idx {
                                        let word = &code[start_idx..end_idx];
                                        if BROWSER_APIS.contains(&word) {
                                            hover_text = Some(format!("📖 API: {}\n{}", word, get_api_doc(word)));
                                        }
                                    }
                                }
                                
                                if let Some(txt) = hover_text {
                                    ui.label(txt);
                                }
                            }
                        });

                        if let Some(cursor_range) = output.cursor_range {
                            let cp = cursor_range.primary.ccursor.index;
                            state.ide_autocomplete_cursor = cp;
                            handle_autocomplete(ui, state, cp);
                            
                            // Auto-Indentation Logic
                            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                let code = state.script_package.code.clone();
                                // Find the start of the line where the cursor was before Enter
                                let mut line_start = cp.saturating_sub(1);
                                while line_start > 0 && code.chars().nth(line_start-1) != Some('\n') {
                                    line_start -= 1;
                                }
                                let prev_line = &code[line_start..cp.saturating_sub(1)];
                                let indent = prev_line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
                                let extra = if prev_line.trim_end().ends_with('{') { "    " } else { "" };
                                let to_insert = format!("{}{}", indent, extra);
                                
                                if !to_insert.is_empty() {
                                    let mut new_code = code.clone();
                                    new_code.insert_str(cp, &to_insert);
                                    state.script_package.code = new_code;
                                }
                            }
                            
                            // Autocomplete Popup
                            if state.ide_autocomplete_open && !state.ide_autocomplete_suggestions.is_empty() {
                                let pos = output.galley_pos + egui::vec2(0.0, 20.0); // Simple positioning
                                egui::Window::new("Suggestions")
                                    .fixed_pos(pos)
                                    .title_bar(false)
                                    .resizable(false)
                                    .frame(Frame::group(ui.style()).fill(design::BG_SURFACE))
                                    .show(ui.ctx(), |ui| {
                                        for (i, sug) in state.ide_autocomplete_suggestions.iter().enumerate() {
                                            let is_selected = i == state.ide_autocomplete_index;
                                            let res = ui.selectable_label(is_selected, sug);
                                            if res.clicked() {
                                                let mut new_code = state.script_package.code.clone();
                                                new_code.insert_str(cp, sug);
                                                state.script_package.code = new_code;
                                                state.ide_autocomplete_open = false;
                                            }
                                        }
                                    });
                            }
                        }

                        // Anlık Hata Kontrolü (Debounced)
                        let now = ui.input(|i| i.time);
                        if now - state.ide_last_check_time > 2.0 {
                            state.ide_last_check_time = now;
                            let package = state.script_package.clone();
                            let tab_id = state.scripting_tab_binding.clone().or(state.selected_tab_id.clone());
                            let port = state.config.remote_debug_port;
                            
                            tokio::spawn(async move {
                                let report = crate::core::scripting::engine::check_script(&package, tab_id, Some(port), false).await;
                                emit(AppEvent::ScriptingCheckResult(report));
                            });
                        }
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
