use crate::state::AppState;
use egui::{Ui, Color32, RichText, Frame, Stroke};
use crate::core::events::AppEvent;
use crate::ui::scrape::emit;

pub fn render(ui: &mut Ui, state: &mut AppState) {
    let tid = match &state.selected_tab_id {
        Some(id) => id.clone(),
        None => { ui.label(RichText::new("NO TARGET SELECTED").strong().color(Color32::RED)); return; }
    };

    // Extract state to avoid borrow conflicts
    let (media_assets, media_count, selected_media_urls, media_search, sort_col, sort_asc) = if let Some(ws) = state.workspaces.get(&tid) {
        (ws.media_assets.clone(), ws.media_assets.len(), ws.selected_media_urls.clone(), ws.media_search.clone(), ws.media_sort_col.clone(), ws.media_sort_asc)
    } else {
        (Vec::new(), 0, std::collections::HashSet::new(), String::new(), "name".to_string(), true)
    };

    let frame_style = Frame::group(ui.style()).fill(Color32::from_gray(25)).stroke(Stroke::new(1.0, Color32::from_gray(50)));

    frame_style.show(ui, |ui| {
        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(RichText::new("CLEAR ASSETS").strong().color(Color32::LIGHT_RED))).clicked() {
                if let Some(ws) = state.workspaces.get_mut(&tid) {
                    ws.media_assets.clear();
                    ws.selected_media_urls.clear();
                }
            }
            if ui.add(egui::Button::new(RichText::new("FORCE RELOAD").strong())).clicked() {
                emit(AppEvent::RequestPageReload(tid.clone()));
            }
            
            ui.separator();
            
            // --- AUTOMATIC CSS SCAN ---
            if ui.add(egui::Button::new(RichText::new("DEEP SCAN (CSS)").strong().color(Color32::GOLD))).on_hover_text("Automatically extract resources from all captured CSS files").clicked() {
                let css_requests: Vec<(String, String)> = {
                    let ws = &state.workspaces[&tid];
                    ws.network_requests.iter()
                        .filter(|r| (r.resource_type.to_lowercase().contains("style") || r.url.to_lowercase().ends_with(".css")) && r.response_body.is_some())
                        .map(|r| (r.url.clone(), r.response_body.as_ref().unwrap().clone()))
                        .collect()
                };
                
                let tid_inner = tid.clone();
                for (url, body) in css_requests {
                    let found_urls = crate::core::browser::BrowserManager::extract_resources_from_css(&body, &url);
                    for f_url in found_urls {
                        let name = f_url.split('/').last().unwrap_or("extracted").to_string();
                        emit(AppEvent::MediaCaptured(tid_inner.clone(), crate::state::MediaAsset {
                            name, url: f_url, mime_type: "image/extracted".into(), size_bytes: 0, data: None
                        }));
                    }
                }
            }

            ui.separator();
            ui.label(RichText::new("FILTER:").strong().color(Color32::LIGHT_BLUE));
            if let Some(ws) = state.workspaces.get_mut(&tid) {
                ui.add(egui::TextEdit::singleline(&mut ws.media_search).desired_width(200.0));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(format!("TOTAL ASSETS: {}", media_count)).monospace().color(Color32::GREEN));
            });
        });
    });

    ui.add_space(8.0);

    // --- SORTING LOGIC ---
    let mut media_assets = media_assets;
    match sort_col.as_str() {
        "name" => media_assets.sort_by(|a, b| if sort_asc { a.name.to_lowercase().cmp(&b.name.to_lowercase()) } else { b.name.to_lowercase().cmp(&a.name.to_lowercase()) }),
        "size" => media_assets.sort_by(|a, b| if sort_asc { a.size_bytes.cmp(&b.size_bytes) } else { b.size_bytes.cmp(&a.size_bytes) }),
        "type" => media_assets.sort_by(|a, b| if sort_asc { a.mime_type.cmp(&b.mime_type) } else { b.mime_type.cmp(&a.mime_type) }),
        _ => {}
    }

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let search = media_search.to_lowercase();
        egui::Grid::new("media_grid_v9").striped(true).num_columns(7).spacing([15.0, 12.0]).show(ui, |ui| {
            // --- HEADER WITH SORTING ---
            ui.label(""); 
            ui.label(RichText::new("PREVIEW").strong().color(Color32::LIGHT_GRAY));
            
            if ui.button(RichText::new("NAME").strong()).clicked() {
                if let Some(ws) = state.workspaces.get_mut(&tid) {
                    if ws.media_sort_col == "name" { ws.media_sort_asc = !ws.media_sort_asc; }
                    else { ws.media_sort_col = "name".to_string(); ws.media_sort_asc = true; }
                }
            }
            if ui.button(RichText::new("TYPE").strong()).clicked() {
                if let Some(ws) = state.workspaces.get_mut(&tid) {
                    if ws.media_sort_col == "type" { ws.media_sort_asc = !ws.media_sort_asc; }
                    else { ws.media_sort_col = "type".to_string(); ws.media_sort_asc = true; }
                }
            }
            if ui.button(RichText::new("SIZE").strong()).clicked() {
                if let Some(ws) = state.workspaces.get_mut(&tid) {
                    if ws.media_sort_col == "size" { ws.media_sort_asc = !ws.media_sort_asc; }
                    else { ws.media_sort_col = "size".to_string(); ws.media_sort_asc = true; }
                }
            }
            ui.label(RichText::new("SOURCE URL").strong().color(Color32::LIGHT_GRAY));
            ui.label(RichText::new("ACTION").strong().color(Color32::LIGHT_GRAY));
            ui.end_row();

            for asset in media_assets {
                if !search.is_empty() && !asset.url.to_lowercase().contains(&search) { continue; }

                let mut is_selected = selected_media_urls.contains(&asset.url);
                if ui.checkbox(&mut is_selected, "").changed() {
                    if let Some(ws) = state.workspaces.get_mut(&tid) {
                        if is_selected { ws.selected_media_urls.insert(asset.url.clone()); }
                        else { ws.selected_media_urls.remove(&asset.url); }
                    }
                }

                ui.allocate_ui(egui::vec2(100.0, 80.0), |ui| {
                    if asset.mime_type.starts_with("image/") {
                        if let Some(data) = &asset.data {
                            let resp = ui.add(egui::Image::from_bytes(format!("bytes://{}", asset.url), data.clone())
                                .max_size(egui::vec2(90.0, 90.0)).corner_radius(4.0).sense(egui::Sense::click()));
                            if resp.clicked() {
                                if let Some(ws) = state.workspaces.get_mut(&tid) {
                                    ws.active_media_url = Some(asset.url.clone());
                                }
                            }
                        }
                    } else { ui.label("NO PREVIEW"); }
                });

                let short_name: String = if asset.name.chars().count() > 15 { asset.name.chars().take(12).collect::<String>() + "..." } else { asset.name.clone() };
                ui.add(egui::Label::new(RichText::new(short_name).strong())).on_hover_text(&asset.name);
                ui.label(RichText::new(&asset.mime_type).small().color(Color32::LIGHT_BLUE));
                ui.label(RichText::new(format!("{:.1} KB", asset.size_bytes as f64 / 1024.0)).monospace().color(Color32::YELLOW));
                
                ui.horizontal(|ui| {
                    let trunc_url: String = if asset.url.chars().count() > 50 { asset.url.chars().take(47).collect::<String>() + "..." } else { asset.url.clone() };
                    if ui.add(egui::Label::new(RichText::new(trunc_url).small().color(Color32::from_gray(180))).sense(egui::Sense::click())).on_hover_text("Click to copy URL").clicked() {
                        ui.ctx().copy_text(asset.url.clone());
                    }
                });

                ui.vertical(|ui| {
                    if ui.button("SAVE FILE").clicked() {
                        if let Some(data) = &asset.data {
                            if let Some(path) = rfd::FileDialog::new().set_file_name(&asset.name).save_file() {
                                let _ = std::fs::write(&path, data);
                            }
                        }
                    }
                });
                ui.end_row();
            }
        });
    });

    // --- MEDIA PREVIEW MODAL ---
    if let Some(ws) = state.workspaces.get_mut(&tid) {
        if let Some(url) = ws.active_media_url.clone() {
            if let Some(asset) = ws.media_assets.iter().find(|a| a.url == url) {
                let mut open = true;
                egui::Window::new(RichText::new(format!("PREVIEW: {}", asset.name)).strong())
                    .open(&mut open)
                    .default_size([800.0, 600.0])
                    .max_width(1024.0)
                    .max_height(720.0)
                    .resizable(true)
                    .vscroll(true)
                    .hscroll(true)
                    .show(ui.ctx(), |ui| {
                        if let Some(data) = &asset.data {
                            ui.add(egui::Image::from_bytes(format!("preview://{}", asset.url), data.clone())
                                .corner_radius(4.0));
                        } else {
                            ui.vertical_centered(|ui| {
                                ui.add_space(20.0);
                                ui.label(RichText::new("Binary data not available for this resource.").italics());
                            });
                        }
                    });
                if !open { ws.active_media_url = None; }
            } else {
                ws.active_media_url = None;
            }
        }
    }
}
