mod app;
mod state;
mod ui;
mod logger;
mod core;
mod config;

use app::CrawlerApp;
use state::AppState;
use config::loader::load_config;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long, default_value_t = false)]
    cli: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Kanalları Hazırla
    let (log_sender, log_receiver) = tokio::sync::mpsc::unbounded_channel();
    let (event_sender, event_receiver) = tokio::sync::mpsc::unbounded_channel();
    
    // 2. Loglama Sistemini Başlat
    let (_log_guard, timestamp) = logger::init_logging(log_sender);

    // 3. Config Yükle (Versiyonlu & Migrated)
    let config = match load_config() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Config loading failed: {}", e);
            return Err(e);
        }
    };

    // 4. Uygulama Durumu
    let state = AppState::new(config, timestamp);

    // 5. GUI'yi Başlat
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    tracing::info!("🚀 Sniper Scraper Studio {} - FAZ 1 Başlatılıyor...", env!("CARGO_PKG_VERSION"));
    
    // Event sender'ı statik olarak veya UI üzerinden ulaştırmak için altyapı
    crate::ui::scrape::set_event_sender(event_sender);

    eframe::run_native(
        "Sniper Scraper Studio",
        native_options,
        Box::new(|cc| {
            // Font ve stil ayarları burada yapılabilir
            Ok(Box::new(CrawlerApp::new(cc, state, log_receiver, event_receiver)))
        }),
    ).map_err(|e| anyhow::anyhow!("GUI Error: {}", e))?;

    Ok(())
}
