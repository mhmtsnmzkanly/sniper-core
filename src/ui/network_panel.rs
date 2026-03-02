use crate::state::AppState;
use egui::{Ui, Color32, RichText};
use crate::core::events::AppEvent;
use crate::ui::scrape::emit;

pub fn render(ui: &mut Ui, state: &mut AppState) {
    ui.heading("NETWORK INSPECTOR");
    ui.add_space(10.0);

    ui.group(|ui| {
        ui.horizontal(|ui| {
            let label = if state.network_recording { "STOP RECORDING" } else { "START RECORDING" };
            let color = if state.network_recording { Color32::RED } else { Color32::GREEN };
            
            if ui.button(RichText::new(label).color(color).strong()).clicked() {
                if let Some(tab_id) = state.selected_tab_id.clone() {
                    state.network_recording = !state.network_recording;
                    if state.network_recording {
                        emit(AppEvent::RequestNetworkToggle(tab_id, true));
                    }
                } else {
                    tracing::warn!("Select a tab first!");
                }
            }

            if ui.button("CLEAR LOGS").clicked() {
                state.network_requests.clear();
            }

            ui.add_space(20.0);
            ui.label(format!("Captured: {} requests", state.network_requests.len()));
        });
    });

    ui.add_space(10.0);

    // Request Table
    egui::Frame::none().fill(Color32::from_black_alpha(20)).show(ui, |ui| {
        let column_width = (ui.available_width() - 40.0) / 4.0;
        
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("network_grid")
                .num_columns(4)
                .min_col_width(column_width)
                .spacing([10.0, 8.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label(RichText::new("METHOD").strong());
                    ui.label(RichText::new("STATUS").strong());
                    ui.label(RichText::new("TYPE").strong());
                    ui.label(RichText::new("URL").strong());
                    ui.end_row();

                    // Son gelenler en üstte
                    for req in state.network_requests.iter().rev().take(100) {
                        ui.label(RichText::new(&req.method).monospace());
                        
                        let status_text = match req.status {
                            Some(s) => {
                                let color = if s >= 400 { Color32::RED } else if s >= 300 { Color32::YELLOW } else { Color32::GREEN };
                                RichText::new(s.to_string()).color(color).strong()
                            }
                            None => RichText::new("PENDING").color(Color32::GRAY).italics(),
                        };
                        ui.label(status_text);
                        
                        ui.label(RichText::new(&req.resource_type).small());
                        
                        let trunc_url = if req.url.len() > 80 { format!("{}...", &req.url[..80]) } else { req.url.clone() };
                        ui.label(RichText::new(trunc_url).small().color(Color32::GRAY));
                        
                        ui.end_row();
                    }
                });
        });
    });
}
