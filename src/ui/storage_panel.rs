use crate::state::AppState;
use egui::{Ui, Color32, RichText};
use crate::core::events::AppEvent;
use crate::ui::scrape::emit;

pub fn render(ui: &mut Ui, state: &mut AppState) {
    ui.heading("DEVTOOLS MINI STUDIO (Storage & Emulation)");
    ui.add_space(10.0);

    // 1. Emulation Section
    ui.group(|ui| {
        ui.label(RichText::new("Cihaz ve Kimlik Taklidi (Emulation)").strong());
        ui.add_space(5.0);
        
        ui.horizontal(|ui| {
            ui.label("User-Agent:");
            ui.text_edit_singleline(&mut state.user_agent_override);
        });

        ui.horizontal(|ui| {
            ui.label("Latitude:");
            ui.add(egui::DragValue::new(&mut state.latitude).speed(0.01));
            ui.add_space(10.0);
            ui.label("Longitude:");
            ui.add(egui::DragValue::new(&mut state.longitude).speed(0.01));
        });

        if ui.button("Apply Emulation Settings").clicked() {
            if let Some(tab_id) = state.selected_tab_id.clone() {
                emit(AppEvent::RequestEmulation(tab_id, state.user_agent_override.clone(), state.latitude, state.longitude));
            }
        }
    });

    ui.add_space(15.0);

    // 2. Cookies Section
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Oturum Çerezleri (Cookies)").strong());
            if ui.button("🔄 Fetch Cookies").clicked() {
                if let Some(tab_id) = state.selected_tab_id.clone() {
                    emit(AppEvent::RequestCookies(tab_id));
                }
            }
        });

        ui.add_space(5.0);

        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
            if state.cookies.is_empty() {
                ui.label(RichText::new("No cookies loaded. Click Fetch.").italics().color(Color32::GRAY));
            } else {
                egui::Grid::new("cookie_grid").striped(true).num_columns(3).show(ui, |ui| {
                    ui.label(RichText::new("NAME").strong());
                    ui.label(RichText::new("DOMAIN").strong());
                    ui.label(RichText::new("VALUE").strong());
                    ui.end_row();

                    for cookie in &state.cookies {
                        ui.label(RichText::new(&cookie.name).color(Color32::LIGHT_BLUE));
                        ui.label(&cookie.domain);
                        ui.add(egui::Label::new(RichText::new(&cookie.value).small()).truncate());
                        ui.end_row();
                    }
                });
            }
        });
    });
}
