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
    let (media_assets, media_count, selected_media_urls, media_search, sort_col, sort_asc, type_filter, preview_size) = if let Some(ws) = state.workspaces.get(&tid) {
        (ws.media_assets.clone(), ws.media_assets.len(), ws.selected_media_urls.clone(), ws.media_search.clone(), ws.media_sort_col.clone(), ws.media_sort_asc, ws.media_type_filter.clone(), ws.media_preview_size)
    } else {
        (Vec::new(), 0, std::collections::HashSet::new(), String::new(), "name".to_string(), true, std::collections::HashSet::new(), 100.0)
    };

    let frame_style = Frame::group(ui.style()).fill(Color32::from_gray(25)).stroke(Stroke::new(1.0, Color32::from_gray(50)));

    frame_style.show(ui, |ui| {
        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(RichText::new("CLEAR").strong().color(Color32::LIGHT_RED))).clicked() {
                if let Some(ws) = state.workspaces.get_mut(&tid) {
                    ws.media_assets.clear();
                    ws.selected_media_urls.clear();
                }
            }
            
            ui.separator();
            
            // --- AUTOMATIC CSS SCAN ---
            if ui.add(egui::Button::new(RichText::new("DEEP SCAN (CSS)").strong().color(Color32::GOLD))).on_hover_text("Extract resources from all captured CSS files").clicked() {
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
                        let mime = if f_url.ends_with(".woff2") || f_url.ends_with(".ttf") { "font/woff2".to_string() } 
                                   else if f_url.ends_with(".svg") { "image/svg+xml".to_string() }
                                   else { "image/extracted".into() };

                        emit(AppEvent::MediaCaptured(tid_inner.clone(), crate::state::MediaAsset {
                            name, url: f_url, mime_type: mime, size_bytes: 0, data: None
                        }));
                    }
                }
            }

            ui.separator();
            ui.label(RichText::new("TYPE:").strong().color(Color32::LIGHT_BLUE));
            if let Some(ws) = state.workspaces.get_mut(&tid) {
                let types = ["IMAGE", "VIDEO", "AUDIO", "STYLES", "SCRIPTS", "FONTS"];
                ui.menu_button(format!("TYPES ({})", ws.media_type_filter.len()), |ui| {
                    for t in types {
                        let mut is_selected = ws.media_type_filter.contains(t);
                        if ui.checkbox(&mut is_selected, t).changed() {
                            if is_selected { ws.media_type_filter.insert(t.to_string()); }
                            else { ws.media_type_filter.remove(t); }
                        }
                    }
                    if ui.button("CLEAR ALL").clicked() { ws.media_type_filter.clear(); }
                });
            }

            ui.separator();
            ui.label(RichText::new("SEARCH:").strong().color(Color32::LIGHT_BLUE));
            if let Some(ws) = state.workspaces.get_mut(&tid) {
                ui.add(egui::TextEdit::singleline(&mut ws.media_search).desired_width(120.0));
            }

            ui.separator();
            ui.label(RichText::new("SIZE:").strong().color(Color32::LIGHT_BLUE));
            if let Some(ws) = state.workspaces.get_mut(&tid) {
                ui.add(egui::Slider::new(&mut ws.media_preview_size, 40.0..=250.0).show_value(false));
            }

            ui.separator();
            if ui.add(egui::Button::new(RichText::new("EXPORT JSON").strong().color(Color32::from_rgb(0, 150, 255)))).on_hover_text("Export filtered/selected media to JSON with custom columns").clicked() {
                if let Some(ws) = state.workspaces.get_mut(&tid) {
                    ws.show_media_export = true;
                    if ws.media_export_types.is_empty() { ws.media_export_types = ["IMAGE", "VIDEO", "AUDIO", "STYLES", "SCRIPTS", "FONTS"].iter().map(|s| s.to_string()).collect(); }
                    if ws.media_export_cols.is_empty() { ws.media_export_cols = ["url", "name", "type"].iter().map(|s| s.to_string()).collect(); }
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(format!("TOTAL: {}", media_count)).monospace().color(Color32::GREEN));
            });
        });
    });

    ui.add_space(8.0);

    // --- EXPORT MODAL ---
    if let Some(ws) = state.workspaces.get_mut(&tid) {
        if ws.show_media_export {
            let mut open = true;
            egui::Window::new("JSON EXPORT SETTINGS")
                .open(&mut open)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.set_width(400.0);
                    ui.heading("Select Media Types");
                    ui.horizontal_wrapped(|ui| {
                        for t in ["IMAGE", "VIDEO", "AUDIO", "STYLES", "SCRIPTS", "FONTS"] {
                            let mut is_selected = ws.media_export_types.contains(t);
                            if ui.checkbox(&mut is_selected, t).changed() {
                                if is_selected { ws.media_export_types.insert(t.to_string()); }
                                else { ws.media_export_types.remove(t); }
                            }
                        }
                    });
                    ui.add_space(10.0);
                    ui.heading("Select Columns");
                    ui.horizontal_wrapped(|ui| {
                        for c in ["url", "name", "type", "size"] {
                            let mut is_selected = ws.media_export_cols.contains(c);
                            if ui.checkbox(&mut is_selected, c).changed() {
                                if is_selected { ws.media_export_cols.insert(c.to_string()); }
                                else { ws.media_export_cols.remove(c); }
                            }
                        }
                    });
                    ui.add_space(15.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("GENERATE EXPORT").strong().color(Color32::GREEN)).clicked() {
                            let export_types = ws.media_export_types.clone();
                            let export_cols = ws.media_export_cols.clone();
                            let assets = ws.media_assets.clone();
                            
                            let mut results = Vec::new();
                            for asset in assets {
                                let mt = asset.mime_type.to_lowercase();
                                let mut matches = false;
                                if export_types.contains("IMAGE") && (mt.contains("image") || asset.url.ends_with(".svg")) { matches = true; }
                                if export_types.contains("VIDEO") && mt.contains("video") { matches = true; }
                                if export_types.contains("AUDIO") && (mt.contains("audio") || asset.url.ends_with(".mp3") || asset.url.ends_with(".wav")) { matches = true; }
                                if export_types.contains("STYLES") && (mt.contains("style") || asset.url.ends_with(".css")) { matches = true; }
                                if export_types.contains("SCRIPTS") && (mt.contains("script") || asset.url.ends_with(".js")) { matches = true; }
                                if export_types.contains("FONTS") && (mt.contains("font") || asset.url.ends_with(".woff") || asset.url.ends_with(".woff2") || asset.url.ends_with(".ttf")) { matches = true; }
                                
                                if matches {
                                    let mut entry = serde_json::Map::new();
                                    if export_cols.contains("url") { entry.insert("url".to_string(), serde_json::Value::String(asset.url)); }
                                    if export_cols.contains("name") { entry.insert("name".to_string(), serde_json::Value::String(asset.name)); }
                                    if export_cols.contains("type") { entry.insert("type".to_string(), serde_json::Value::String(asset.mime_type)); }
                                    if export_cols.contains("size") { entry.insert("size_bytes".to_string(), serde_json::Value::Number(asset.size_bytes.into())); }
                                    results.push(serde_json::Value::Object(entry));
                                }
                            }

                            if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).set_file_name("export.json").save_file() {
                                if let Ok(json) = serde_json::to_string_pretty(&results) {
                                    let _ = std::fs::write(path, json);
                                }
                            }
                            ws.show_media_export = false;
                        }
                        if ui.button("CANCEL").clicked() { ws.show_media_export = false; }
                    });
                });
            if !open { ws.show_media_export = false; }
        }
    }

    // --- FILTER & SORTING LOGIC ---
    let mut filtered_assets: Vec<_> = media_assets.into_iter().filter(|asset| {
        let search = media_search.to_lowercase();
        if !search.is_empty() && !asset.url.to_lowercase().contains(&search) && !asset.name.to_lowercase().contains(&search) { return false; }
        
        if !type_filter.is_empty() {
            let mt = asset.mime_type.to_lowercase();
            let mut match_type = false;
            if type_filter.contains("IMAGE") && (mt.contains("image") || asset.url.ends_with(".svg")) { match_type = true; }
            if type_filter.contains("VIDEO") && mt.contains("video") { match_type = true; }
            if type_filter.contains("AUDIO") && (mt.contains("audio") || asset.url.ends_with(".mp3") || asset.url.ends_with(".wav")) { match_type = true; }
            if type_filter.contains("STYLES") && (mt.contains("style") || asset.url.ends_with(".css")) { match_type = true; }
            if type_filter.contains("SCRIPTS") && (mt.contains("script") || asset.url.ends_with(".js")) { match_type = true; }
            if type_filter.contains("FONTS") && (mt.contains("font") || asset.url.ends_with(".woff") || asset.url.ends_with(".woff2") || asset.url.ends_with(".ttf")) { match_type = true; }
            if !match_type { return false; }
        }
        true
    }).collect();

    match sort_col.as_str() {
        "name" => filtered_assets.sort_by(|a, b| if sort_asc { a.name.to_lowercase().cmp(&b.name.to_lowercase()) } else { b.name.to_lowercase().cmp(&a.name.to_lowercase()) }),
        "size" => filtered_assets.sort_by(|a, b| if sort_asc { a.size_bytes.cmp(&b.size_bytes) } else { b.size_bytes.cmp(&a.size_bytes) }),
        "type" => filtered_assets.sort_by(|a, b| if sort_asc { a.mime_type.cmp(&b.mime_type) } else { b.mime_type.cmp(&a.mime_type) }),
        _ => {}
    }

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        egui::Grid::new("media_grid_v10").striped(true).num_columns(7).spacing([15.0, 12.0]).show(ui, |ui| {
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

            for asset in filtered_assets {
                let mut is_selected = selected_media_urls.contains(&asset.url);
                if ui.checkbox(&mut is_selected, "").changed() {
                    if let Some(ws) = state.workspaces.get_mut(&tid) {
                        if is_selected { ws.selected_media_urls.insert(asset.url.clone()); }
                        else { ws.selected_media_urls.remove(&asset.url); }
                    }
                }

                ui.allocate_ui(egui::vec2(preview_size, preview_size * 0.8), |ui| {
                    if asset.mime_type.starts_with("image/") || asset.mime_type.contains("svg") {
                        if let Some(data) = &asset.data {
                            let resp = ui.add(egui::Image::from_bytes(format!("bytes://{}", asset.url), data.clone())
                                .max_size(egui::vec2(preview_size, preview_size)).corner_radius(4.0).sense(egui::Sense::click()));
                            if resp.clicked() {
                                if let Some(ws) = state.workspaces.get_mut(&tid) {
                                    ws.active_media_url = Some(asset.url.clone());
                                }
                            }
                        }
                    } else if asset.mime_type.contains("video") {
                        ui.label(RichText::new("🎬 VIDEO").color(Color32::LIGHT_BLUE));
                    } else if asset.mime_type.contains("audio") {
                        ui.label(RichText::new("🎵 AUDIO").color(Color32::LIGHT_GREEN));
                    } else if asset.mime_type.contains("font") {
                        ui.label(RichText::new("🅰 FONT").color(Color32::LIGHT_YELLOW));
                    } else if asset.mime_type.contains("style") || asset.url.ends_with(".css") {
                        ui.label(RichText::new("🎨 CSS").color(Color32::KHAKI));
                    } else if asset.mime_type.contains("script") || asset.url.ends_with(".js") {
                        ui.label(RichText::new("📜 JS").color(Color32::LIGHT_GRAY));
                    } else { ui.label("📄 FILE"); }
                });

                let short_name: String = if asset.name.chars().count() > 15 { asset.name.chars().take(12).collect::<String>() + "..." } else { asset.name.clone() };
                ui.add(egui::Label::new(RichText::new(short_name).strong())).on_hover_text(&asset.name);
                ui.label(RichText::new(&asset.mime_type).small().color(Color32::LIGHT_BLUE));
                ui.label(RichText::new(format!("{:.1} KB", asset.size_bytes as f64 / 1024.0)).monospace().color(Color32::YELLOW));
                
                ui.horizontal(|ui| {
                    let trunc_url: String = if asset.url.chars().count() > 40 { asset.url.chars().take(37).collect::<String>() + "..." } else { asset.url.clone() };
                    if ui.add(egui::Label::new(RichText::new(trunc_url).small().color(Color32::from_gray(180))).sense(egui::Sense::click())).on_hover_text("Click to copy URL").clicked() {
                        ui.ctx().copy_text(asset.url.clone());
                    }
                });

                ui.vertical(|ui| {
                    if ui.button("SAVE").clicked() {
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
