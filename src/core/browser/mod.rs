use anyhow::{anyhow, Result};
use tracing::info;
use std::path::PathBuf;
use std::process::{Stdio, Child};
use chromiumoxide::Browser;
use crate::state::ChromeTabInfo;
use futures::StreamExt;

pub struct BrowserManager;

impl BrowserManager {
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
            let paths = ["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome", "/Applications/Chromium.app/Contents/MacOS/Chromium"];
            for p in paths {
                let path = PathBuf::from(p);
                if path.exists() { return Ok(path); }
            }
        }
        #[cfg(target_os = "windows")]
        {
            let paths = ["C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe", "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe"];
            for p in paths {
                let path = PathBuf::from(p);
                if path.exists() { return Ok(path); }
            }
        }
        Err(anyhow!("Chromium executable not found."))
    }

    pub async fn launch(url: &str, profile: PathBuf, port: u16, timestamp: String) -> Result<Child> {
        let exec_path = Self::find_executable()?;
        let _ = std::fs::create_dir_all("logs");
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

        Ok(cmd.spawn()?)
    }

    pub async fn list_tabs(port: u16) -> Result<Vec<ChromeTabInfo>> {
        let client = rquest::Client::builder().timeout(std::time::Duration::from_millis(800)).build()?;
        let url = format!("http://127.0.0.1:{}/json/list", port);
        let tabs: Vec<ChromeTabInfo> = client.get(url).send().await?.json().await?;
        Ok(tabs.into_iter().filter(|t| t.tab_type == "page" && !t.url.is_empty()).collect())
    }

    pub async fn get_ws_url(port: u16) -> Result<String> {
        let client = rquest::Client::new();
        let json: serde_json::Value = client.get(format!("http://127.0.0.1:{}/json/version", port)).send().await?.json().await?;
        Ok(json["webSocketDebuggerUrl"].as_str().ok_or(anyhow!("WS URL missing"))?.to_string())
    }

    pub async fn capture_html(port: u16, tab_id: String, save_root: PathBuf, mirror_mode: bool) -> Result<PathBuf> {
        let ws_url = Self::get_ws_url(port).await?;
        let (browser, mut handler) = Browser::connect(ws_url).await?;
        tokio::spawn(async move { while let Some(_) = handler.next().await {} });

        let pages = browser.pages().await?;
        let page = pages.into_iter().find(|p| p.target_id().as_ref() == tab_id).ok_or(anyhow!("Tab not found"))?;

        let url_str = page.url().await?.unwrap_or_default();
        let html = page.content().await?;
        
        if mirror_mode {
            let assets = crate::core::dom::DomProcessor::discover_assets(&html, &url_str);
            let parsed_url = url::Url::parse(&url_str)?;
            let domain = parsed_url.host_str().unwrap_or("unknown");
            let base_dir = save_root.join(domain);
            
            for asset in assets {
                if let Ok(asset_url) = url::Url::parse(&asset.url) {
                    let path = asset_url.path().trim_matches('/');
                    if !path.is_empty() {
                        let _ = crate::core::downloader::Downloader::download_asset(&asset.url, base_dir.join(path)).await;
                    }
                }
            }
        }

        let parsed_url = url::Url::parse(&url_str)?;
        let domain = parsed_url.host_str().unwrap_or("unknown");
        let dir = save_root.join(domain);
        std::fs::create_dir_all(&dir)?;
        let filename = parsed_url.path().trim_matches('/').replace('/', ".");
        let filename = if filename.is_empty() { "index.html".to_string() } else { format!("{}.html", filename) };
        let final_path = dir.join(filename);
        std::fs::write(&final_path, html.as_bytes())?;
        Ok(final_path)
    }

    pub async fn execute_script(port: u16, tab_id: String, script: String) -> Result<String> {
        let ws_url = Self::get_ws_url(port).await?;
        let (browser, mut handler) = Browser::connect(ws_url).await?;
        tokio::spawn(async move { while let Some(_) = handler.next().await {} });
        let pages = browser.pages().await?;
        let page = pages.into_iter().find(|p| p.target_id().as_ref() == tab_id).ok_or(anyhow!("Tab not found"))?;
        let result = page.evaluate(script).await?;
        Ok(result.value().map(|v| format!("{}", v)).unwrap_or_else(|| "No return value".to_string()))
    }

    pub async fn enable_network_monitoring(port: u16, tab_id: String) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::network::{EventRequestWillBeSent, EventResponseReceived, EnableParams};
        let ws_url = Self::get_ws_url(port).await?;
        let (browser, mut handler) = Browser::connect(ws_url).await?;
        let pages = browser.pages().await?;
        let page = pages.into_iter().find(|p| p.target_id().as_ref() == tab_id).ok_or(anyhow!("Tab not found"))?;
        
        page.execute(EnableParams::default()).await?;
        
        let mut request_events = page.event_listener::<EventRequestWillBeSent>().await?;
        let mut response_events = page.event_listener::<EventResponseReceived>().await?;
        
        tokio::spawn(async move {
            tokio::select! {
                _ = async {
                    while let Some(e) = request_events.next().await {
                        let req = crate::state::NetworkRequest {
                            request_id: e.request_id.as_ref().to_string(),
                            url: e.request.url.clone(),
                            method: e.request.method.clone(),
                            resource_type: format!("{:?}", e.r#type),
                            status: None,
                            timestamp: 0.0,
                        };
                        crate::ui::scrape::emit(crate::core::events::AppEvent::NetworkRequestSent(req));
                    }
                } => {}
                _ = async {
                    while let Some(e) = response_events.next().await {
                        crate::ui::scrape::emit(crate::core::events::AppEvent::NetworkResponseReceived(
                            e.request_id.as_ref().to_string(),
                            e.response.status as u16
                        ));
                    }
                } => {}
                _ = async { while let Some(_) = handler.next().await {} } => {}
            }
        });
        Ok(())
    }

    pub async fn get_cookies(port: u16, tab_id: String) -> Result<Vec<crate::state::ChromeCookie>> {
        let ws_url = Self::get_ws_url(port).await?;
        let (browser, mut handler) = Browser::connect(ws_url).await?;
        tokio::spawn(async move { while let Some(_) = handler.next().await {} });
        let pages = browser.pages().await?;
        let page = pages.into_iter().find(|p| p.target_id().as_ref() == tab_id).ok_or(anyhow!("Tab not found"))?;
        let cookies = page.get_cookies().await?;
        Ok(cookies.into_iter().map(|c| crate::state::ChromeCookie {
            name: c.name.clone(), value: c.value.clone(), domain: c.domain.clone(), path: c.path.clone(),
            expires: c.expires, secure: c.secure, http_only: c.http_only,
        }).collect())
    }

    pub async fn set_emulation(port: u16, tab_id: String, ua: String, lat: f64, lon: f64) -> Result<()> {
        let ws_url = Self::get_ws_url(port).await?;
        let (browser, mut handler) = Browser::connect(ws_url).await?;
        tokio::spawn(async move { while let Some(_) = handler.next().await {} });
        let pages = browser.pages().await?;
        let page = pages.into_iter().find(|p| p.target_id().as_ref() == tab_id).ok_or(anyhow!("Tab not found"))?;
        
        if !ua.is_empty() {
            let params = chromiumoxide::cdp::browser_protocol::network::SetUserAgentOverrideParams::new(ua);
            page.execute(params).await?;
        }
        
        let geo_params = chromiumoxide::cdp::browser_protocol::emulation::SetGeolocationOverrideParams::builder()
            .latitude(lat)
            .longitude(lon)
            .accuracy(1.0)
            .build();
        page.execute(geo_params).await?;
        Ok(())
    }
}
