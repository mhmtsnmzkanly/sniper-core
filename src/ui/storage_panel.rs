use crate::state::{AppState, ChromeCookie};
use egui::{Ui, Color32, RichText};
use crate::core::events::AppEvent;
use crate::ui::scrape::emit;

pub fn render(ui: &mut Ui, state: &mut AppState) {
    ui.heading("STORAGE & SESSION MANAGER");
    ui.add_space(5.0);

    ui.group(|ui| {
        ui.horizontal(|ui| {
            if ui.button("➕ Add Cookie").clicked() {
                state.cookie_edit_buffer = ChromeCookie::default();
                state.show_cookie_modal = true;
                tracing::info!("STORAGE <-> Cookie creation modal opened.");
            }
            if ui.button("🔄 Refresh All").clicked() {
                if let Some(tid) = state.selected_tab_id.clone() { 
                    tracing::info!("STORAGE <-> Refreshing cookies...");
                    emit(AppEvent::RequestCookies(tid)); 
                }
            }
        });
    });

    ui.add_space(10.0);

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("cookie_grid_v3")
            .striped(true)
            .num_columns(4)
            .spacing([10.0, 12.0])
            .min_col_width(ui.available_width() * 0.2)
            .show(ui, |ui| {
                ui.label(RichText::new("DOMAIN").strong());
                ui.label(RichText::new("NAME").strong());
                ui.label(RichText::new("VALUE").strong());
                ui.label(RichText::new("ACTIONS").strong());
                ui.end_row();

                let cookies = state.cookies.clone();
                for cookie in cookies {
                    ui.label(RichText::new(&cookie.domain).small());
                    ui.label(RichText::new(&cookie.name).color(Color32::LIGHT_BLUE).strong());
                    
                    // Larger Value Display
                    ui.allocate_ui(egui::vec2(ui.available_width() * 0.4, 30.0), |ui| {
                        ui.add(egui::Label::new(RichText::new(&cookie.value).small()).truncate());
                    });
                    
                    ui.horizontal(|ui| {
                        if ui.button("✏").on_hover_text("Edit Cookie").clicked() {
                            state.cookie_edit_buffer = cookie.clone();
                            state.show_cookie_modal = true;
                            tracing::info!("STORAGE <-> Editing cookie: {}", cookie.name);
                        }
                        if ui.button("❌").on_hover_text("Delete Cookie").clicked() {
                            if let Some(tid) = state.selected_tab_id.clone() {
                                tracing::warn!("STORAGE <-> Deleting cookie: {}", cookie.name);
                                emit(AppEvent::RequestCookieDelete(tid, cookie.name.clone(), cookie.domain.clone()));
                            }
                        }
                    });
                    ui.end_row();
                }
            });
    });

    // Cookie Editor Modal
    if state.show_cookie_modal {
        let mut open = true;
        egui::Window::new("Edit Cookie")
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.vertical(|ui| {
                    egui::Grid::new("edit_grid_v2").show(ui, |ui| {
                        ui.label("Domain:"); ui.text_edit_singleline(&mut state.cookie_edit_buffer.domain); ui.end_row();
                        ui.label("Name:"); ui.text_edit_singleline(&mut state.cookie_edit_buffer.name); ui.end_row();
                        ui.label("Value:"); ui.add(egui::TextEdit::multiline(&mut state.cookie_edit_buffer.value).desired_rows(4).desired_width(300.0)); ui.end_row();
                        ui.checkbox(&mut state.cookie_edit_buffer.secure, "Secure");
                        ui.checkbox(&mut state.cookie_edit_buffer.http_only, "HttpOnly");
                        ui.end_row();
                    });
                    
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("💾 SAVE").strong()).clicked() {
                            if let Some(tid) = state.selected_tab_id.clone() {
                                tracing::info!("STORAGE <-> Saving cookie: {}", state.cookie_edit_buffer.name);
                                emit(AppEvent::RequestCookieAdd(tid, state.cookie_edit_buffer.clone()));
                                state.show_cookie_modal = false;
                            }
                        }
                        if ui.button("Cancel").clicked() { state.show_cookie_modal = false; }
                    });
                });
            });
        if !open { state.show_cookie_modal = false; }
    }
}
