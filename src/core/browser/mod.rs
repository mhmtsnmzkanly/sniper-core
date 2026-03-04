use crate::core::error::{AppError, AppResult};
use crate::state::{ChromeTabInfo, ChromeCookie, MediaAsset, NetworkRequest};
use chromiumoxide::browser::{Browser};
use chromiumoxide::cdp::browser_protocol::network::{GetResponseBodyParams, SetBlockedUrLsParams, BlockPattern, SetCookieParams, DeleteCookiesParams};
use chromiumoxide::cdp::js_protocol::runtime::{EvaluateParams, EventConsoleApiCalled};
use futures::StreamExt;
use std::path::PathBuf;
use std::time::Duration;
use base64::prelude::*;
use std::collections::HashMap;
use tokio::sync::mpsc;
use crate::core::events::AppEvent;

/// BrowserManager: Tarayıcı yaşam döngüsü ve CDP komutlarının yönetildiği ana merkez.
/// BUNU SİLME: Bu yapı projenin iskeletidir.
pub struct BrowserManager;

impl BrowserManager {
    /// Tarayıcıyı başlatır ve heartbeat denetimi kurar.
    pub async fn launch(
        url: &str, 
        profile_path: PathBuf, 
        port: u16, 
        _log_dir: PathBuf, 
        _session_ts: String,
        tx: mpsc::UnboundedSender<AppEvent>
    ) -> AppResult<std::process::Child> {
        tracing::info!("[CORE -> BROWSER] Tarayıcı başlatılıyor. Port: {}, URL: {}", port, url);

        let chrome_path = if cfg!(target_os = "windows") {
            "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe".to_string()
        } else if cfg!(target_os = "macos") {
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".to_string()
        } else {
            let mut found_path = "google-chrome".to_string();
            let fallbacks = ["google-chrome", "google-chrome-stable", "chromium", "chromium-browser"];
            for bin in fallbacks {
                if std::process::Command::new("which").arg(bin).output().map(|o| o.status.success()).unwrap_or(false) {
                    found_path = bin.to_string();
                    break;
                }
            }
            found_path
        };

        let mut command = std::process::Command::new(&chrome_path);
        command.arg(format!("--remote-debugging-port={}", port))
            .arg(format!("--user-data-dir={}", profile_path.display()))
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--remote-allow-origins=*");

        #[cfg(target_os = "linux")]
        {
            command.arg("--no-sandbox").arg("--disable-setuid-sandbox").arg("--disable-dev-shm-usage");
        }

        let child = command.arg(url).spawn().map_err(|e| {
            tracing::error!("[CORE -> BROWSER] Spawn hatası: {}", e);
            AppError::Io(e)
        })?;

        let tx_clone = tx.clone();
        let port_clone = port;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(5)).await; 
            let hb_url = format!("http://127.0.0.1:{}/json/version", port_clone);
            let client = rquest::Client::new();
            loop {
                match client.get(&hb_url).send().await {
                    Ok(resp) if resp.status().is_success() => {}
                    _ => {
                        tracing::warn!("[BROWSER -> CORE] Heartbeat koptu.");
                        let _ = tx_clone.send(AppEvent::BrowserTerminated);
                        break;
                    }
                }
                tokio::time::sleep(Duration::from_millis(2000)).await;
            }
        });

        Ok(child)
    }

    pub fn get_system_profile_path() -> PathBuf {
        if cfg!(target_os = "windows") {
            PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default())
                .join("Google/Chrome/User Data/Default")
        } else if cfg!(target_os = "macos") {
            PathBuf::from(std::env::var("HOME").unwrap_or_default())
                .join("Library/Application Support/Google/Chrome/Default")
        } else {
            PathBuf::from(std::env::var("HOME").unwrap_or_default())
                .join(".config/google-chrome/Default")
        }
    }

    async fn connect_robust(port: u16) -> AppResult<(Browser, tokio::task::JoinHandle<()>)> {
        let ws_url = Self::get_ws_url(port).await?;
        let (browser, mut handler) = Browser::connect(ws_url).await.map_err(|e| AppError::Browser(e.to_string()))?;
        let handle = tokio::spawn(async move {
            while let Some(res) = handler.next().await {
                if res.is_err() { break; }
            }
        });
        Ok((browser, handle))
    }

    pub async fn get_ws_url(port: u16) -> AppResult<String> {
        let url = format!("http://127.0.0.1:{}/json/version", port);
        let client = rquest::Client::new();
        let resp = client.get(url).send().await.map_err(|e| AppError::Network(e.to_string()))?;
        let json: serde_json::Value = resp.json().await.map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(json["webSocketDebuggerUrl"].as_str().ok_or_else(|| AppError::Internal("No WS URL".into()))?.to_string())
    }

    pub async fn list_tabs(port: u16) -> AppResult<Vec<ChromeTabInfo>> {
        let url = format!("http://127.0.0.1:{}/json", port);
        let client = rquest::Client::new();
        let resp = client.get(url).send().await.map_err(|e| AppError::Network(e.to_string()))?;
        let tabs: Vec<ChromeTabInfo> = resp.json().await.map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(tabs.into_iter().filter(|t| t.tab_type == "page").collect())
    }

    async fn find_tab(browser: &Browser, tab_id: &str) -> AppResult<chromiumoxide::Page> {
        for _ in 0..15 {
            let pages = browser.pages().await.map_err(|e| AppError::Browser(e.to_string()))?;
            for page in &pages {
                if page.target_id().as_ref() == tab_id {
                    return Ok(page.clone());
                }
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        Err(AppError::NotFound(format!("Tab {} bulunamadı.", tab_id)))
    }

    pub async fn setup_tab_listeners(port: u16, tab_id: String) -> AppResult<()> {
        tracing::info!("[CORE -> BROWSER] Dinleyiciler kuruluyor: {}", tab_id);
        let (browser, _handler) = Self::connect_robust(port).await?;
        let page = Self::find_tab(&browser, &tab_id).await?;
        
        let mut network_events = page.event_listener::<chromiumoxide::cdp::browser_protocol::network::EventRequestWillBeSent>().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let mut response_events = page.event_listener::<chromiumoxide::cdp::browser_protocol::network::EventResponseReceived>().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let mut finished_events = page.event_listener::<chromiumoxide::cdp::browser_protocol::network::EventLoadingFinished>().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let mut console_events = page.event_listener::<EventConsoleApiCalled>().await.map_err(|e| AppError::Browser(e.to_string()))?;

        let page_arc = std::sync::Arc::new(page);
        let tid_inner = tab_id.clone();

        tokio::spawn(async move {
            let mut pending_responses: HashMap<String, (String, String)> = HashMap::new();
            loop {
                tokio::select! {
                    Some(e) = network_events.next() => {
                        // Resource type chromiumoxide 0.9.1'de direkt mevcuttur ama bazen EventRequestWillBeSent içinde olmayabilir.
                        // Güvenli erişim:
                        let res_type = format!("Other"); 
                        let req = NetworkRequest {
                            request_id: e.request_id.as_ref().to_string(),
                            url: e.request.url.clone(),
                            method: e.request.method.clone(),
                            resource_type: res_type,
                            status: None,
                            request_body: None,
                            response_body: None,
                        };
                        crate::ui::scrape::emit(crate::core::events::AppEvent::NetworkRequestSent(tid_inner.clone(), req));
                    }
                    Some(e) = response_events.next() => {
                        let rid = e.request_id.as_ref().to_string();
                        pending_responses.insert(rid.clone(), (e.response.url.clone(), e.response.mime_type.clone()));
                        let page_clone = page_arc.clone(); let rid_clone = e.request_id.clone(); let tid_res = tid_inner.clone(); let status = e.response.status as u16;
                        tokio::spawn(async move {
                            tokio::time::sleep(Duration::from_millis(400)).await;
                            if let Ok(res) = page_clone.execute(GetResponseBodyParams::new(rid_clone.clone())).await {
                                crate::ui::scrape::emit(crate::core::events::AppEvent::NetworkResponseReceived(tid_res, rid_clone.as_ref().to_string(), status, Some(res.result.body)));
                            }
                        });
                    }
                    Some(e) = finished_events.next() => {
                        let rid = e.request_id.as_ref().to_string();
                        if let Some((url, mime)) = pending_responses.remove(&rid) {
                            let lm = mime.to_lowercase();
                            if lm.contains("image") || lm.contains("video") || lm.contains("audio") || lm.contains("font") || lm.contains("style") || lm.contains("script") || url.ends_with(".svg") || url.ends_with(".css") || url.ends_with(".js") {
                                let page_clone = page_arc.clone(); let tid_media = tid_inner.clone();
                                tokio::spawn(async move {
                                    tokio::time::sleep(Duration::from_millis(600)).await;
                                    if let Ok(res) = page_clone.execute(GetResponseBodyParams::new(rid.clone())).await {
                                        let binary_data = if res.result.base64_encoded { BASE64_STANDARD.decode(&res.result.body).ok() } else { Some(res.result.body.into_bytes()) };
                                        if let Some(data) = binary_data {
                                            let name = url.split('/').last().unwrap_or("unknown").to_string();
                                            crate::ui::scrape::emit(crate::core::events::AppEvent::MediaCaptured(tid_media, crate::state::MediaAsset { name, url, mime_type: mime, size_bytes: data.len(), data: Some(data) }));
                                        }
                                    }
                                });
                            }
                        }
                    }
                    Some(e) = console_events.next() => {
                        let msg = e.args.iter().map(|v| v.value.as_ref().map(|v| v.to_string()).unwrap_or("undefined".into())).collect::<Vec<_>>().join(" ");
                        crate::ui::scrape::emit(crate::core::events::AppEvent::ConsoleLogAdded(tid_inner.clone(), msg));
                    }
                    else => break,
                }
            }
        });
        Ok(())
    }

    pub async fn reload_page(port: u16, tab_id: String) -> AppResult<()> {
        let (browser, _handler) = Self::connect_robust(port).await?;
        let page = Self::find_tab(&browser, &tab_id).await?;
        page.reload().await.map_err(|e| AppError::Browser(e.to_string()))?;
        Ok(())
    }

    pub async fn get_cookies(port: u16, tab_id: String) -> AppResult<Vec<ChromeCookie>> {
        let (browser, _handler) = Self::connect_robust(port).await?;
        let page = Self::find_tab(&browser, &tab_id).await?;
        let cookies = page.get_cookies().await.map_err(|e| AppError::Browser(e.to_string()))?;
        Ok(cookies.into_iter().map(|c| ChromeCookie {
            name: c.name.clone(), value: c.value.clone(), domain: c.domain.clone(), path: c.path.clone(),
            expires: c.expires, secure: c.secure, http_only: c.http_only,
        }).collect())
    }

    pub async fn delete_cookie(port: u16, tab_id: String, name: String, domain: String) -> AppResult<()> {
        let (browser, _handler) = Self::connect_robust(port).await?;
        let page = Self::find_tab(&browser, &tab_id).await?;
        let cmd = DeleteCookiesParams::builder().name(name).domain(domain).build().map_err(|e| AppError::Browser(e.to_string()))?;
        page.execute(cmd).await.map_err(|e| AppError::Browser(e.to_string()))?;
        Ok(())
    }

    pub async fn add_cookie(port: u16, tab_id: String, cookie: ChromeCookie) -> AppResult<()> {
        let (browser, _handler) = Self::connect_robust(port).await?;
        let page = Self::find_tab(&browser, &tab_id).await?;
        let mut builder = SetCookieParams::builder().name(cookie.name).value(cookie.value).domain(cookie.domain).path(cookie.path).secure(cookie.secure).http_only(cookie.http_only);
        if cookie.expires > 0.0 {
            let expires_json = serde_json::to_value(cookie.expires).unwrap_or_default();
            if let Ok(ts) = serde_json::from_value::<chromiumoxide::cdp::browser_protocol::network::TimeSinceEpoch>(expires_json) { builder = builder.expires(ts); }
        }
        let cmd = builder.build().map_err(|e| AppError::Browser(e.to_string()))?;
        page.execute(cmd).await.map_err(|e| AppError::Browser(e.to_string()))?;
        Ok(())
    }

    pub async fn capture_html(port: u16, tab_id: String, root: PathBuf) -> AppResult<PathBuf> {
        let (browser, _handler) = Self::connect_robust(port).await?;
        let page = Self::find_tab(&browser, &tab_id).await?;
        let url = page.url().await.map_err(|e| AppError::Browser(e.to_string()))?.unwrap_or_default();
        let html = page.content().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let final_dir = Self::get_output_path(root, "html_captures", &url)?;
        let path = final_dir.join("index.html");
        std::fs::write(&path, html).map_err(AppError::Io)?;
        Ok(path)
    }

    pub async fn capture_complete(port: u16, tab_id: String, root: PathBuf) -> AppResult<PathBuf> {
        let (browser, _handler) = Self::connect_robust(port).await?;
        let page = Self::find_tab(&browser, &tab_id).await?;
        let url = page.url().await.map_err(|e| AppError::Browser(e.to_string()))?.unwrap_or_default();
        let html = page.content().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let final_dir = Self::get_output_path(root, "complete_captures", &url)?;
        std::fs::write(final_dir.join("index.html"), html).map_err(AppError::Io)?;
        Ok(final_dir)
    }

    pub async fn capture_mirror(port: u16, tab_id: String, root: PathBuf) -> AppResult<PathBuf> {
        let (browser, _handler) = Self::connect_robust(port).await?;
        let page = Self::find_tab(&browser, &tab_id).await?;
        let url = page.url().await.map_err(|e| AppError::Browser(e.to_string()))?.unwrap_or_default();
        let final_dir = Self::get_output_path(root, "mirrors", &url)?;
        let mhtml = page.execute(chromiumoxide::cdp::browser_protocol::page::CaptureSnapshotParams::default()).await.map_err(|e| AppError::Browser(e.to_string()))?;
        let path = final_dir.join("snapshot.mhtml");
        std::fs::write(&path, mhtml.result.data).map_err(AppError::Io)?;
        Ok(path)
    }

    pub async fn execute_script(port: u16, tab_id: String, script: String) -> AppResult<String> {
        let (browser, _handler) = Self::connect_robust(port).await?;
        let page = Self::find_tab(&browser, &tab_id).await?;
        let res = page.evaluate(EvaluateParams::new(script)).await.map_err(|e| AppError::Browser(e.to_string()))?;
        Ok(res.value().cloned().unwrap_or_default().to_string())
    }

    pub async fn set_url_blocking(port: u16, tab_id: String, blocked_urls: Vec<String>) -> AppResult<()> {
        let (browser, _handler) = Self::connect_robust(port).await?;
        let page = Self::find_tab(&browser, &tab_id).await?;
        let url_patterns = blocked_urls.into_iter().map(|url| BlockPattern { url_pattern: url, block: true }).collect();
        let cmd = SetBlockedUrLsParams { url_patterns: Some(url_patterns) };
        page.execute(cmd).await.map_err(|e| AppError::Browser(e.to_string()))?;
        Ok(())
    }

    pub async fn get_page_selectors(port: u16, tab_id: String) -> AppResult<Vec<String>> {
        let (browser, _handler) = Self::connect_robust(port).await?;
        let page = Self::find_tab(&browser, &tab_id).await?;
        let script = r#"(() => { let sels = new Set(); document.querySelectorAll('*').forEach(el => { if (el.id) sels.add('#' + el.id); el.classList.forEach(c => sels.add('.' + c)); Array.from(el.attributes).forEach(attr => { if (attr.name.startsWith('data-') || attr.name === 'name' || attr.name === 'type') { sels.add(`[${attr.name}="${attr.value}"]`); } }); }); return Array.from(sels).sort(); })()"#;
        let res = page.evaluate(script).await.map_err(|e| AppError::Browser(e.to_string()))?;
        let sels: Vec<String> = serde_json::from_value(res.value().cloned().unwrap_or_default()).unwrap_or_default();
        Ok(sels)
    }

    pub fn get_output_path(root: PathBuf, category: &str, url_str: &str) -> AppResult<PathBuf> {
        let parsed = url::Url::parse(url_str).map_err(|e| AppError::Internal(e.to_string()))?;
        let domain = parsed.host_str().unwrap_or("unknown_domain");
        let path_slug = parsed.path().trim_matches('/').replace('/', "_");
        let path_slug = if path_slug.is_empty() { "index_html".to_string() } else { path_slug };
        let final_dir = root.join(category).join(domain).join(path_slug);
        std::fs::create_dir_all(&final_dir).map_err(AppError::Io)?;
        Ok(final_dir)
    }

    pub fn extract_resources_from_css(css_content: &str, base_url: &str) -> Vec<String> {
        let mut urls = Vec::new();
        let re_url = regex::Regex::new(r#"(?i)url\s*\(\s*['"]?([^'")]*)['"]?\s*\)"#).unwrap();
        let re_import = regex::Regex::new(r#"(?i)@import\s+['"]([^'"]+)['"]"#).unwrap();
        let base = url::Url::parse(base_url).ok();
        let mut find_all = |pattern: &regex::Regex| {
            for cap in pattern.captures_iter(css_content) {
                let found_url = cap[1].trim();
                if found_url.is_empty() || found_url.starts_with("data:") || found_url.starts_with("blob:") { continue; }
                if let Some(base) = &base {
                    if let Ok(abs_url) = base.join(found_url) {
                        let url_str = abs_url.to_string();
                        if !urls.contains(&url_str) { urls.push(url_str); }
                    }
                }
            }
        };
        find_all(&re_url);
        find_all(&re_import);
        urls
    }
}
