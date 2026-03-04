use crate::state::AppState;
use egui::{Ui, Color32, RichText, Frame};

pub fn render(ui: &mut Ui, state: &mut AppState) {
    let tid = match &state.selected_tab_id {
        Some(id) => id.clone(),
        None => { ui.label("No active tab selected."); return; }
    };

    let (media_assets, media_count, selected_media_urls, media_search, _sort_col, _sort_asc, type_filter, preview_size) = {
        let ws = state.workspaces.get(&tid).unwrap();
        (ws.media_assets.clone(), ws.media_assets.len(), ws.selected_media_urls.clone(), ws.media_search.clone(), ws.media_sort_col.clone(), ws.media_sort_asc, ws.media_type_filter.clone(), ws.media_preview_size)
    };

    ui.vertical(|ui| {
        Frame::group(ui.style()).fill(Color32::from_gray(25)).inner_margin(8.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button(RichText::new("🗑 CLEAR").color(Color32::LIGHT_RED)).clicked() {
                    if let Some(ws) = state.workspaces.get_mut(&tid) { ws.media_assets.clear(); ws.selected_media_urls.clear(); }
                }
                
                ui.separator();
                ui.label(RichText::new("TYPE:").color(Color32::LIGHT_BLUE));
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
                    });
                }

                ui.separator();
                ui.label("SIZE:");
                if let Some(ws) = state.workspaces.get_mut(&tid) {
                    ui.add(egui::Slider::new(&mut ws.media_preview_size, 40.0..=250.0).show_value(false));
                }

                ui.separator();
                let can_download = !selected_media_urls.is_empty();
                if ui.add_enabled(can_download, egui::Button::new(RichText::new(format!("📥 DOWNLOAD SELECTED ({})", selected_media_urls.len())).strong().color(Color32::GREEN))).clicked() {
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

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("TOTAL: {}", media_count)).color(Color32::GREEN).monospace());
                });
            });
        });

        ui.add_space(5.0);

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

        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("media_grid").striped(true).num_columns(6).spacing([15.0, 10.0]).show(ui, |ui| {
                for asset in filtered_assets {
                    let mut is_selected = selected_media_urls.contains(&asset.url);
                    if ui.checkbox(&mut is_selected, "").changed() {
                        if let Some(ws) = state.workspaces.get_mut(&tid) {
                            if is_selected { ws.selected_media_urls.insert(asset.url.clone()); }
                            else { ws.selected_media_urls.remove(&asset.url); }
                        }
                    }

                    ui.allocate_ui(egui::vec2(preview_size, preview_size * 0.8), |ui| {
                        if asset.mime_type.starts_with("image/") {
                            if let Some(data) = &asset.data {
                                ui.add(egui::Image::from_bytes(format!("bytes://{}", asset.url), data.clone())
                                    .max_size(egui::vec2(preview_size, preview_size)));
                            }
                        } else {
                            ui.label(RichText::new("FILE").color(Color32::GRAY));
                        }
                    });

                    ui.label(RichText::new(&asset.name).strong());
                    ui.label(RichText::new(&asset.mime_type).small().color(Color32::LIGHT_BLUE));
                    ui.label(format!("{:.1} KB", asset.size_bytes as f64 / 1024.0));
                    if ui.button("SAVE").clicked() {
                        if let Some(data) = &asset.data {
                            if let Some(path) = rfd::FileDialog::new().set_file_name(&asset.name).save_file() {
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
