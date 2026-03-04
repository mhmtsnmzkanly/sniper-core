use crate::state::AppState;
use egui::{Ui, Color32, RichText, Frame};
use egui_extras::{TableBuilder, Column};

pub fn render(ui: &mut Ui, state: &mut AppState) {
    let tid = match &state.selected_tab_id {
        Some(id) => id.clone(),
        None => { ui.label("No active tab selected."); return; }
    };

    // Filter Logic
    let (requests, mut search, mut type_filter) = {
        let ws = state.workspaces.get(&tid).unwrap();
        (ws.network_requests.clone(), ws.network_search.clone(), ws.network_type_filter.clone())
    };

    ui.vertical(|ui| {
        Frame::group(ui.style()).fill(Color32::from_gray(25)).inner_margin(8.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("FILTER:").strong().color(Color32::LIGHT_BLUE));
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
                            Some(s) if s >= 200 && s < 300 => Color32::GREEN,
                            Some(s) if s >= 400 => Color32::RED,
                            _ => Color32::GRAY,
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
    }
}
