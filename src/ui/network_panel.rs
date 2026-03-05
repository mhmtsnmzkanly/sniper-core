use crate::state::AppState;
use crate::ui::design;
use egui::{Ui, Color32, RichText, Frame};
use egui_extras::{TableBuilder, Column};

pub fn render(ui: &mut Ui, state: &mut AppState) {
    let tid = match &state.selected_tab_id {
        Some(id) => id.clone(),
        None => { ui.label("No active tab selected."); return; }
    };

    let (requests, mut search, mut type_filter, mut status_filter) = {
        let ws = state.workspaces.get(&tid).unwrap();
        (
            ws.network_requests.clone(),
            ws.network_search.clone(),
            ws.network_type_filter.clone(),
            ws.network_status_filter.clone(),
        )
    };

    ui.vertical(|ui| {
        design::title(ui, "Network Inspector", design::ACCENT_CYAN);
        ui.add_space(6.0);
        Frame::group(ui.style()).fill(design::BG_SURFACE).inner_margin(8.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("FILTER").strong().color(design::ACCENT_ORANGE));
                let types = ["XHR", "JS", "CSS", "IMG", "DOC", "OTHER"];
                ui.menu_button(format!("TYPES ({})", type_filter.len()), |ui| {
                    for t in types {
                        let mut is_selected = type_filter.contains(t);
                        if ui.checkbox(&mut is_selected, t).changed() {
                            if is_selected { type_filter.insert(t.to_string()); }
                            else { type_filter.insert(t.to_string()); type_filter.remove(t); }
                        }
                    }
                    if ui.button("CLEAR ALL").clicked() { type_filter.clear(); }
                });

                ui.separator();
                ui.label("SEARCH:");
                ui.add(egui::TextEdit::singleline(&mut search).desired_width(150.0));

                ui.separator();
                ui.label(RichText::new("QUICK").strong().color(design::ACCENT_ORANGE));
                if ui.button("XHR").clicked() {
                    type_filter.clear();
                    type_filter.insert("XHR".to_string());
                }
                if ui.button("JS").clicked() {
                    type_filter.clear();
                    type_filter.insert("JS".to_string());
                }
                if ui.button("IMG").clicked() {
                    type_filter.clear();
                    type_filter.insert("IMG".to_string());
                }
                if ui.button("Errors").clicked() {
                    status_filter = "error".to_string();
                }
                if ui.button("2xx").clicked() {
                    status_filter = "ok".to_string();
                }
                if ui.button("Reset").clicked() {
                    type_filter.clear();
                    status_filter = "all".to_string();
                    search.clear();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🗑 CLEAR LOGS").clicked() {
                        if let Some(ws) = state.workspaces.get_mut(&tid) { ws.network_requests.clear(); }
                    }
                });
            });
        });

        ui.add_space(5.0);

        let filtered_requests: Vec<_> = requests.into_iter().filter(|r| {
            let s = search.to_lowercase();
            if !s.is_empty() && !r.url.to_lowercase().contains(&s) { return false; }

            match status_filter.as_str() {
                "ok" => {
                    if !matches!(r.status, Some(code) if (200..300).contains(&code)) {
                        return false;
                    }
                }
                "error" => {
                    if !matches!(r.status, Some(code) if code >= 400) {
                        return false;
                    }
                }
                _ => {}
            }
            
            if !type_filter.is_empty() {
                let rt = r.resource_type.to_lowercase();
                let mut match_type = false;
                if type_filter.contains("XHR") && (rt.contains("xhr") || rt.contains("fetch")) { match_type = true; }
                if type_filter.contains("JS") && rt.contains("script") { match_type = true; }
                if type_filter.contains("CSS") && rt.contains("style") { match_type = true; }
                if type_filter.contains("IMG") && rt.contains("image") { match_type = true; }
                if type_filter.contains("DOC") && rt.contains("document") { match_type = true; }
                if type_filter.contains("OTHER") && !match_type { match_type = true; }
                if !match_type { return false; }
            }
            true
        }).collect();

        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::remainder())
            .column(Column::auto())
            .header(20.0, |mut h| {
                h.col(|ui| { ui.strong("METHOD"); });
                h.col(|ui| { ui.strong("STATUS"); });
                h.col(|ui| { ui.strong("URL"); });
                h.col(|ui| { ui.strong("TYPE"); });
            })
            .body(|b| {
                b.rows(20.0, filtered_requests.len(), |mut r| {
                    let req = &filtered_requests[r.index()];
                    r.col(|ui| { ui.label(&req.method); });
                    r.col(|ui| { 
                        let color = match req.status {
                            Some(s) if s >= 200 && s < 300 => design::ACCENT_GREEN,
                            Some(s) if s >= 400 => Color32::from_rgb(255, 120, 120),
                            _ => design::TEXT_MUTED,
                        };
                        ui.label(RichText::new(req.status.map(|s| s.to_string()).unwrap_or("...".into())).color(color));
                    });
                    r.col(|ui| { ui.label(&req.url); });
                    r.col(|ui| { ui.label(&req.resource_type); });
                });
            });
    });

    if let Some(ws) = state.workspaces.get_mut(&tid) {
        ws.network_search = search;
        ws.network_type_filter = type_filter;
        ws.network_status_filter = status_filter;
    }
}
