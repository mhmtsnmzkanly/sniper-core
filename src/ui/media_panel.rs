use crate::state::AppState;
use egui::{Ui, Color32, RichText, Frame};
use crate::core::events::AppEvent;
use crate::ui::scrape::emit;

pub fn render(ui: &mut Ui, state: &mut AppState) {
    let tid = match &state.selected_tab_id {
        Some(id) => id.clone(),
        None => { ui.label("No active tab selected."); return; }
    };

    // Extract state for local use
    let (media_assets, media_count, selected_media_urls, media_search, type_filter, preview_size, show_export) = {
        let ws = state.workspaces.get(&tid).unwrap();
        (
            ws.media_assets.clone(), 
            ws.media_assets.len(), 
            ws.selected_media_urls.clone(), 
            ws.media_search.clone(), 
            ws.media_type_filter.clone(), 
            ws.media_preview_size,
            ws.show_media_export
        )
    };

    ui.vertical(|ui| {
        // --- CONTROL HEADER ---
        Frame::group(ui.style()).fill(Color32::from_gray(25)).inner_margin(8.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button(RichText::new("🗑 CLEAR").color(Color32::LIGHT_RED)).clicked() {
                    if let Some(ws) = state.workspaces.get_mut(&tid) { 
                        ws.media_assets.clear(); 
                        ws.selected_media_urls.clear(); 
                    }
                }
                
                ui.separator();
                
                // TYPE FILTER
                ui.label(RichText::new("TYPE:").color(Color32::LIGHT_BLUE).strong());
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
                ui.label("SIZE:");
                if let Some(ws) = state.workspaces.get_mut(&tid) {
                    ui.add(egui::Slider::new(&mut ws.media_preview_size, 40.0..=250.0).show_value(false));
                }

                ui.separator();
                // BATCH DOWNLOAD
                let can_download = !selected_media_urls.is_empty();
                if ui.add_enabled(can_download, egui::Button::new(RichText::new(format!("📥 DOWNLOAD ({})", selected_media_urls.len())).strong().color(Color32::GREEN))).clicked() {
                    tracing::info!("[UI] Click: BATCH DOWNLOAD {} items", selected_media_urls.len());
                    for url in &selected_media_urls {
                        if let Some(asset) = media_assets.iter().find(|a| &a.url == url) {
                            if let Some(data) = &asset.data {
                                if let Some(path) = rfd::FileDialog::new().set_file_name(&asset.name).save_file() {
                                    let _ = std::fs::write(path, data);
                                }
                            }
                        }
                    }
                }

                ui.separator();
                // EXPORT JSON BUTTON (RESTORED)
                if ui.button(RichText::new("📄 EXPORT JSON").strong().color(Color32::from_rgb(0, 150, 255))).clicked() {
                    if let Some(ws) = state.workspaces.get_mut(&tid) {
                        ws.show_media_export = true;
                        // Default selections if empty
                        if ws.media_export_types.is_empty() { ws.media_export_types = ["IMAGE", "VIDEO", "AUDIO", "STYLES", "SCRIPTS", "FONTS"].iter().map(|s| s.to_string()).collect(); }
                        if ws.media_export_cols.is_empty() { ws.media_export_cols = ["url", "name", "type"].iter().map(|s| s.to_string()).collect(); }
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("TOTAL: {}", media_count)).color(Color32::GREEN).monospace());
                });
            });
        });

        ui.add_space(5.0);

        // --- EXPORT MODAL (RESTORED) ---
        if show_export {
            let mut open = true;
            egui::Window::new("JSON EXPORT SETTINGS")
                .open(&mut open)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.set_width(400.0);
                    if let Some(ws) = state.workspaces.get_mut(&tid) {
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
                                tracing::info!("[UI] Click: GENERATE JSON EXPORT");
                                let export_types = ws.media_export_types.clone();
                                let export_cols = ws.media_export_cols.clone();
                                let assets = ws.media_assets.clone();
                                
                                let mut results = Vec::new();
                                for asset in assets {
                                    let mt = asset.mime_type.to_lowercase();
                                    let mut matches = false;
                                    if export_types.contains("IMAGE") && (mt.contains("image") || asset.url.ends_with(".svg")) { matches = true; }
                                    if export_types.contains("VIDEO") && mt.contains("video") { matches = true; }
                                    if export_types.contains("AUDIO") && (mt.contains("audio") || asset.url.ends_with(".mp3")) { matches = true; }
                                    if export_types.contains("STYLES") && (mt.contains("style") || asset.url.ends_with(".css")) { matches = true; }
                                    if export_types.contains("SCRIPTS") && (mt.contains("script") || asset.url.ends_with(".js")) { matches = true; }
                                    if export_types.contains("FONTS") && mt.contains("font") { matches = true; }
                                    
                                    if matches {
                                        let mut entry = serde_json::Map::new();
                                        if export_cols.contains("url") { entry.insert("url".to_string(), serde_json::Value::String(asset.url)); }
                                        if export_cols.contains("name") { entry.insert("name".to_string(), serde_json::Value::String(asset.name)); }
                                        if export_cols.contains("type") { entry.insert("type".to_string(), serde_json::Value::String(asset.mime_type)); }
                                        if export_cols.contains("size") { entry.insert("size_bytes".to_string(), serde_json::Value::Number(asset.size_bytes.into())); }
                                        results.push(serde_json::Value::Object(entry));
                                    }
                                }

                                if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).set_file_name("media_export.json").save_file() {
                                    if let Ok(json) = serde_json::to_string_pretty(&results) {
                                        tracing::info!("[UI] JSON Export saved to: {:?}", path);
                                        let _ = std::fs::write(path, json);
                                    }
                                }
                                ws.show_media_export = false;
                            }
                            if ui.button("CANCEL").clicked() { ws.show_media_export = false; }
                        });
                    }
                });
            if !open { if let Some(ws) = state.workspaces.get_mut(&tid) { ws.show_media_export = false; } }
        }

        // --- FILTERING LOGIC ---
        let filtered_assets: Vec<_> = media_assets.into_iter().filter(|asset| {
            let search = media_search.to_lowercase();
            if !search.is_empty() && !asset.url.to_lowercase().contains(&search) && !asset.name.to_lowercase().contains(&search) { return false; }
            
            if !type_filter.is_empty() {
                let mt = asset.mime_type.to_lowercase();
                let mut match_type = false;
                if type_filter.contains("IMAGE") && (mt.contains("image") || asset.url.ends_with(".svg")) { match_type = true; }
                if type_filter.contains("VIDEO") && mt.contains("video") { match_type = true; }
                if type_filter.contains("AUDIO") && (mt.contains("audio") || asset.url.ends_with(".mp3")) { match_type = true; }
                if type_filter.contains("STYLES") && (mt.contains("style") || asset.url.ends_with(".css")) { match_type = true; }
                if type_filter.contains("SCRIPTS") && (mt.contains("script") || asset.url.ends_with(".js")) { match_type = true; }
                if type_filter.contains("FONTS") && mt.contains("font") { match_type = true; }
                if !match_type { return false; }
            }
            true
        }).collect();

        // --- ASSET GRID ---
        egui::ScrollArea::vertical().max_height(600.0).show(ui, |ui| {
            egui::Grid::new("media_grid_v4").striped(true).num_columns(6).spacing([15.0, 10.0]).show(ui, |ui| {
                for asset in filtered_assets {
                    let mut is_selected = selected_media_urls.contains(&asset.url);
                    if ui.checkbox(&mut is_selected, "").changed() {
                        if let Some(ws) = state.workspaces.get_mut(&tid) {
                            if is_selected { ws.selected_media_urls.insert(asset.url.clone()); }
                            else { ws.selected_media_urls.remove(&asset.url); }
                        }
                    }

                    // Preview
                    ui.allocate_ui(egui::vec2(preview_size, preview_size * 0.8), |ui| {
                        if asset.mime_type.starts_with("image/") {
                            if let Some(data) = &asset.data {
                                ui.add(egui::Image::from_bytes(format!("bytes://{}", asset.url), data.clone())
                                    .max_size(egui::vec2(preview_size, preview_size)));
                            }
                        } else {
                            let icon = if asset.mime_type.contains("video") { "🎬" }
                                      else if asset.mime_type.contains("audio") { "🎵" }
                                      else if asset.mime_type.contains("font") { "🅰" }
                                      else if asset.mime_type.contains("style") || asset.url.ends_with(".css") { "🎨" }
                                      else if asset.mime_type.contains("script") || asset.url.ends_with(".js") { "📜" }
                                      else { "📄" };
                            ui.centered_and_justified(|ui| { ui.label(RichText::new(icon).size(24.0)); });
                        }
                    });

                    // Info
                    ui.vertical(|ui| {
                        ui.set_width(300.0);
                        ui.label(RichText::new(&asset.name).strong());
                        ui.add(egui::Label::new(RichText::new(&asset.url).size(9.0).color(Color32::GRAY)).wrap());
                    });

                    ui.label(RichText::new(&asset.mime_type).small().color(Color32::LIGHT_BLUE));
                    ui.label(format!("{:.1} KB", asset.size_bytes as f64 / 1024.0));
                    
                    if ui.button("SAVE").clicked() {
                        if let Some(data) = &asset.data {
                            if let Some(path) = rfd::FileDialog::new().set_file_name(&asset.name).save_file() {
                                tracing::info!("[UI] Manual save for item {} to {:?}", asset.name, path);
                                let _ = std::fs::write(path, data);
                            }
                        }
                    }
                    ui.end_row();
                }
            });
        });
    });
}
