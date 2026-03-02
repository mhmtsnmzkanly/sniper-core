use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::process::{Stdio, Child};
use chromiumoxide::Browser;
use crate::state::ChromeTabInfo;
use futures::StreamExt;
use tracing::{info, error};

pub struct BrowserManager;

impl BrowserManager {
    /// İşletim sistemine göre Chromium/Chrome yolunu bulur
    pub fn find_executable() -> Result<PathBuf> {
        #[cfg(target_os = "linux")]
        {
            let paths = ["/usr/bin/chromium", "/usr/bin/google-chrome", "/usr/bin/chrome"];
            for p in paths {
                let path = PathBuf::from(p);
                if path.exists() { return Ok(path); }
            }
        }

        #[cfg(target_os = "macos")]
        {
            let paths = [
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
                "/Applications/Chromium.app/Contents/MacOS/Chromium",
            ];
            for p in paths {
                let path = PathBuf::from(p);
                if path.exists() { return Ok(path); }
            }
        }

        #[cfg(target_os = "windows")]
        {
            let paths = [
                "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
                "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
            ];
            for p in paths {
                let path = PathBuf::from(p);
                if path.exists() { return Ok(path); }
            }
        }

        Err(anyhow!("Chromium executable not found on this system."))
    }

    /// Tarayıcıyı başlatır
    pub async fn launch(url: &str, profile: PathBuf, port: u16, timestamp: String) -> Result<Child> {
        let exec_path = Self::find_executable()?;
        let chrome_log_file = std::fs::File::create(format!("logs/chrome.{}.log", timestamp))?;

        let mut cmd = std::process::Command::new(exec_path);
        cmd.arg("--no-sandbox")
            .arg(format!("--remote-debugging-port={}", port))
            .arg("--remote-allow-origins=*")
            .arg("--disable-features=OptimizationGuideModelDownloading,OnDeviceModel")
            .arg("--no-first-run")
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg(url)
            .stdout(Stdio::from(chrome_log_file.try_clone()?))
            .stderr(Stdio::from(chrome_log_file));

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }

        let child = cmd.spawn().map_err(|e| anyhow!("Failed to spawn browser: {}", e))?;
        Ok(child)
    }

    /// Porttaki sekmeleri listeler
    pub async fn list_tabs(port: u16) -> Result<Vec<ChromeTabInfo>> {
        let client = rquest::Client::builder()
            .timeout(std::time::Duration::from_millis(800))
            .build()?;
        
        let url = format!("http://127.0.0.1:{}/json/list", port);
        let tabs: Vec<ChromeTabInfo> = client.get(url).send().await?.json().await?;

        Ok(tabs.into_iter()
            .filter(|t| t.tab_type == "page" && !t.url.is_empty() && t.url != "about:blank")
            .collect())
    }

    /// Belirli bir sekmeden HTML içeriğini çeker (Mirror modu destekli)
    pub async fn capture_html(port: u16, tab_id: String, save_root: PathBuf, mirror_mode: bool) -> Result<PathBuf> {
        let ws_url = Self::get_ws_url(port).await?;
        let (browser, mut handler) = Browser::connect(ws_url).await?;
        tokio::spawn(async move { while let Some(_) = handler.next().await {} });

        let pages = browser.pages().await?;
        let page = pages.into_iter()
            .find(|p| p.target_id().as_ref() == tab_id)
            .ok_or(anyhow!("Tab not found"))?;

        let url_str = page.url().await?.unwrap_or_default();
        let html = page.content().await?;
        
        let parsed_url = url::Url::parse(&url_str)?;
        let domain = parsed_url.host_str().unwrap_or("unknown");
        let base_dir = save_root.join(domain);
        std::fs::create_dir_all(&base_dir)?;

        let filename = parsed_url.path().trim_matches('/').replace('/', ".");
        let filename = if filename.is_empty() { "index.html".to_string() } else { format!("{}.html", filename) };
        let final_path = base_dir.join(filename);

        // --- FAZ 2: ASSET DISCOVERY & MIRROR ---
        if mirror_mode {
            info!("Faz 2: Mirror Mode Active. Discovering assets...");
            let assets = crate::core::dom::DomProcessor::discover_assets(&html, &url_str);
            info!("Found {} unique assets to download.", assets.len());

            for asset in assets {
                if let Ok(asset_url) = url::Url::parse(&asset.url) {
                    let mut asset_rel_path = asset_url.path().trim_matches('/').to_string();
                    if asset_rel_path.is_empty() { continue; }
                    
                    let asset_target = base_dir.join(&asset_rel_path);
                    let _ = crate::core::downloader::Downloader::download_asset(&asset.url, asset_target).await;
                }
            }
        }

        /// Belirli bir sekmede JavaScript çalıştırır
        pub async fn execute_script(port: u16, tab_id: String, script: String) -> Result<String> {
            let ws_url = Self::get_ws_url(port).await?;
            let (browser, mut handler) = Browser::connect(ws_url).await?;
            tokio::spawn(async move { while let Some(_) = handler.next().await {} });

            let pages = browser.pages().await?;
            let page = pages.into_iter()
                .find(|p| p.target_id().as_ref() == tab_id)
                .ok_or(anyhow!("Tab not found"))?;

            let result = page.evaluate(script).await?;

            /// Sekmenin ağ trafiğini dinlemeye başlar
            pub async fn enable_network_monitoring(port: u16, tab_id: String) -> Result<()> {
                use chromiumoxide::cdp::browser_protocol::network::Event;

                let ws_url = Self::get_ws_url(port).await?;
                let (browser, mut handler) = Browser::connect(ws_url).await?;

                // Önce sekmeyi bul
                let pages = browser.pages().await?;
                let page = pages.into_iter()
                    .find(|p| p.target_id().as_ref() == tab_id)
                    .ok_or(anyhow!("Tab not found"))?;

                // Ağ izlemeyi aç
                page.execute(chromiumoxide::cdp::browser_protocol::network::EnableParams::default()).await?;

                // Olayları dinle (Ayrı bir görevde)
                let mut events = page.event_listener::<Event>().await?;

                tokio::spawn(async move {
                    tokio::select! {
                        _ = async {
                            while let Some(event) = events.next().await {
                                match event {
                                    Event::RequestWillBeSent(e) => {
                                        let req = crate::state::NetworkRequest {
                                            request_id: e.request_id.to_string(),
                                            url: e.request.url.clone(),
                                            method: e.request.method.clone(),
                                            resource_type: format!("{:?}", e.r#type),
                                            status: None,
                                            timestamp: e.timestamp.clone().into(),
                                        };
                                        crate::ui::scrape::emit(crate::core::events::AppEvent::NetworkRequestSent(req));
                                    }
                                    Event::ResponseReceived(e) => {
                                        crate::ui::scrape::emit(crate::core::events::AppEvent::NetworkResponseReceived(
                                            e.request_id.to_string(),
                                            e.response.status as u16
                                        ));
                                    }
                                    _ => {}
                                }
                            }
                        } => {}
                        _ = async {
                            while let Some(_) = handler.next().await {}
                        } => {}
                    }
                });

                Ok(())
            }
            }
