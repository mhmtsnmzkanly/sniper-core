use crate::state::AppState;
use egui::{Ui, Color32, RichText};
use crate::core::events::AppEvent;
use crate::ui::scrape::emit;

pub fn render(ui: &mut Ui, state: &mut AppState) {
    let tid = match &state.selected_tab_id {
        Some(id) => id.clone(),
        None => {
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);
                ui.label(RichText::new("⚠ SELECT A TAB IN THE SCRAPE PANEL TO VIEW STORAGE").strong().color(Color32::YELLOW));
            });
            return;
        }
    };

    if !state.workspaces.contains_key(&tid) { return; }
    let ws = state.workspaces.get_mut(&tid).unwrap();

    ui.heading(format!("STORAGE & SESSION MANAGER: {}", ws.title));
    ui.add_space(5.0);

    ui.group(|ui| {
        ui.horizontal(|ui| {
            if ui.button("➕ Add Cookie").clicked() {
                ws.cookie_edit_buffer = crate::state::ChromeCookie::default();
                ws.show_cookie_modal = true;
                tracing::info!("[STORAGE <-> COOKIES] Manual creation started.");
            }
            if ui.button("🔄 Refresh All").clicked() {
                tracing::info!("[STORAGE <-> COMMAND] Refreshing cookies for tab: {}", ws.title);
                emit(AppEvent::RequestCookies(tid.clone())); 
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

                let cookies = ws.cookies.clone();
                for cookie in cookies {
                    ui.label(RichText::new(&cookie.domain).small());
                    ui.label(RichText::new(&cookie.name).color(Color32::LIGHT_BLUE).strong());
                    
                    let val_width = (ui.available_width() * 0.4).max(0.0);
                    ui.allocate_ui(egui::vec2(val_width, 30.0), |ui| {
                        ui.add(egui::Label::new(RichText::new(&cookie.value).small()).truncate());
                    });
                    
                    ui.horizontal(|ui| {
                        if ui.button("✏").on_hover_text("Edit").clicked() {
                            ws.cookie_edit_buffer = cookie.clone();
                            ws.show_cookie_modal = true;
                        }
                        if ui.button("❌").on_hover_text("Delete").clicked() {
                            tracing::warn!("[STORAGE <-> COOKIES] Deleting: {}", cookie.name);
                            emit(AppEvent::RequestCookieDelete(tid.clone(), cookie.name.clone(), cookie.domain.clone()));
                        }
                    });
                    ui.end_row();
                }
            });
    });

    // IMPROVED COOKIE MODAL
    if ws.show_cookie_modal {
        let mut open = true;
        egui::Window::new(RichText::new("Cookie Editor").strong())
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.set_width(400.0);
                ui.vertical(|ui| {
                    egui::Grid::new("edit_grid_v3").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
                        ui.label("Domain:"); ui.text_edit_singleline(&mut ws.cookie_edit_buffer.domain); ui.end_row();
                        ui.label("Name:"); ui.text_edit_singleline(&mut ws.cookie_edit_buffer.name); ui.end_row();
                        ui.label("Value:"); ui.add(egui::TextEdit::multiline(&mut ws.cookie_edit_buffer.value).desired_rows(5).desired_width(280.0)); ui.end_row();
                        ui.label("Flags:");
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut ws.cookie_edit_buffer.secure, "Secure");
                            ui.checkbox(&mut ws.cookie_edit_buffer.http_only, "HttpOnly");
                        });
                        ui.end_row();
                    });
                    
                    ui.add_space(15.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("💾 APPLY CHANGES").strong().size(16.0)).clicked() {
                            tracing::info!("[STORAGE <-> COOKIES] Applying changes for: {}", ws.cookie_edit_buffer.name);
                            emit(AppEvent::RequestCookieAdd(tid.clone(), ws.cookie_edit_buffer.clone()));
                            ws.show_cookie_modal = false;
                        }
                        if ui.button("Close").clicked() { ws.show_cookie_modal = false; }
                    });
                });
            });
        if !open { ws.show_cookie_modal = false; }
    }
}
