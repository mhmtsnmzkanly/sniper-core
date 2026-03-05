mod app;
mod state;
mod ui;
mod logger;
mod core;

use app::CrawlerApp;
use state::{AppState, AppConfig};
use clap::Parser;
use eframe::egui;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "9222")]
    port: u16,
}

/// Automatically detects the location of the Chrome/Chromium binary based on the OS.
fn find_chrome_binary() -> String {
    if cfg!(target_os = "windows") {
        let paths = [
            "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
            "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
        ];
        for p in paths { if std::path::PathBuf::from(p).exists() { return p.to_string(); } }
    } else if cfg!(target_os = "macos") {
        let paths = [
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Users/Shared/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        ];
        for p in paths { if std::path::PathBuf::from(p).exists() { return p.to_string(); } }
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

/// Automatically detects the default Chrome profile path based on the OS.
/// Note: We point to the parent directory of 'Default' so Chrome can manage profile selection.
fn find_chrome_profile() -> String {
    let p = if cfg!(target_os = "windows") {
        // Windows: User Data is usually in Local AppData
        dirs::data_local_dir().map(|d| d.join("Google\\Chrome\\User Data"))
    } else if cfg!(target_os = "macos") {
        // macOS: Application Support/Google/Chrome
        dirs::home_dir().map(|d| d.join("Library/Application Support/Google/Chrome"))
    } else {
        // Linux: .config/google-chrome
        dirs::home_dir().map(|d| d.join(".config/google-chrome"))
    };
    
    p.map(|path| path.to_string_lossy().to_string()).unwrap_or_default()
}

/// Main Entry Point: Initializes logging, state, and the native UI window.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    // Initialize application configuration with auto-detected values.
    let mut config = AppConfig::default();
    config.remote_debug_port = args.port;
    config.chrome_binary_path = find_chrome_binary();
    config.chrome_profile_path = find_chrome_profile();

    // Create async communication channels.
    let (log_sender, log_receiver) = tokio::sync::mpsc::unbounded_channel();
    let (event_sender, event_receiver) = tokio::sync::mpsc::unbounded_channel();
    
    // Setup logging and event routing.
    let session_ts = logger::init_logging(log_sender);
    ui::scrape::set_event_sender(event_sender);

    let state = AppState::new(config, session_ts);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([1000.0, 700.0])
            .with_maximized(true),
        ..Default::default()
    };

    // Run the native UI loop.
    eframe::run_native(
        "Sniper Studio",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(CrawlerApp::new(cc, state, log_receiver, event_receiver)))
        }),
    ).map_err(|e| format!("GUI Error: {}", e))?;

    Ok(())
}
