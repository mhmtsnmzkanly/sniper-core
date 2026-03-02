use crate::state::AppState;
use egui::{Ui, Color32, RichText};
use crate::core::events::AppEvent;
use crate::ui::scrape::emit;

pub fn render(ui: &mut Ui, state: &mut AppState) {
    let tid = match &state.selected_tab_id {
        Some(id) => id.clone(),
        None => { ui.label("No tab selected."); return; }
    };
    
    let (ws_title, media_count, media_assets, selected_media_urls, media_search) = {
        if !state.workspaces.contains_key(&tid) { return; }
        let ws = &state.workspaces[&tid];
        (ws.title.clone(), ws.media_assets.len(), ws.media_assets.clone(), ws.selected_media_urls.clone(), ws.media_search.clone())
    };

    ui.heading(format!("MEDIA FORENSIC ENGINE: {}", ws_title));
    ui.add_space(5.0);

    ui.group(|ui| {
        ui.horizontal(|ui| {
            if ui.button("🗑 CLEAR ALL").clicked() {
                if let Some(ws) = state.workspaces.get_mut(&tid) {
                    ws.media_assets.clear();
                    ws.selected_media_urls.clear();
                }
            }
            if ui.button("🔄 FORCE RELOAD").clicked() {
                emit(AppEvent::RequestPageReload(tid.clone()));
            }
            ui.separator();
            ui.label("🔍 FILTER:");
            if let Some(ws) = state.workspaces.get_mut(&tid) {
                ui.text_edit_singleline(&mut ws.media_search);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("Assets: {}", media_count));
            });
        });
    });

    ui.add_space(5.0);

    ui.horizontal(|ui| {
        if ui.button("✅ Select All").clicked() {
            if let Some(ws) = state.workspaces.get_mut(&tid) {
                for asset in &ws.media_assets { ws.selected_media_urls.insert(asset.url.clone()); }
            }
        }
        if ui.button("⬜ Deselect").clicked() {
            if let Some(ws) = state.workspaces.get_mut(&tid) {
                ws.selected_media_urls.clear();
            }
        }
        
        let selected_count = selected_media_urls.len();
        if ui.add_enabled(selected_count > 0, egui::Button::new(RichText::new(format!("💾 DOWNLOAD SELECTED ({})", selected_count)).strong())).clicked() {
            if let Some(first_asset) = media_assets.iter().find(|a| selected_media_urls.contains(&a.url)) {
                let root = state.config.output_dir.clone();
                if let Ok(dir) = crate::core::browser::BrowserManager::get_output_path(root, "MEDIA", &first_asset.url) {
                    let mut saved = 0;
                    for asset in &media_assets {
                        if selected_media_urls.contains(&asset.url) {
                            if let Some(data) = &asset.data {
                                let _ = std::fs::write(dir.join(&asset.name), data);
                                saved += 1;
                            }
                        }
                    }
                    state.notify("Batch Download", &format!("Successfully saved {} files to {:?}", saved, dir), false);
                }
            }
        }
    });

    ui.add_space(5.0);

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let search = media_search.to_lowercase();
        egui::Grid::new("media_grid_v6").striped(true).num_columns(7).spacing([15.0, 20.0]).show(ui, |ui| {
            ui.label(""); ui.label("PREVIEW"); ui.label("NAME"); ui.label("TYPE"); ui.label("SIZE"); ui.label("SOURCE"); ui.label("ACTION");
            ui.end_row();

            for asset in media_assets.iter().rev() {
                if !search.is_empty() && !asset.url.to_lowercase().contains(&search) { continue; }

                let mut is_selected = selected_media_urls.contains(&asset.url);
                if ui.checkbox(&mut is_selected, "").changed() {
                    if let Some(ws) = state.workspaces.get_mut(&tid) {
                        if is_selected { ws.selected_media_urls.insert(asset.url.clone()); }
                        else { ws.selected_media_urls.remove(&asset.url); }
                    }
                }

                ui.allocate_ui(egui::vec2(120.0, 120.0), |ui| {
                    if asset.mime_type.starts_with("image/") {
                        if let Some(data) = &asset.data {
                            let resp = ui.add(egui::Image::from_bytes(format!("bytes://{}", asset.url), data.clone())
                                .max_size(egui::vec2(110.0, 110.0)).corner_radius(8.0).sense(egui::Sense::click()));
                            if resp.clicked() {
                                if let Some(ws) = state.workspaces.get_mut(&tid) {
                                    ws.active_media_url = Some(asset.url.clone());
                                }
                            }
                        }
                    } else { ui.label("🎬 MEDIA"); }
                });

                ui.label(RichText::new(&asset.name).strong());
                ui.label(RichText::new(&asset.mime_type).small().color(Color32::LIGHT_BLUE));
                ui.label(RichText::new(format!("{:.1} KB", asset.size_bytes as f64 / 1024.0)).monospace().color(Color32::YELLOW));
                
                ui.horizontal(|ui| {
                    let trunc_url: String = asset.url.chars().take(25).collect::<String>() + "...";
                    ui.label(RichText::new(trunc_url).small().color(Color32::GRAY));
                    if ui.button("📋").on_hover_text("Copy URL to clipboard").clicked() {
                        ui.ctx().copy_text(asset.url.clone());
                    }
                });

                ui.vertical(|ui| {
                    if ui.button("💾 SAVE").clicked() {
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
                egui::Window::new(RichText::new(&asset.name).strong())
                    .open(&mut open)
                    .default_size([500.0, 500.0])
                    .resizable(true)
                    .show(ui.ctx(), |ui| {
                        if let Some(data) = &asset.data {
                            ui.add(egui::Image::from_bytes(format!("preview://{}", asset.url), data.clone())
                                .max_size(ui.available_size()));
                        } else {
                            ui.label("Binary data not available.");
                        }
                    });
                if !open { ws.active_media_url = None; }
            } else {
                ws.active_media_url = None;
            }
        }
    }
}
