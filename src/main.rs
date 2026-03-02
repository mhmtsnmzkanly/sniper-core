mod app;
mod state;
mod ui;
mod logger;
mod core;
mod config;

use app::CrawlerApp;
use crate::core::events::AppEvent;
use state::AppState;
use config::loader::load_config;
use clap::Parser;
use eframe::egui;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long, default_value_t = false)]
    cli: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    let _args = Args::parse();
    
    let (log_sender, log_receiver) = tokio::sync::mpsc::unbounded_channel();
    let (event_sender, event_receiver) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
    
    let (_log_guard, timestamp) = logger::init_logging(log_sender);

    let config = match load_config() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("[CONFIG <-> LOAD] Failed: {}", e);
            return Err(e.into());
        }
    };

    let state = AppState::new(config, timestamp);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 850.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    tracing::info!("[SYSTEM <-> INIT] Sniper Scraper Studio {} starting...", env!("CARGO_PKG_VERSION"));
    
    crate::ui::scrape::set_event_sender(event_sender);

    eframe::run_native(
        "Sniper Studio 1.1.0",
        native_options,
        Box::new(|cc| {
            // --- UNIVERSAL OS FONT SUPPORT ---
            let mut fonts = egui::FontDefinitions::default();
            
            // Priority list for Universal Unicode & Asian Language Support
            let system_fonts = [
                // Linux (Noto is standard for full coverage)
                "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/noto/NotoSansCJK-Regular.ttc",
                // Windows (Standard UI fonts with Asian support)
                "C:\\Windows\\Fonts\\msyh.ttc", // Microsoft YaHei
                "C:\\Windows\\Fonts\\malgun.ttf", // Malgun Gothic (Korean)
                "C:\\Windows\\Fonts\\seguiemj.ttf", // Segoe UI Emoji
                // macOS (Standard CJK support)
                "/System/Library/Fonts/PingFang.ttc",
                "/System/Library/Fonts/STHeiti Light.ttc",
            ];

            for path in system_fonts {
                if let Ok(font_bytes) = std::fs::read(path) {
                    fonts.font_data.insert(
                        "univ_font".to_owned(),
                        egui::FontData::from_owned(font_bytes).into(),
                    );
                    fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap()
                        .insert(0, "univ_font".to_owned());
                    fonts.families.get_mut(&egui::FontFamily::Monospace).unwrap()
                        .push("univ_font".to_owned());
                    tracing::info!("[SYSTEM <-> FONT] Loaded native support: {}", path);
                    break;
                }
            }

            cc.egui_ctx.set_fonts(fonts);
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(CrawlerApp::new(cc, state, log_receiver, event_receiver)))
        }),
    ).map_err(|e| format!("GUI Error: {}", e))?;

    Ok(())
}
