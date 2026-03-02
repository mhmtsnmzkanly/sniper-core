use crate::state::AppState;
use egui::{Ui, Color32, RichText, Frame, Stroke};
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

        egui::Frame::default().fill(Color32::from_black_alpha(30)).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                let search = ws.network_search.to_lowercase();
                egui::Grid::new("network_grid_v9").striped(true).num_columns(5).spacing([10.0, 8.0]).show(ui, |ui| {
                    ui.label(RichText::new("METHOD").strong().color(Color32::LIGHT_GRAY));
                    ui.label(RichText::new("STATUS").strong().color(Color32::LIGHT_GRAY));
                    ui.label(RichText::new("TYPE").strong().color(Color32::LIGHT_GRAY));
                    ui.label(RichText::new("URL (CLICK TO INSPECT)").strong().color(Color32::LIGHT_GRAY));
                    ui.label(RichText::new("ACTIONS").strong().color(Color32::LIGHT_GRAY));
                    ui.end_row();

                    for req in &ws.network_requests {
                        if !search.is_empty() && !req.url.to_lowercase().contains(&search) { continue; }

                        let method_color = match req.method.as_str() {
                            "GET" => Color32::LIGHT_BLUE,
                            "POST" => Color32::LIGHT_GREEN,
                            "PUT" | "PATCH" => Color32::GOLD,
                            "DELETE" => Color32::LIGHT_RED,
                            _ => Color32::GRAY,
                        };
                        ui.label(RichText::new(&req.method).strong().color(method_color));

                        let status_color = match req.status {
                            Some(s) if (200..300).contains(&s) => Color32::GREEN,
                            Some(s) if (300..400).contains(&s) => Color32::YELLOW,
                            Some(s) if (400..600).contains(&s) => Color32::RED,
                            _ => Color32::GRAY,
                        };
                        ui.label(RichText::new(req.status.map(|s| s.to_string()).unwrap_or_else(|| "-".into())).strong().color(status_color));

                        ui.label(RichText::new(&req.resource_type).small());

                        let trunc_url: String = if req.url.chars().count() > 80 { req.url.chars().take(77).collect::<String>() + "..." } else { req.url.clone() };
                        if ui.add(egui::Label::new(RichText::new(trunc_url).small().monospace().color(Color32::LIGHT_GRAY)).sense(egui::Sense::click())).clicked() {
                             ui.ctx().copy_text(req.url.clone());
                        }

                        ui.horizontal(|ui| {
                            if ui.button("REQ").on_hover_text("View Request Payload").clicked() {
                                ws.active_request_id = Some(format!("req_{}", req.request_id));
                            }
                            if ui.button("RES").on_hover_text("View Response Body").clicked() {
                                ws.active_request_id = Some(format!("res_{}", req.request_id));
                            }
                            
                            let is_blocked = ws.blocked_urls.contains(&req.url);
                            let block_btn = if is_blocked { 
                                egui::Button::new(RichText::new("UNBLOCK").color(Color32::GREEN)) 
                            } else { 
                                egui::Button::new(RichText::new("BLOCK").color(Color32::RED)) 
                            };
                            
                            if ui.add(block_btn).clicked() {
                                if is_blocked {
                                    crate::ui::scrape::emit(AppEvent::RequestUrlUnblock(tid.clone(), req.url.clone()));
                                } else {
                                    crate::ui::scrape::emit(AppEvent::RequestUrlBlock(tid.clone(), req.url.clone()));
                                }
                            }
                            
                            // --- CSS RESOURCE EXTRACTOR ---
                            let is_css = req.resource_type.to_lowercase().contains("style") || req.url.to_lowercase().ends_with(".css");
                            if is_css && req.response_body.is_some() {
                                if ui.add(egui::Button::new(RichText::new("EXTRACT").color(Color32::GOLD))).on_hover_text("Extract hidden assets").clicked() {
                                    if let Some(css_body) = &req.response_body {
                                        let found_urls = crate::core::browser::BrowserManager::extract_resources_from_css(css_body, &req.url);
                                        let mut added = 0;
                                        
                                        let tid_inner = tid.clone();
                                        for url in found_urls {
                                            let name = url.split('/').last().unwrap_or("extracted_asset").to_string();
                                            let mime = if url.ends_with(".woff2") || url.ends_with(".ttf") { "font/woff2".to_string() } else { "image/extracted".to_string() };
                                            
                                            crate::ui::scrape::emit(crate::core::events::AppEvent::MediaCaptured(tid_inner.clone(), crate::state::MediaAsset {
                                                name,
                                                url,
                                                mime_type: mime,
                                                size_bytes: 0,
                                                data: None,
                                            }));
                                            added += 1;
                                        }
                                        tracing::info!("[CSS <-> EXTRACT] Found and added {} resources from CSS.", added);
                                    }
                                }
                            }
                        });
                        ui.end_row();
                    }
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
