mod app;
mod state;
mod ui;
mod logger;
mod core;

use app::CrawlerApp;
use state::{AppState, AppConfig};
use clap::Parser;
use eframe::egui;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "9222")]
    port: u16,
}

fn find_chrome_binary() -> String {
    if cfg!(target_os = "windows") {
        let paths = [
            "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
            "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
        ];
        for p in paths { if PathBuf::from(p).exists() { return p.to_string(); } }
    } else if cfg!(target_os = "macos") {
        let paths = [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Users/Shared/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        ];
        for p in paths { if PathBuf::from(p).exists() { return p.to_string(); } }
    } else {
        let fallbacks = ["google-chrome", "google-chrome-stable", "chromium", "chromium-browser"];
        for bin in fallbacks {
            if std::process::Command::new("which").arg(bin).output().map(|o| o.status.success()).unwrap_or(false) {
                return bin.to_string();
            }
        }
    }
    "google-chrome".to_string()
}

fn find_chrome_profile() -> String {
    let p = if cfg!(target_os = "windows") {
        dirs::cache_dir().map(|d| d.join("Google\\Chrome\\User Data\\Default"))
    } else if cfg!(target_os = "macos") {
        dirs::home_dir().map(|d| d.join("Library/Application Support/Google/Chrome/Default"))
    } else {
        dirs::home_dir().map(|d| d.join(".config/google-chrome/Default"))
    };
    
    p.map(|path| path.to_string_lossy().to_string()).unwrap_or_default()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    let mut config = AppConfig::default();
    config.remote_debug_port = args.port;
    config.chrome_binary_path = find_chrome_binary();
    config.chrome_profile_path = find_chrome_profile();

    let (log_sender, log_receiver) = tokio::sync::mpsc::unbounded_channel();
    let (event_sender, event_receiver) = tokio::sync::mpsc::unbounded_channel();
    
    let session_ts = logger::init_logging(log_sender);
    ui::scrape::set_event_sender(event_sender);

    let state = AppState::new(config, session_ts);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([1000.0, 700.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Sniper Studio",
        options,
        Box::new(|cc| {
            // Install image loaders for egui_extras
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(CrawlerApp::new(cc, state, log_receiver, event_receiver)))
        }),
    ).map_err(|e| format!("GUI Error: {}", e))?;

    Ok(())
}
