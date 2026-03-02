use crate::state::AppState;
use egui::{Ui, Color32, RichText};

pub fn render(ui: &mut Ui, state: &mut AppState) {
    let tid = match &state.selected_tab_id {
        Some(id) => id.clone(),
        None => { ui.label("No tab selected."); return; }
    };
    
    if !state.workspaces.contains_key(&tid) { return; }
    let ws = state.workspaces.get_mut(&tid).unwrap();

    ui.heading(format!("NETWORK INSPECTOR: {}", ws.title));
    ui.add_space(5.0);

    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label("🔍 SEARCH:");
            ui.text_edit_singleline(&mut ws.network_search);
            if ui.button("🗑 CLEAR").clicked() { ws.network_requests.clear(); }
            
            ui.separator();
            
            if ui.button(RichText::new("📂 SAVE LOGS").strong()).clicked() {
                if let Some(first_req) = ws.network_requests.first() {
                    let root = state.config.output_dir.clone();
                    if let Ok(dir) = crate::core::browser::BrowserManager::get_output_path(root, "NETWORK", &first_req.url) {
                        let log_path = dir.join("traffic.json");
                        let json = serde_json::to_string_pretty(&ws.network_requests).unwrap_or_default();
                        let _ = std::fs::write(log_path, json);
                        tracing::info!("[NETWORK <-> IO] Logs exported to: {:?}", dir);
                    }
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("Captured: {}", ws.network_requests.len()));
            });
        });
    });

    ui.add_space(5.0);

    egui::Frame::default().fill(Color32::from_black_alpha(30)).show(ui, |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("net_grid_v4").num_columns(5).spacing([10.0, 8.0]).striped(true).show(ui, |ui| {
                ui.label(RichText::new("M").strong()); 
                ui.label(RichText::new("S").strong()); 
                ui.label(RichText::new("TYPE").strong());
                ui.label(RichText::new("URL").strong());
                ui.label(RichText::new("INSPECT").strong());
                ui.end_row();

                let search = ws.network_search.to_lowercase();
                for req in ws.network_requests.iter().rev() {
                    if !search.is_empty() && !req.url.to_lowercase().contains(&search) { continue; }

                    ui.label(RichText::new(&req.method).monospace().small().color(Color32::KHAKI));
                    
                    let status_color = match req.status {
                        Some(s) if s >= 400 => Color32::RED,
                        Some(s) if s >= 300 => Color32::YELLOW,
                        Some(_) => Color32::GREEN,
                        None => Color32::GRAY,
                    };
                    ui.label(RichText::new(req.status.map(|s| s.to_string()).unwrap_or("...".into())).color(status_color).strong());
                    ui.label(RichText::new(&req.resource_type).size(9.0).color(Color32::LIGHT_GRAY));
                    
                    ui.horizontal(|ui| {
                        let trunc_url: String = req.url.chars().take(70).collect();
                        ui.label(RichText::new(format!("{}...", trunc_url)).small());
                        if ui.button("📋").clicked() { ui.ctx().copy_text(req.url.clone()); }
                    });

                    ui.horizontal(|ui| {
                        ui.button("REQ").on_hover_ui(|ui| {
                            ui.label(RichText::new(req.request_body.as_deref().unwrap_or("No payload")).monospace().small());
                        });
                        ui.button("RES").on_hover_ui(|ui| {
                            ui.label(RichText::new(req.response_body.as_deref().unwrap_or("Empty body")).monospace().small().color(Color32::GREEN));
                        });
                    });
                    ui.end_row();
                }
            });
        });
    });
}
