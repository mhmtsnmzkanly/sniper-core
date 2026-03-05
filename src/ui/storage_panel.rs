use crate::state::AppState;
use crate::ui::design;
use egui::{Ui, Color32, RichText};
use egui_extras::{TableBuilder, Column};
use crate::core::events::AppEvent;
use crate::ui::scrape::emit;
use url::Url;

pub fn render(ui: &mut Ui, state: &mut AppState, tid: &str) {
    if !state.workspaces.contains_key(tid) { return; }
    
    // Get current tab domain for filtering
    let current_tab_url = state.available_tabs.iter()
        .find(|t| t.id == tid)
        .map(|t| t.url.clone())
        .unwrap_or_default();
    
    let target_domain = Url::parse(&current_tab_url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .unwrap_or_default();

    let (mut cookies, title, mut show_modal, mut edit_buffer) = {
        let ws = state.workspaces.get(tid).unwrap();
        (ws.cookies.clone(), ws.title.clone(), ws.show_cookie_modal, ws.cookie_edit_buffer.clone())
    };

    let filtered_cookies: Vec<(usize, crate::state::ChromeCookie)> = cookies.iter().enumerate()
        .filter(|(_, c)| {
            if target_domain.is_empty() { return true; }
            let c_domain = c.domain.trim_start_matches('.');
            target_domain.contains(c_domain) || c_domain.contains(&target_domain)
        })
        .map(|(i, c)| (i, c.clone()))
        .collect();

    design::title(ui, "Storage / Cookie Manager", design::ACCENT_CYAN);
    ui.add_space(6.0);

    egui::Frame::group(ui.style()).fill(design::BG_SURFACE).inner_margin(8.0).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("Workspace: {}", title)).color(design::TEXT_MUTED));
            ui.separator();
            ui.label(RichText::new(format!("DOMAIN: {}", target_domain)).small().color(design::ACCENT_ORANGE));
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🔄 REFRESH").clicked() {
                    emit(AppEvent::RequestCookies(tid.to_string()));
                }
                if ui.button("📤 EXPORT").on_hover_text("Export filtered cookies to JSON").clicked() {
                    let export_data: Vec<crate::state::ChromeCookie> = filtered_cookies.iter().map(|(_, c)| c.clone()).collect();
                    if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).set_file_name("cookies.json").save_file() {
                        if let Ok(json) = serde_json::to_string_pretty(&export_data) {
                            let _ = std::fs::write(path, json);
                        }
                    }
                }
                if ui.button("📥 IMPORT").on_hover_text("Import cookies from JSON").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() {
                        if let Ok(content) = std::fs::read_to_string(path) {
                            if let Ok(imported_cookies) = serde_json::from_str::<Vec<crate::state::ChromeCookie>>(&content) {
                                for cookie in imported_cookies {
                                    emit(AppEvent::RequestCookieAdd(tid.to_string(), cookie));
                                }
                            }
                        }
                    }
                }
            });
        });
    });
    
    ui.add_space(10.0);

    let mut cookie_to_delete = None;
    let mut cookie_to_update = None;

    // RESPONSIVE TABLE
    let _table_h = ui.available_height() - 80.0;
    ui.push_id(format!("{}_cookie_table", tid), |ui| {
        let table = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::auto().at_least(100.0).resizable(true)) // DOMAIN
            .column(Column::auto().at_least(100.0).resizable(true)) // KEY
            .column(Column::remainder().at_least(200.0))           // VALUE
            .column(Column::auto().at_least(40.0));                // DELETE

        table.header(25.0, |mut header| {
            header.col(|ui| { ui.strong("DOMAIN"); });
            header.col(|ui| { ui.strong("KEY"); });
            header.col(|ui| { ui.strong("VALUE"); });
            header.col(|ui| { ui.strong(""); });
        })
        .body(|body| {
            body.rows(30.0, filtered_cookies.len(), |mut row| {
                let row_idx = row.index();
                let (original_idx, _) = &filtered_cookies[row_idx];
                let original_idx = *original_idx;
                
                let cookie = &mut cookies[original_idx];
                
                row.col(|ui| { ui.label(&cookie.domain); });
                row.col(|ui| {
                    if ui.add(egui::TextEdit::singleline(&mut cookie.name).desired_width(f32::INFINITY)).changed() {
                        cookie_to_update = Some(original_idx);
                    }
                });
                row.col(|ui| {
                    if ui.add(egui::TextEdit::singleline(&mut cookie.value).desired_width(f32::INFINITY)).changed() {
                        cookie_to_update = Some(original_idx);
                    }
                });
                row.col(|ui| {
                    if ui.button(RichText::new("🗑").color(Color32::from_rgb(255, 80, 80))).clicked() {
                        cookie_to_delete = Some(original_idx);
                    }
                });
            });
        });
    });

    if let Some(idx) = cookie_to_delete {
        let c = &cookies[idx];
        emit(AppEvent::RequestCookieDelete(tid.to_string(), c.name.clone(), c.domain.clone()));
    }

    if let Some(idx) = cookie_to_update {
        emit(AppEvent::RequestCookieAdd(tid.to_string(), cookies[idx].clone()));
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(8.0);

    // ADD NEW COOKIE BUTTON AT BOTTOM
    ui.horizontal(|ui| {
        if ui.add(egui::Button::new(RichText::new("➕ ADD NEW COOKIE").strong())
            .min_size([200.0, 35.0].into())
            .fill(Color32::from_rgb(0, 100, 180))).clicked() {
            edit_buffer = crate::state::ChromeCookie {
                domain: if target_domain.is_empty() { "example.com".into() } else { target_domain.clone() },
                path: "/".to_string(),
                ..Default::default()
            };
            show_modal = true;
        }
    });

    // MODAL FOR NEW COOKIE
    if show_modal {
        let mut open = true;
        egui::Window::new(format!("Add Cookie - {}", title))
            .open(&mut open)
            .id(egui::Id::new(format!("{}_new_cookie_win", tid)))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .resizable(false)
            .collapsible(false)
            .show(ui.ctx(), |ui| {
                ui.set_width(400.0);
                egui::Grid::new(format!("{}_new_cookie_grid", tid)).num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
                    ui.label("Domain:"); ui.text_edit_singleline(&mut edit_buffer.domain); ui.end_row();
                    ui.label("Key (Name):"); ui.text_edit_singleline(&mut edit_buffer.name); ui.end_row();
                    ui.label("Value:"); ui.add(egui::TextEdit::multiline(&mut edit_buffer.value).desired_rows(3).desired_width(280.0)); ui.end_row();
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("CREATE").strong()).clicked() {
                        emit(AppEvent::RequestCookieAdd(tid.to_string(), edit_buffer.clone()));
                        show_modal = false;
                    }
                    if ui.button("CANCEL").clicked() { show_modal = false; }
                });
            });
        if !open { show_modal = false; }
    }

    // Sync back state
    if let Some(ws) = state.workspaces.get_mut(tid) {
        ws.show_cookie_modal = show_modal;
        ws.cookie_edit_buffer = edit_buffer;
        ws.cookies = cookies;
    }
}
