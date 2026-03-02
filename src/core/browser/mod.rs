use crate::core::error::{AppError, AppResult};
use std::path::PathBuf;
use std::process::{Stdio, Child};
use chromiumoxide::Browser;
use crate::state::{ChromeTabInfo, ChromeCookie};
use futures::StreamExt;
use tracing::{debug, error};

pub struct BrowserManager;

impl BrowserManager {
    pub fn find_executable() -> AppResult<PathBuf> {
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
        Err(AppError::Browser("Chromium executable not found.".into()))
    }

    pub async fn launch(url: &str, profile: PathBuf, port: u16, timestamp: String) -> AppResult<Child> {
        let exec_path = Self::find_executable()?;
        let _ = std::fs::create_dir_all("logs");
        let chrome_log_file = std::fs::File::create(format!("logs/chrome.{}.log", timestamp))
            .map_err(AppError::Io)?;

        let mut cmd = std::process::Command::new(exec_path);
        cmd.arg("--no-sandbox")
            .arg(format!("--remote-debugging-port={}", port))
            .arg("--remote-allow-origins=*")
            .arg("--disable-features=OptimizationGuideModelDownloading,OnDeviceModel")
            .arg("--no-first-run")
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg(url)
            .stdout(Stdio::from(chrome_log_file.try_clone().map_err(AppError::Io)?))
            .stderr(Stdio::from(chrome_log_file));

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }

        cmd.spawn().map_err(AppError::Io)
    }

    pub async fn list_tabs(port: u16) -> AppResult<Vec<ChromeTabInfo>> {
        let client = rquest::Client::builder()
            .timeout(std::time::Duration::from_millis(1500))
            .build()?;
            
        let url = format!("http://127.0.0.1:{}/json/list", port);
        let resp = client.get(url).send().await
            .map_err(|e| AppError::Network(format!("Failed to connect to browser: {}", e)))?;
            
        let tabs: Vec<ChromeTabInfo> = resp.json().await?;
        Ok(tabs.into_iter().filter(|t| t.tab_type == "page" && !t.url.is_empty()).collect())
    }

    pub async fn get_ws_url(port: u16) -> AppResult<String> {
        let client = rquest::Client::new();
        let json: serde_json::Value = client.get(format!("http://127.0.0.1:{}/json/version", port))
            .send()
            .await?
            .json()
            .await?;

        Ok(json["webSocketDebuggerUrl"]
            .as_str()
            .ok_or_else(|| AppError::NotFound("WS URL missing".into()))?
            .to_string())
    }

    pub async fn capture_html(port: u16, tab_id: String, save_root: PathBuf, mirror_mode: bool) -> AppResult<PathBuf> {
        let ws_url = Self::get_ws_url(port).await?;
        let (browser, mut handler) = Browser::connect(ws_url).await
            .map_err(|e| AppError::Browser(format!("WS Connection failed: {}", e)))?;
            
        tokio::spawn(async move { while let Some(_) = handler.next().await {} });
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let pages = browser.pages().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let page = pages.into_iter()
            .find(|p| p.target_id().as_ref() == tab_id)
            .ok_or_else(|| AppError::NotFound(format!("Tab {} not found", tab_id)))?;

        let url_str = page.url().await.map_err(|e| AppError::Browser(e.to_string()))?.unwrap_or_default();
        let html = page.content().await.map_err(|e| AppError::Browser(e.to_string()))?;
        
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
        std::fs::create_dir_all(&dir).map_err(AppError::Io)?;
        let filename = parsed_url.path().trim_matches('/').replace('/', ".");
        let filename = if filename.is_empty() { "index.html".to_string() } else { format!("{}.html", filename) };
        let final_path = dir.join(filename);
        std::fs::write(&final_path, html.as_bytes()).map_err(AppError::Io)?;
        Ok(final_path)
    }

    pub async fn execute_script(port: u16, tab_id: String, script: String) -> AppResult<String> {
        let ws_url = Self::get_ws_url(port).await?;
        let (browser, mut handler) = Browser::connect(ws_url).await.map_err(|e| AppError::Browser(e.to_string()))?;
        tokio::spawn(async move { while let Some(_) = handler.next().await {} });
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let pages = browser.pages().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let page = pages.into_iter().find(|p| p.target_id().as_ref() == tab_id).ok_or_else(|| AppError::NotFound("Tab not found".into()))?;
        let result = page.evaluate(script).await.map_err(|e| AppError::Browser(e.to_string()))?;
        Ok(result.value().map(|v| format!("{}", v)).unwrap_or_else(|| "No return value".to_string()))
    }

    pub async fn setup_tab_listeners(port: u16, tab_id: String) -> AppResult<()> {
        use chromiumoxide::cdp::browser_protocol::network::{EventRequestWillBeSent, EventResponseReceived, EnableParams as NetEnable};
        use chromiumoxide::cdp::js_protocol::runtime::{EventConsoleApiCalled, EnableParams as RuntimeEnable};
        
        let ws_url = Self::get_ws_url(port).await?;
        let (browser, mut handler) = Browser::connect(ws_url).await.map_err(|e| AppError::Browser(e.to_string()))?;
        tokio::spawn(async move { while let Some(_) = handler.next().await {} });
        
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let pages = browser.pages().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let page = pages.into_iter().find(|p| p.target_id().as_ref() == tab_id).ok_or_else(|| AppError::NotFound("Tab not found".into()))?;
        
        // Enable Domains
        page.execute(NetEnable::default()).await.map_err(|e| AppError::Browser(e.to_string()))?;
        page.execute(RuntimeEnable::default()).await.map_err(|e| AppError::Browser(e.to_string()))?;
        
        let mut request_events = page.event_listener::<EventRequestWillBeSent>().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let mut response_events = page.event_listener::<EventResponseReceived>().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let mut console_events = page.event_listener::<EventConsoleApiCalled>().await.map_err(|e| AppError::Browser(e.to_string()))?;
        
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(e) = request_events.next() => {
                        let req = crate::state::NetworkRequest {
                            request_id: e.request_id.as_ref().to_string(),
                            url: e.request.url.clone(),
                            method: e.request.method.clone(),
                            resource_type: format!("{:?}", e.r#type),
                            status: None,
                            request_body: None,
                            response_body: None,
                        };
                        crate::ui::scrape::emit(crate::core::events::AppEvent::NetworkRequestSent(req));
                    }
                    Some(e) = response_events.next() => {
                        crate::ui::scrape::emit(crate::core::events::AppEvent::NetworkResponseReceived(
                            e.request_id.as_ref().to_string(), 
                            e.response.status as u16,
                            None
                        ));
                    }
                    Some(e) = console_events.next() => {
                        let msg = e.args.iter().map(|v| format!("{:?}", v.value)).collect::<Vec<_>>().join(" ");
                        crate::ui::scrape::emit(crate::core::events::AppEvent::ConsoleLogAdded(msg));
                    }
                    else => break,
                }
            }
        });
        Ok(())
    }

    pub async fn get_cookies(port: u16, tab_id: String) -> AppResult<Vec<crate::state::ChromeCookie>> {
        let ws_url = Self::get_ws_url(port).await?;
        let (browser, mut handler) = Browser::connect(ws_url).await.map_err(|e| AppError::Browser(e.to_string()))?;
        tokio::spawn(async move { while let Some(_) = handler.next().await {} });
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let pages = browser.pages().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let page = pages.into_iter().find(|p| p.target_id().as_ref() == tab_id).ok_or_else(|| AppError::NotFound("Tab not found".into()))?;
        let cookies = page.get_cookies().await.map_err(|e| AppError::Browser(e.to_string()))?;
        Ok(cookies.into_iter().map(|c| crate::state::ChromeCookie {
            name: c.name.clone(), value: c.value.clone(), domain: c.domain.clone(), path: c.path.clone(),
            expires: c.expires, secure: c.secure, http_only: c.http_only,
        }).collect())
    }

    pub async fn manage_cookie(port: u16, tab_id: String, cookie: ChromeCookie, delete: bool) -> AppResult<()> {
        use chromiumoxide::cdp::browser_protocol::network::{SetCookieParams, DeleteCookiesParams};
        let ws_url = Self::get_ws_url(port).await?;
        let (browser, mut handler) = Browser::connect(ws_url).await.map_err(|e| AppError::Browser(e.to_string()))?;
        tokio::spawn(async move { while let Some(_) = handler.next().await {} });
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let pages = browser.pages().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let page = pages.into_iter().find(|p| p.target_id().as_ref() == tab_id).ok_or_else(|| AppError::NotFound("Tab not found".into()))?;
        
        if delete {
            let params = DeleteCookiesParams::builder()
                .name(cookie.name)
                .domain(cookie.domain)
                .build()
                .map_err(|e| AppError::Browser(e))?;
            page.execute(params).await.map_err(|e| AppError::Browser(e.to_string()))?;
        } else {
            let params = SetCookieParams::builder()
                .name(cookie.name)
                .value(cookie.value)
                .domain(cookie.domain)
                .path(cookie.path)
                .secure(cookie.secure)
                .http_only(cookie.http_only)
                .build()
                .map_err(|e| AppError::Browser(e))?;
            page.execute(params).await.map_err(|e| AppError::Browser(e.to_string()))?;
        }
        Ok(())
    }
}
