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
    
    // We need to be careful with borrowing here.
    // Let's extract what we need first or use a scoped borrow.
    
    let (mut cookies, title, mut show_modal, mut edit_buffer) = {
        let ws = state.workspaces.get(&tid).unwrap();
        (ws.cookies.clone(), ws.title.clone(), ws.show_cookie_modal, ws.cookie_edit_buffer.clone())
    };

    ui.horizontal(|ui| {
        ui.heading(RichText::new(format!("COOKIE MANAGER: {}", title)).color(Color32::KHAKI));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("🔄 REFRESH").clicked() {
                emit(AppEvent::RequestCookies(tid.clone()));
            }
        });
    });
    ui.add_space(10.0);

    egui::ScrollArea::vertical().max_height(ui.available_height() - 60.0).show(ui, |ui| {
        egui::Grid::new("cookie_manager_grid")
            .striped(true)
            .num_columns(4)
            .spacing([10.0, 8.0])
            .min_col_width(80.0)
            .show(ui, |ui| {
                ui.label(RichText::new("DOMAIN").strong().color(Color32::GRAY));
                ui.label(RichText::new("KEY").strong().color(Color32::GRAY));
                ui.label(RichText::new("VALUE").strong().color(Color32::GRAY));
                ui.label(RichText::new("").strong());
                ui.end_row();

                let mut cookie_to_delete = None;
                let mut cookie_to_update = None;

                for (idx, cookie) in cookies.iter_mut().enumerate() {
                    // DOMAIN (Read-only usually, but let's keep it simple)
                    ui.label(RichText::new(&cookie.domain).small());

                    // KEY (NAME) - Editable
                    if ui.add(egui::TextEdit::singleline(&mut cookie.name).desired_width(120.0)).changed() {
                        cookie_to_update = Some(idx);
                    }

                    // VALUE - Editable
                    ui.horizontal(|ui| {
                        if ui.add(egui::TextEdit::singleline(&mut cookie.value).desired_width(300.0)).changed() {
                            cookie_to_update = Some(idx);
                        }
                        if ui.button(RichText::new("🗑").color(Color32::RED)).on_hover_text("Delete Cookie").clicked() {
                            cookie_to_delete = Some(idx);
                        }
                    });
                    
                    ui.end_row();
                }

                if let Some(idx) = cookie_to_delete {
                    let c = &cookies[idx];
                    emit(AppEvent::RequestCookieDelete(tid.clone(), c.name.clone(), c.domain.clone()));
                }

                if let Some(idx) = cookie_to_update {
                    // In a real app we might want a "Save" button to avoid spamming CDP, 
                    // but user asked for "Key and Value press to change". 
                    // We'll send update on every change for now as requested.
                    emit(AppEvent::RequestCookieAdd(tid.clone(), cookies[idx].clone()));
                }
            });
    });

    ui.add_space(15.0);
    ui.separator();
    ui.add_space(10.0);

    // ADD NEW COOKIE BUTTON AT BOTTOM
    ui.horizontal(|ui| {
        if ui.add(egui::Button::new(RichText::new("➕ ADD NEW COOKIE").strong())
            .min_size([200.0, 35.0].into())
            .fill(Color32::from_rgb(0, 120, 215))).clicked() {
            edit_buffer = crate::state::ChromeCookie {
                domain: "example.com".to_string(),
                path: "/".to_string(),
                ..Default::default()
            };
            show_modal = true;
        }
    });

    // MODAL FOR NEW COOKIE
    if show_modal {
        let mut open = true;
        egui::Window::new("ADD NEW COOKIE")
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .resizable(false)
            .collapsible(false)
            .show(ui.ctx(), |ui| {
                ui.set_width(400.0);
                egui::Grid::new("new_cookie_grid").num_columns(2).spacing([10.0, 10.0]).show(ui, |ui| {
                    ui.label("Domain:"); ui.text_edit_singleline(&mut edit_buffer.domain); ui.end_row();
                    ui.label("Key (Name):"); ui.text_edit_singleline(&mut edit_buffer.name); ui.end_row();
                    ui.label("Value:"); ui.add(egui::TextEdit::multiline(&mut edit_buffer.value).desired_rows(3).desired_width(280.0)); ui.end_row();
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("CREATE").strong()).clicked() {
                        emit(AppEvent::RequestCookieAdd(tid.clone(), edit_buffer.clone()));
                        show_modal = false;
                    }
                    if ui.button("CANCEL").clicked() { show_modal = false; }
                });
            });
        if !open { show_modal = false; }
    }

    // Sync back state
    if let Some(ws) = state.workspaces.get_mut(&tid) {
        ws.show_cookie_modal = show_modal;
        ws.cookie_edit_buffer = edit_buffer;
    }
}
