use crate::core::events::AppEvent;
use crate::state::AppState;
use crate::ui::{design, scrape::emit};
use egui::{Color32, Frame, RichText, Ui, Stroke};

pub fn render(ui: &mut Ui, state: &mut AppState, tid: &str) {
    // Extract state for local use
    let (media_assets, media_count, selected_media_urls, media_search, mut type_filter, mut preview_size, _show_export, mut sort_col, mut sort_asc, mut min_size_kb) = {
        let ws = state.workspaces.get(tid).unwrap();
        (
            ws.media_assets.clone(), 
            ws.media_assets.len(), 
            ws.selected_media_urls.clone(), 
            ws.media_search.clone(), 
            ws.media_type_filter.clone(), 
            ws.media_preview_size,
            ws.show_media_export,
            ws.media_sort_col.clone(),
            ws.media_sort_asc,
            ws.media_min_size_kb
        )
    };

    ui.vertical(|ui| {
        design::title(ui, "Media Vault", design::ACCENT_CYAN);
        ui.label(RichText::new("Captured assets with preview, filtering and batch download").small().color(design::TEXT_MUTED));
        ui.add_space(8.0);

        // --- CONTROL HEADER ---
        Frame::group(ui.style()).fill(design::BG_SURFACE).inner_margin(10.0).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                // Actions
                if ui.button(RichText::new("🗑 CLEAR ALL").color(Color32::from_rgb(255, 100, 100))).clicked() {
                    if let Some(ws) = state.workspaces.get_mut(tid) { 
                        ws.media_assets.clear(); 
                        ws.selected_media_urls.clear(); 
                    }
                }
                
                ui.separator();
                
                // TYPE FILTER
                ui.label(RichText::new("TYPES:").color(design::ACCENT_ORANGE).strong());
                let types = ["IMAGE", "VIDEO", "AUDIO", "STYLES", "SCRIPTS", "FONTS"];
                ui.menu_button(format!("({})", type_filter.len()), |ui| {
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
                ui.label("MIN SIZE:");
                ui.add(egui::DragValue::new(&mut min_size_kb).suffix(" KB").speed(10.0));

                ui.separator();
                ui.label("SORT:");
                egui::ComboBox::from_id_salt(format!("{}_media_sort", tid))
                    .selected_text(&sort_col)
                    .width(80.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut sort_col, "name".into(), "Name");
                        ui.selectable_value(&mut sort_col, "type".into(), "Type");
                        ui.selectable_value(&mut sort_col, "size".into(), "Size");
                    });
                if ui.button(if sort_asc { "🔼" } else { "🔽" }).clicked() { sort_asc = !sort_asc; }

                ui.separator();
                ui.label("ZOOM:");
                ui.add(egui::Slider::new(&mut preview_size, 60.0..=220.0).show_value(false));

                ui.separator();
                // BATCH DOWNLOAD
                let can_download = !selected_media_urls.is_empty();
                if ui.add_enabled(can_download, egui::Button::new(RichText::new(format!("📥 DOWNLOAD ({})", selected_media_urls.len())).strong().color(design::ACCENT_GREEN))).clicked() {
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
                // EXPORT JSON
                if ui.button(RichText::new("📄 EXPORT JSON").color(Color32::from_rgb(100, 180, 255))).clicked() {
                    if let Some(ws) = state.workspaces.get_mut(tid) {
                        ws.show_media_export = true;
                        if ws.media_export_types.is_empty() { ws.media_export_types = ["IMAGE", "VIDEO", "AUDIO", "STYLES", "SCRIPTS", "FONTS"].iter().map(|s| s.to_string()).collect(); }
                        if ws.media_export_cols.is_empty() { ws.media_export_cols = ["url", "name", "type"].iter().map(|s| s.to_string()).collect(); }
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("TOTAL: {}", media_count)).color(design::ACCENT_GREEN).monospace());
                });
            });
        });

        ui.add_space(6.0);

        // --- FILTERING & SORTING LOGIC ---
        let mut filtered_assets: Vec<crate::state::MediaAsset> = media_assets.into_iter().filter(|asset| {
            let search = media_search.to_lowercase();
            if !search.is_empty() && !asset.url.to_lowercase().contains(&search) && !asset.name.to_lowercase().contains(&search) { return false; }
            
            if min_size_kb > 0 && asset.size_bytes < min_size_kb * 1024 { return false; }

            if !type_filter.is_empty() {
                let mt = asset.mime_type.to_lowercase();
                let lu = asset.url.to_lowercase();
                let mut match_type = false;
                if type_filter.contains("IMAGE") && (mt.contains("image") || asset.url.ends_with(".svg")) { match_type = true; }
                if type_filter.contains("VIDEO") && (mt.contains("video") || mt.contains("mpegurl") || mt.contains("dash+xml") || lu.ends_with(".m3u8") || lu.ends_with(".ts") || lu.ends_with(".mpd") || lu.ends_with(".m4s")) { match_type = true; }
                if type_filter.contains("AUDIO") && (mt.contains("audio") || asset.url.ends_with(".mp3")) { match_type = true; }
                if type_filter.contains("STYLES") && (mt.contains("style") || asset.url.ends_with(".css")) { match_type = true; }
                if type_filter.contains("SCRIPTS") && (mt.contains("script") || asset.url.ends_with(".js")) { match_type = true; }
                if type_filter.contains("FONTS") && mt.contains("font") { match_type = true; }
                if !match_type { return false; }
            }
            true
        }).collect();

        // SORTING
        filtered_assets.sort_by(|a, b| {
            let res = match sort_col.as_str() {
                "name" => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                "type" => a.mime_type.cmp(&b.mime_type),
                "size" => a.size_bytes.cmp(&b.size_bytes),
                _ => a.name.cmp(&b.name),
            };
            if sort_asc { res } else { res.reverse() }
        });

        // --- ASSET GRID (RESPONSIVE) ---
        // KOD NOTU: Grid artık tamamen responsive. Sütun sayısı panel genişliğine göre dinamik hesaplanır.
        let scroll_h = ui.available_height();
        egui::ScrollArea::vertical()
            .max_height(scroll_h)
            .id_salt(format!("{}_media_scroll", tid))
            .show(ui, |ui| {
                if filtered_assets.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label(RichText::new("NO ASSETS MATCH CURRENT FILTERS").italics().color(Color32::GRAY));
                    });
                } else {
                    let spacing = 8.0;
                    let card_w = (preview_size * 1.5).clamp(140.0, 320.0);
                    let cols = (ui.available_width() / (card_w + spacing)).floor().max(1.0) as usize;
                    
                    egui::Grid::new(format!("{}_media_grid", tid))
                        .num_columns(cols)
                        .spacing([spacing, spacing])
                        .show(ui, |ui| {
                            let mut count = 0;
                            for asset in filtered_assets {
                                ui.vertical(|ui| {
                                    let mut is_selected = selected_media_urls.contains(&asset.url);
                                    let border_col = if is_selected { design::ACCENT_GREEN } else { Color32::from_gray(60) };
                                    
                                    Frame::group(ui.style())
                                        .fill(design::BG_ELEVATED)
                                        .stroke(Stroke::new(if is_selected { 2.0 } else { 1.0 }, border_col))
                                        .inner_margin(6.0)
                                        .corner_radius(8.0)
                                        .show(ui, |ui| {
                                            ui.set_width(card_w);
                                            
                                            // 1. Selector + Type
                                            ui.horizontal(|ui| {
                                                if ui.checkbox(&mut is_selected, "").changed() {
                                                    if let Some(ws) = state.workspaces.get_mut(tid) {
                                                        if is_selected { ws.selected_media_urls.insert(asset.url.clone()); }
                                                        else { ws.selected_media_urls.remove(&asset.url); }
                                                    }
                                                }
                                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                    ui.label(RichText::new(asset.mime_type.split('/').next().unwrap_or("file")).small().color(design::TEXT_MUTED));
                                                });
                                            });

                                            // 2. Preview
                                            ui.add_space(4.0);
                                            let img_h = card_w * 0.65;
                                            Frame::new().fill(design::BG_SURFACE).corner_radius(4.0).show(ui, |ui| {
                                                ui.set_min_size(egui::vec2(card_w - 12.0, img_h));
                                                ui.centered_and_justified(|ui| {
                                                    if asset.mime_type.starts_with("image/") {
                                                        if let Some(data) = &asset.data {
                                                            ui.add(egui::Image::from_bytes(format!("bytes://{}", asset.url), data.clone())
                                                                .max_size(egui::vec2(card_w - 12.0, img_h)));
                                                        } else {
                                                            ui.label(RichText::new("📷").size(24.0));
                                                        }
                                                    } else if let Some(thumb_data) = &asset.thumbnail {
                                                        ui.add(egui::Image::from_bytes(format!("bytes://thumb_{}", asset.url), thumb_data.clone())
                                                            .max_size(egui::vec2(card_w - 12.0, img_h)));
                                                    } else {
                                                        let icon = if asset.mime_type.contains("video") || asset.url.contains(".m3u8") { "🎬" }
                                                                  else if asset.mime_type.contains("audio") { "🎵" }
                                                                  else if asset.mime_type.contains("style") { "🎨" }
                                                                  else if asset.mime_type.contains("script") { "📜" }
                                                                  else { "📄" };
                                                        ui.label(RichText::new(icon).size(32.0));
                                                    }
                                                });
                                            });

                                            // 3. Details
                                            ui.add_space(4.0);
                                            ui.add(egui::Label::new(RichText::new(&asset.name).strong().size(11.0)).truncate());
                                            ui.add(egui::Label::new(RichText::new(&asset.url).size(9.0).color(design::TEXT_MUTED)).truncate());
                                            
                                            ui.add_space(4.0);
                                            ui.horizontal(|ui| {
                                                ui.label(RichText::new(format!("{:.1} KB", asset.size_bytes as f64 / 1024.0)).small());
                                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                    if ui.button(RichText::new("SAVE").small()).clicked() {
                                                        if let Some(data) = &asset.data {
                                                            if let Some(path) = rfd::FileDialog::new().set_file_name(&asset.name).save_file() {
                                                                let _ = std::fs::write(path, data);
                                                            }
                                                        }
                                                    }
                                                    // HLS DL
                                                    if crate::core::video_downloader::is_hls_url(&asset.url) {
                                                        if ui.add(egui::Button::new(RichText::new("HLS").small().color(design::ACCENT_GREEN))).clicked() {
                                                            emit(AppEvent::RequestVideoDownload(tid.to_string(), asset.url.clone(), asset.name.clone()));
                                                        }
                                                    }
                                                });
                                            });
                                        });
                                });
                                
                                count += 1;
                                if count % cols == 0 { ui.end_row(); }
                            }
                        });
                }
            });
    });

    // Update state back
    if let Some(ws) = state.workspaces.get_mut(tid) {
        ws.media_search = media_search;
        ws.media_type_filter = type_filter;
        ws.media_preview_size = preview_size;
        ws.media_sort_col = sort_col;
        ws.media_sort_asc = sort_asc;
        ws.media_min_size_kb = min_size_kb;
    }
}
