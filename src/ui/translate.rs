use crate::state::AppState;
use crate::ui::design;
use egui::{Color32, Frame, RichText, Ui};

pub fn render(ui: &mut Ui, state: &mut AppState) {
    design::title(ui, "AI Translation Studio", design::ACCENT_CYAN);
    ui.label(RichText::new("Automated HTML translation using Gemini AI model").small().color(design::TEXT_MUTED));
    ui.add_space(10.0);

    let panel_stroke = egui::Stroke::new(1.0, Color32::from_rgb(42, 64, 78));

    let avail_w = ui.available_width();
    let is_wide = avail_w > 600.0;
    let (left_w, right_w) = if is_wide {
        let lw = (avail_w * 0.45).clamp(280.0, 450.0);
        (lw, avail_w - lw - ui.spacing().item_spacing.x - 2.0)
    } else {
        (avail_w, avail_w)
    };

    let render_config = |ui: &mut Ui, state: &mut AppState, w: f32| {
        ui.vertical(|ui| {
            ui.set_width(w);
            Frame::group(ui.style()).fill(design::BG_SURFACE).stroke(panel_stroke).inner_margin(10.0).corner_radius(8.0).show(ui, |ui| {
                ui.label(RichText::new("Workflow Configuration").strong().color(design::ACCENT_ORANGE));
                ui.add_space(8.0);

                ui.label(RichText::new("Target Folder:").color(design::TEXT_MUTED));
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(RichText::new(state.config.output_dir.to_string_lossy()).small().monospace()).truncate());
                    if ui.button("Browse").clicked() { if let Some(path) = rfd::FileDialog::new().pick_folder() { state.config.output_dir = path; } }
                });
                ui.add_space(4.0);
                ui.label(RichText::new("Translating all .html files in folder.").small().italics().color(design::TEXT_MUTED));

                ui.separator(); ui.add_space(4.0);
                ui.label(RichText::new("Translation Profile:").strong().color(design::TEXT_MUTED));
                egui::Grid::new("translate_profile_grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                    ui.label("Source:"); ui.label(RichText::new("Auto-detect").color(design::ACCENT_CYAN)); ui.end_row();
                    ui.label("Target:"); ui.label(RichText::new("Turkish (TR)").color(design::ACCENT_GREEN)); ui.end_row();
                    ui.label("Model:"); ui.label("Gemini-2.0-Flash"); ui.end_row();
                });

                ui.add_space(16.0);
                let can_start = !state.is_translating && !state.config.gemini_api_key.is_empty();
                let btn_text = if state.is_translating { "RUNNING..." } else { "🚀 START TRANSLATION" };
                if ui.add_enabled(can_start, egui::Button::new(RichText::new(btn_text).strong()).min_size([ui.available_width(), 44.0].into())).clicked() {
                    state.is_translating = true;
                    state.notify(crate::state::NotificationLevel::Info, "Translator", "Translation pipeline started.");
                }
                if state.config.gemini_api_key.is_empty() {
                    ui.add_space(4.0);
                    ui.colored_label(Color32::from_rgb(255, 100, 100), "⚠ API Key missing!");
                }
            });
        });
    };

    let render_timeline = |ui: &mut Ui, state: &mut AppState, w: f32| {
        ui.vertical(|ui| {
            ui.set_width(w);
            Frame::group(ui.style()).fill(design::BG_SURFACE).stroke(panel_stroke).inner_margin(10.0).corner_radius(8.0).show(ui, |ui| {
                ui.label(RichText::new("Execution Timeline").strong().color(design::ACCENT_GREEN));
                ui.add_space(8.0);

                if state.is_translating {
                    ui.horizontal(|ui| { ui.add(egui::Spinner::new().size(16.0)); ui.label("API processing..."); });
                    ui.add_space(8.0);
                    ui.add(egui::ProgressBar::new(0.35).show_percentage());
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        ui.label(RichText::new("IDLE").size(18.0).color(Color32::from_gray(80)));
                        ui.label(RichText::new("Ready to begin translation pipeline.").small().color(design::TEXT_MUTED));
                        ui.add_space(20.0);
                    });
                }

                ui.separator();
                ui.label(RichText::new("Batch Report Log:").small().color(design::TEXT_MUTED));
                egui::ScrollArea::vertical().max_height(300.0).auto_shrink([false, true]).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
                    ui.label(RichText::new("No recent logs found.").small().color(Color32::from_gray(100)));
                });
            });
        });
    };

    if is_wide {
        ui.horizontal_top(|ui| {
            render_config(ui, state, left_w);
            render_timeline(ui, state, right_w);
        });
    } else {
        ui.vertical(|ui| {
            render_config(ui, state, left_w);
            ui.add_space(8.0);
            render_timeline(ui, state, right_w);
        });
    }
}
