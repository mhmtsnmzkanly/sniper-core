use crate::state::AppState;
use crate::ui::design;
use egui::{Ui, Color32, RichText, Frame};

pub fn render(ui: &mut Ui, state: &mut AppState, tid: &str) {
    let (requests, mut search, mut type_filter, mut status_filter, blocked_urls) = {
        let ws = state.workspaces.get(tid).unwrap();
        (
            ws.network_requests.clone(),
            ws.network_search.clone(),
            ws.network_type_filter.clone(),
            ws.network_status_filter.clone(),
            ws.blocked_urls.clone(),
        )
    };

    ui.vertical(|ui| {
        design::title(ui, "Network Inspector", design::ACCENT_CYAN);
        ui.add_space(6.0);

        // --- FILTER BAR ---
        Frame::group(ui.style()).fill(design::BG_SURFACE).inner_margin(8.0).show(ui, |ui| {
            // KOD NOTU: horizontal_wrapped() dar ekranlarda kontrollerin alt satıra geçmesini sağlar.
            ui.horizontal_wrapped(|ui| {
                // Type filter
                ui.label(RichText::new("FILTER:").strong().color(design::ACCENT_ORANGE));
                let types = ["XHR", "JS", "CSS", "IMG", "DOC", "OTHER"];
                ui.menu_button(format!("TYPES ({})", type_filter.len()), |ui| {
                    ui.set_width(120.0);
                    for t in types {
                        let mut is_selected = type_filter.contains(t);
                        if ui.checkbox(&mut is_selected, t).changed() {
                            if is_selected { type_filter.insert(t.to_string()); }
                            else { type_filter.remove(t); }
                        }
                    }
                    if ui.button("CLEAR ALL").clicked() { type_filter.clear(); }
                });

                ui.separator();
                ui.label("STATUS:");
                for (label, val, color) in [
                    ("ALL", "all", design::TEXT_MUTED),
                    ("2xx", "ok", design::ACCENT_GREEN),
                    ("4xx/5xx", "error", Color32::from_rgb(255, 120, 120)),
                ] {
                    let selected = status_filter == val;
                    if ui.selectable_label(selected, RichText::new(label).color(color)).clicked() {
                        status_filter = val.to_string();
                    }
                }

                ui.separator();
                ui.label("SEARCH:");
                ui.add(egui::TextEdit::singleline(&mut search).desired_width(180.0).hint_text("url contains..."));
                
                ui.horizontal(|ui| {
                    if ui.button("Block").clicked() && !search.trim().is_empty() {
                        crate::ui::scrape::emit(crate::core::events::AppEvent::RequestUrlBlock(tid.to_string(), search.trim().to_string()));
                    }
                    if ui.button("Unblock").clicked() && !search.trim().is_empty() {
                        crate::ui::scrape::emit(crate::core::events::AppEvent::RequestUrlUnblock(tid.to_string(), search.trim().to_string()));
                    }
                });

                ui.separator();
                // Actions (Right aligned in wrapped layout usually stays at far right or next row)
                if ui.button("🗑 CLEAR").clicked() {
                    if let Some(ws) = state.workspaces.get_mut(tid) { ws.network_requests.clear(); }
                }

                ui.menu_button(RichText::new(format!("🛡 BLOCKED ({})", blocked_urls.len())).color(design::ACCENT_ORANGE), |ui| {
                    ui.set_width(260.0);
                    if blocked_urls.is_empty() {
                        ui.label("No active blocks.");
                    } else {
                        for url in blocked_urls.clone().into_iter() {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&url).small());
                                if ui.button("x").clicked() {
                                    crate::ui::scrape::emit(crate::core::events::AppEvent::RequestUrlUnblock(tid.to_string(), url));
                                }
                            });
                        }
                    }
                });
                
                ui.label(RichText::new(format!("TOTAL: {}", requests.len())).color(design::ACCENT_GREEN).monospace());
            });
        });

        ui.add_space(5.0);

        // --- REQUEST LIST ---
        let filtered_requests: Vec<crate::state::NetworkRequest> = requests.into_iter().filter(|r| {
            let s = search.to_lowercase();
            if !s.is_empty() && !r.url.to_lowercase().contains(&s) { return false; }
            match status_filter.as_str() {
                "ok" => if !matches!(r.status, Some(code) if (200..300).contains(&code)) { return false; },
                "error" => if !matches!(r.status, Some(code) if code >= 400) { return false; },
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

        let list_height = ui.available_height();
        egui::ScrollArea::vertical()
            .max_height(list_height)
            .id_salt(format!("{}_net_list", tid))
            .auto_shrink([false, false])
            .show(ui, |ui| {
            for req in filtered_requests {
                Frame::group(ui.style())
                    .stroke(egui::Stroke::new(1.0, Color32::from_gray(55)))
                    .fill(design::BG_SURFACE)
                    .inner_margin(8.0)
                    .corner_radius(6.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let method_col = match req.method.as_str() {
                                "GET" => Color32::from_rgb(90, 180, 255),
                                "POST" => design::ACCENT_ORANGE,
                                "PUT" => Color32::from_rgb(180, 150, 255),
                                "DELETE" => Color32::from_rgb(255, 120, 140),
                                _ => design::TEXT_MUTED,
                            };
                            ui.label(RichText::new(req.method.clone()).color(method_col).monospace().strong());

                            let status_txt = req.status.map(|s| s.to_string()).unwrap_or_else(|| "...".into());
                            let status_col = match req.status {
                                Some(s) if (200..300).contains(&s) => design::ACCENT_GREEN,
                                Some(s) if s >= 400 => Color32::from_rgb(255, 120, 120),
                                _ => Color32::from_gray(140),
                            };
                            ui.label(RichText::new(status_txt).color(status_col).monospace());
                            ui.label(RichText::new(req.resource_type.clone()).color(design::TEXT_MUTED).size(10.0));
                            
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(format!("ID: {}", req.request_id)).size(9.0).color(Color32::from_gray(80)));
                            });
                        });
                        ui.add(egui::Label::new(RichText::new(&req.url).monospace().size(11.0).color(Color32::from_gray(210))).truncate());
                    });
                ui.add_space(4.0);
            }
        });
    });

    if let Some(ws) = state.workspaces.get_mut(tid) {
        ws.network_search = search;
        ws.network_type_filter = type_filter;
        ws.network_status_filter = status_filter;
    }
}
