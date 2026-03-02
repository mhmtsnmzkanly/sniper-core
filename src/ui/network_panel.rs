use crate::state::AppState;
use egui::{Ui, Color32, RichText};

pub fn render(ui: &mut Ui, state: &mut AppState) {
    ui.heading("NETWORK INSPECTOR");
    ui.add_space(5.0);

    ui.group(|ui| {
        ui.horizontal(|ui| {
            if ui.button(if state.network_recording { "STOP" } else { "START" }).clicked() {
                if let Some(tid) = state.selected_tab_id.clone() {
                    state.network_recording = !state.network_recording;
                    crate::ui::scrape::emit(crate::core::events::AppEvent::RequestNetworkToggle(tid, state.network_recording));
                }
            }
            if ui.button("CLEAR").clicked() { state.network_requests.clear(); }
            ui.label(format!("Count: {}", state.network_requests.len()));
        });
    });

    ui.add_space(5.0);

    egui::Frame::default().fill(Color32::from_black_alpha(30)).show(ui, |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("net_grid_v2").num_columns(5).spacing([8.0, 6.0]).striped(true).show(ui, |ui| {
                ui.label(RichText::new("M").strong()); // Method
                ui.label(RichText::new("S").strong()); // Status
                ui.label(RichText::new("T").strong()); // Type
                ui.label(RichText::new("URL").strong());
                ui.label(RichText::new("DATA").strong());
                ui.end_row();

                for req in state.network_requests.iter().rev().take(100) {
                    ui.label(RichText::new(&req.method).monospace().small());
                    
                    let status_color = match req.status {
                        Some(s) if s >= 400 => Color32::RED,
                        Some(s) if s >= 300 => Color32::YELLOW,
                        Some(_) => Color32::GREEN,
                        None => Color32::GRAY,
                    };
                    ui.label(RichText::new(req.status.map(|s| s.to_string()).unwrap_or("...".into())).color(status_color).strong());
                    
                    ui.label(RichText::new(&req.resource_type).size(9.0).color(Color32::LIGHT_GRAY));
                    
                    ui.horizontal(|ui| {
                        let trunc_url: String = req.url.chars().take(60).collect();
                        ui.label(RichText::new(trunc_url).small());
                        if ui.button("📋").on_hover_text("Copy URL").clicked() {
                            ui.ctx().copy_text(req.url.clone());
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.button("REQ").on_hover_ui(|ui| {
                            ui.label(RichText::new(req.request_body.as_deref().unwrap_or("No data")).monospace());
                        });
                        ui.button("RES").on_hover_ui(|ui| {
                            ui.label(RichText::new(req.response_body.as_deref().unwrap_or("No data")).monospace());
                        });
                    });
                    ui.end_row();
                }
            });
        });
    });
}
