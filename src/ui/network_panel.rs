use crate::state::AppState;
use egui::{Ui, Color32, RichText, Frame, Stroke};
use egui_extras::{TableBuilder, Column};
use crate::core::events::AppEvent;

pub fn render(ui: &mut Ui, state: &mut AppState) {
    let tid = match &state.selected_tab_id {
        Some(id) => id.clone(),
        None => { ui.label(RichText::new("NO TARGET SELECTED").strong().color(Color32::RED)); return; }
    };

    let current_title = state.available_tabs.iter().find(|t| t.id == tid).map(|t| t.title.clone()).unwrap_or_else(|| "Tab".into());

    if let Some(ws) = state.workspaces.get_mut(&tid) {
        ui.heading(RichText::new(format!("TRAFFIC MONITOR // {}", current_title.to_uppercase())).strong());
        ui.add_space(5.0);

        let frame_style = Frame::group(ui.style()).fill(Color32::from_gray(25)).stroke(Stroke::new(1.0, Color32::from_gray(50)));

        frame_style.show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(RichText::new("CLEAR LOGS").strong().color(Color32::LIGHT_RED))).clicked() {
                    ws.network_requests.clear();
                }
                
                ui.separator();
                ui.label(RichText::new("FILTER:").strong().color(Color32::LIGHT_BLUE));
                ui.add(egui::TextEdit::singleline(&mut ws.network_search).desired_width(200.0));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("CAPTURED: {}", ws.network_requests.len())).monospace().color(Color32::GREEN));
                });
            });

            if !ws.blocked_urls.is_empty() {
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("BLOCKED PATTERNS:").small().color(Color32::from_rgb(255, 100, 100)));
                    let patterns: Vec<String> = ws.blocked_urls.iter().cloned().collect();
                    for p in patterns {
                        if ui.add(egui::Label::new(RichText::new(&p).small().color(Color32::GRAY)).sense(egui::Sense::click())).on_hover_text("Click to unblock").clicked() {
                            crate::ui::scrape::emit(AppEvent::RequestUrlUnblock(tid.clone(), p));
                        }
                    }
                });
            }
        });

        ui.add_space(5.0);

        // TABLE AREA
        let search_raw = ws.network_search.trim();
        let filtered_requests: Vec<_> = ws.network_requests.iter()
            .filter(|r| {
                if search_raw.is_empty() { return true; }
                
                if let Some(req_query) = search_raw.strip_prefix("req:") {
                    let q = req_query.trim().to_lowercase();
                    if q.is_empty() { return true; }
                    return r.request_body.as_ref().map(|b| b.to_lowercase().contains(&q)).unwrap_or(false);
                }
                
                if let Some(res_query) = search_raw.strip_prefix("res:") {
                    let q = res_query.trim().to_lowercase();
                    if q.is_empty() { return true; }
                    return r.response_body.as_ref().map(|b| b.to_lowercase().contains(&q)).unwrap_or(false);
                }

                // Default: Search in URL
                r.url.to_lowercase().contains(&search_raw.to_lowercase())
            })
            .collect();

        ui.push_id("network_table_area", |ui| {
            let table = TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .column(Column::auto().at_least(60.0))  // METHOD
                .column(Column::auto().at_least(50.0))  // STATUS
                .column(Column::auto().at_least(80.0))  // TYPE
                .column(Column::remainder().at_least(200.0)) // URL
                .column(Column::auto().at_least(120.0)); // ACTIONS

            table.header(20.0, |mut header| {
                header.col(|ui| { ui.strong("METHOD"); });
                header.col(|ui| { ui.strong("STATUS"); });
                header.col(|ui| { ui.strong("TYPE"); });
                header.col(|ui| { ui.strong("URL"); });
                header.col(|ui| { ui.strong("ACTIONS"); });
            })
            .body(|body| {
                body.rows(25.0, filtered_requests.len(), |mut row| {
                    let idx = row.index();
                    let req = filtered_requests[idx];

                    row.col(|ui| {
                        let method_color = match req.method.as_str() {
                            "GET" => Color32::LIGHT_BLUE,
                            "POST" => Color32::LIGHT_GREEN,
                            "PUT" | "PATCH" => Color32::GOLD,
                            "DELETE" => Color32::LIGHT_RED,
                            _ => Color32::GRAY,
                        };
                        ui.label(RichText::new(&req.method).strong().color(method_color));
                    });

                    row.col(|ui| {
                        let status_color = match req.status {
                            Some(s) if (200..300).contains(&s) => Color32::GREEN,
                            Some(s) if (300..400).contains(&s) => Color32::YELLOW,
                            Some(s) if (400..600).contains(&s) => Color32::RED,
                            _ => Color32::GRAY,
                        };
                        ui.label(RichText::new(req.status.map(|s| s.to_string()).unwrap_or_else(|| "-".into())).strong().color(status_color));
                    });

                    row.col(|ui| {
                        ui.label(RichText::new(&req.resource_type).small());
                    });

                    row.col(|ui| {
                        if ui.add(egui::Label::new(RichText::new(&req.url).small().monospace().color(Color32::LIGHT_GRAY)).truncate().sense(egui::Sense::click())).clicked() {
                             ui.ctx().copy_text(req.url.clone());
                        }
                    });

                    row.col(|ui| {
                        ui.horizontal(|ui| {
                            if ui.small_button("REQ").clicked() {
                                ws.active_request_id = Some(format!("req_{}", req.request_id));
                            }
                            if ui.small_button("RES").clicked() {
                                ws.active_request_id = Some(format!("res_{}", req.request_id));
                            }
                            
                            let is_blocked = ws.blocked_urls.contains(&req.url);
                            if is_blocked {
                                if ui.small_button(RichText::new("🔓").color(Color32::GREEN)).clicked() {
                                    crate::ui::scrape::emit(AppEvent::RequestUrlUnblock(tid.clone(), req.url.clone()));
                                }
                            } else {
                                if ui.small_button(RichText::new("🚫").color(Color32::RED)).clicked() {
                                    crate::ui::scrape::emit(AppEvent::RequestUrlBlock(tid.clone(), req.url.clone()));
                                }
                            }
                            
                            // CSS RESOURCE EXTRACTOR
                            let is_css = req.resource_type.to_lowercase().contains("style") || req.url.to_lowercase().ends_with(".css");
                            if is_css && req.response_body.is_some() {
                                if ui.small_button(RichText::new("✨").color(Color32::GOLD)).on_hover_text("Extract Assets").clicked() {
                                    if let Some(css_body) = &req.response_body {
                                        let found_urls = crate::core::browser::BrowserManager::extract_resources_from_css(css_body, &req.url);
                                        let mut added = 0;
                                        for url in found_urls {
                                            let name = url.split('/').last().unwrap_or("extracted_asset").to_string();
                                            let mime = if url.ends_with(".woff2") || url.ends_with(".ttf") { "font/woff2".to_string() } else { "image/extracted".to_string() };
                                            crate::ui::scrape::emit(crate::core::events::AppEvent::MediaCaptured(tid.clone(), crate::state::MediaAsset { name, url, mime_type: mime, size_bytes: 0, data: None }));
                                            added += 1;
                                        }
                                        tracing::info!("[CSS] Extracted {} assets.", added);
                                    }
                                }
                            }
                        });
                    });
                });
            });
        });

        // --- REQUEST INSPECTOR MODAL ---
        if let Some(act_id) = ws.active_request_id.clone() {
            let is_res = act_id.starts_with("res_");
            let rid = act_id.replace("req_", "").replace("res_", "");
            
            if let Some(req) = ws.network_requests.iter().find(|r| r.request_id == rid) {
                let title = if is_res { format!("RESPONSE: {}", req.url) } else { format!("REQUEST: {}", req.url) };
                let content = if is_res { req.response_body.as_deref().unwrap_or("Empty body") } else { req.request_body.as_deref().unwrap_or("No payload") };

                let mut open = true;
                egui::Window::new(RichText::new(title).strong().color(Color32::KHAKI))
                    .open(&mut open)
                    .default_size([600.0, 400.0])
                    .resizable(true)
                    .show(ui.ctx(), |ui| {
                        if ui.button("COPY TO CLIPBOARD").clicked() {
                            ui.ctx().copy_text(content.to_string());
                        }
                        ui.separator();
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let mut text = content.to_string();
                            ui.add(egui::TextEdit::multiline(&mut text)
                                .font(egui::FontId::monospace(11.0))
                                .desired_width(f32::INFINITY));
                        });
                    });
                if !open { ws.active_request_id = None; }
            } else {
                ws.active_request_id = None;
            }
        }
    }
}
