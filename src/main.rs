mod app;
mod state;
mod ui;
mod logger;
mod backend;

use app::CrawlerApp;
use state::AppState;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    url: Option<String>,

    #[arg(long, default_value_t = false)]
    cli: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    let args = Args::parse();
    
    // 1. Kanalları Hazırla
    let (log_sender, log_receiver) = tokio::sync::mpsc::unbounded_channel();
    let (event_sender, event_receiver) = tokio::sync::mpsc::unbounded_channel();
    
    // 2. Loglama Sistemini Başlat
    let (_log_guard, timestamp) = logger::init_logging(log_sender);

    // 3. Uygulama Durumu
    let state = AppState::new(timestamp);

    if args.cli {
        tracing::info!("CLI Mode is not maintained in Sniper 3.0. Use GUI.");
        return Ok(());
    }

    // 4. GUI'yi Başlat
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([950.0, 750.0])
            .with_min_inner_size([700.0, 600.0]),
        ..Default::default()
    };

    tracing::info!("🚀 Sniper Scraper 3.0 Başlatılıyor...");
    
    // Event sender'ı her yere ulaştırmak için Lazy Static veya basitçe kopyalama (Arc) kullanılabilir.
    // Şimdilik UI ve Backend arasındaki köprüyü kuruyoruz.
    crate::ui::scrape::set_event_sender(event_sender);

    eframe::run_native(
        "Sniper Scraper 3.0",
        native_options,
        Box::new(|cc| {
            let mut fonts = egui::FontDefinitions::default();
            
            // 1. ROBOTO-LIGHT (Binary içine gömülü)
            fonts.font_data.insert(
                "roboto_light".to_owned(),
                egui::FontData::from_static(include_bytes!("../static/roboto/Roboto-Light.ttf")).into(),
            );

            // 2. KORECE FALLBACK (Sistemden dinamik)
            let font_paths = [
                "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/adobe-source-han-sans/SourceHanSansK-Regular.otf",
            ];

            let mut korean_loaded = false;
            for path in font_paths {
                if let Ok(font_bytes) = std::fs::read(path) {
                    fonts.font_data.insert(
                        "korean_font".to_owned(),
                        egui::FontData::from_owned(font_bytes).into(),
                    );
                    korean_loaded = true;
                    break;
                }
            }

            // ÖNCELİK SIRALAMASI: Önce Roboto, sonra Korece
            let families = [
                (egui::FontFamily::Proportional, vec!["roboto_light".to_owned()]),
                (egui::FontFamily::Monospace, vec!["roboto_light".to_owned()]),
            ];

            for (family, mut list) in families {
                if korean_loaded {
                    list.push("korean_font".to_owned());
                }
                fonts.families.insert(family, list);
            }

            if !korean_loaded {
                tracing::warn!("Korece destekli sistem fontu bulunamadı. Korece karakterler bozuk görünebilir.");
            }

            cc.egui_ctx.set_fonts(fonts);

            Ok(Box::new(CrawlerApp::new(cc, state, log_receiver, event_receiver)))
        }),
    ).map_err(|e| anyhow::anyhow!("GUI Error: {}", e))?;

    Ok(())
}
