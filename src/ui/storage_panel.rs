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
            }
            if ui.button("🔄 Refresh All").clicked() {
                if let Some(tid) = state.selected_tab_id.clone() { emit(AppEvent::RequestCookies(tid)); }
            }
        });
    });

    ui.add_space(10.0);

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("cookie_grid_v2").striped(true).num_columns(4).spacing([10.0, 8.0]).show(ui, |ui| {
            ui.label(RichText::new("DOMAIN").strong());
            ui.label(RichText::new("NAME").strong());
            ui.label(RichText::new("VALUE").strong());
            ui.label(RichText::new("ACTIONS").strong());
            ui.end_row();

            let cookies = state.cookies.clone();
            for cookie in cookies {
                ui.label(RichText::new(&cookie.domain).small());
                ui.label(RichText::new(&cookie.name).color(Color32::LIGHT_BLUE).strong());
                ui.add(egui::Label::new(RichText::new(&cookie.value).small()).truncate());
                
                ui.horizontal(|ui| {
                    if ui.button("✏").on_hover_text("Edit").clicked() {
                        state.cookie_edit_buffer = cookie.clone();
                        state.show_cookie_modal = true;
                    }
                    if ui.button("❌").on_hover_text("Delete").clicked() {
                        if let Some(tid) = state.selected_tab_id.clone() {
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
        egui::Window::new("Cookie Editor")
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                egui::Grid::new("edit_grid").show(ui, |ui| {
                    ui.label("Domain:"); ui.text_edit_singleline(&mut state.cookie_edit_buffer.domain); ui.end_row();
                    ui.label("Name:"); ui.text_edit_singleline(&mut state.cookie_edit_buffer.name); ui.end_row();
                    ui.label("Value:"); ui.text_edit_singleline(&mut state.cookie_edit_buffer.value); ui.end_row();
                });
                ui.add_space(10.0);
                if ui.button("💾 SAVE COOKIE").clicked() {
                    if let Some(tid) = state.selected_tab_id.clone() {
                        emit(AppEvent::RequestCookieAdd(tid, state.cookie_edit_buffer.clone()));
                        state.show_cookie_modal = false;
                    }
                }
            });
        if !open { state.show_cookie_modal = false; }
    }
}
