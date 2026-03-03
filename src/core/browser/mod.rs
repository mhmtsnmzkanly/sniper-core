use crate::core::error::{AppError, AppResult};
use std::path::PathBuf;
use std::process::{Stdio, Child};
use chromiumoxide::Browser;
use crate::state::ChromeTabInfo;
use futures::StreamExt;
use std::time::Duration;

pub struct BrowserManager;

impl BrowserManager {
    pub fn get_system_profile_path() -> PathBuf {
        #[cfg(target_os = "linux")]
        {
            let home = std::env::var("HOME").unwrap_or_default();
            let paths = [format!("{}/.config/google-chrome", home), format!("{}/.config/chromium", home)];
            for p in paths {
                let path = PathBuf::from(p);
                if path.exists() { return path; }
            }
        }
        #[cfg(target_os = "windows")]
        {
            let appdata = std::env::var("LOCALAPPDATA").unwrap_or_default();
            let path = PathBuf::from(appdata).join("Google").join("Chrome").join("User Data");
            if path.exists() { return path; }
        }
        PathBuf::from("chrome_profile")
    }

    pub fn find_executable() -> AppResult<PathBuf> {
        #[cfg(target_os = "linux")]
        {
            let paths = ["/usr/bin/google-chrome", "/usr/bin/chromium", "/usr/bin/chrome", "/usr/bin/google-chrome-stable"];
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

    pub async fn launch(url: &str, profile: PathBuf, port: u16, log_dir: PathBuf, timestamp: String) -> AppResult<Child> {
        let exec_path = Self::find_executable()?;
        let _ = std::fs::create_dir_all(&log_dir);
        let chrome_log_file = std::fs::File::create(log_dir.join(format!("chrome.{}.log", timestamp))).map_err(AppError::Io)?;
        let mut cmd = std::process::Command::new(exec_path);
        cmd.arg("--no-sandbox")
            .arg(format!("--remote-debugging-port={}", port))
            .arg("--remote-allow-origins=*")
            .arg("--no-first-run")
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg(url)
            .stdout(Stdio::from(chrome_log_file.try_clone().map_err(AppError::Io)?))
            .stderr(Stdio::from(chrome_log_file));
        #[cfg(unix)] { use std::os::unix::process::CommandExt; cmd.process_group(0); }
        cmd.spawn().map_err(AppError::Io)
    }

    pub async fn get_page_selectors(port: u16, tab_id: String) -> AppResult<Vec<String>> {
        let (browser, _handler) = Self::connect_robust(port).await?;
        let pages = browser.pages().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let page = pages.into_iter().find(|p| p.target_id().as_ref() == tab_id).ok_or_else(|| AppError::NotFound("Page not found".into()))?;
        
        let js = r#"(() => {
            const results = [];
            const seenElements = new Set();
            const importantAttrs = ['data-testid', 'data-id', 'name', 'id', 'href', 'role'];
            
            // Focus on interactive and structure-heavy elements
            const elements = document.querySelectorAll('a, button, input, select, textarea, [id], [data-testid]');
            
            elements.forEach(el => {
                if (seenElements.has(el)) return;
                seenElements.add(el);

                const tag = el.tagName.toLowerCase();
                
                // Priority 1: ID
                if (el.id) {
                    results.push(`${tag}#${el.id}`);
                    return;
                }

                // Priority 2: Data Attributes or Name
                for (const attr of ['data-testid', 'data-id', 'name']) {
                    const val = el.getAttribute(attr);
                    if (val && val.length < 50) {
                        results.push(`${tag}[${attr}="${val}"]`);
                        return;
                    }
                }

                // Priority 3: Classes (if not too many or dynamic-looking)
                if (el.classList.length > 0) {
                    const classes = Array.from(el.classList)
                        .filter(c => !/\d/.test(c)) // Skip classes with numbers (likely dynamic)
                        .map(c => '.' + c).join('');
                    if (classes) {
                        results.push(`${tag}${classes}`);
                        return;
                    }
                }

                // Priority 4: Href (for links)
                const href = el.getAttribute('href');
                if (href && href.length < 40 && href !== '#') {
                    results.push(`${tag}[href="${href}"]`);
                    return;
                }
            });

            return Array.from(new Set(results)).sort();
        })()"#;
        
        let res = page.evaluate(js).await.map_err(|e| AppError::Browser(e.to_string()))?;
        let value = res.value().clone().cloned().unwrap_or(serde_json::Value::Array(vec![]));
        let selectors: Vec<String> = serde_json::from_value(value).unwrap_or_default();
        tracing::info!("[BROWSER <-> DISCOVERY] Found {} unique selectors.", selectors.len());
        Ok(selectors)
    }

    pub async fn get_ws_url(port: u16) -> AppResult<String> {
        let client = rquest::Client::new();
        let resp = client.get(format!("http://127.0.0.1:{}/json/version", port)).send().await?;
        let json: serde_json::Value = resp.json().await?;
        Ok(json["webSocketDebuggerUrl"].as_str().ok_or_else(|| AppError::NotFound("WS URL missing".into()))?.to_string())
    }

    async fn connect_robust(port: u16) -> AppResult<(Browser, tokio::task::JoinHandle<()>)> {
        let ws_url = Self::get_ws_url(port).await?;
        let (browser, mut handler) = Browser::connect(ws_url).await.map_err(|e| AppError::Browser(e.to_string()))?;
        let handle = tokio::spawn(async move { while let Some(_) = handler.next().await {} });
        tokio::time::sleep(Duration::from_millis(200)).await;
        Ok((browser, handle))
    }

    pub async fn list_tabs(port: u16) -> AppResult<Vec<ChromeTabInfo>> {
        let client = rquest::Client::builder().timeout(Duration::from_millis(1500)).build()?;
        let url = format!("http://127.0.0.1:{}/json/list", port);
        let resp = client.get(url).send().await.map_err(|e: rquest::Error| AppError::Network(e.to_string()))?;
        let tabs: Vec<ChromeTabInfo> = resp.json().await?;
        Ok(tabs.into_iter().filter(|t| t.tab_type == "page").collect())
    }

    pub async fn setup_tab_listeners(port: u16, tab_id: String) -> AppResult<()> {
        use chromiumoxide::cdp::browser_protocol::network::{EventRequestWillBeSent, EventResponseReceived, EventLoadingFinished, EnableParams as NetEnable, GetResponseBodyParams};
        use chromiumoxide::cdp::js_protocol::runtime::{EventConsoleApiCalled, EnableParams as RuntimeEnable};
        use base64::{prelude::BASE64_STANDARD, Engine};

        let (browser, _handler_job) = Self::connect_robust(port).await?;
        let pages = browser.pages().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let page = pages.into_iter().find(|p| p.target_id().as_ref() == tab_id).ok_or_else(|| AppError::NotFound("No page".into()))?;
        
        page.execute(NetEnable::default()).await.map_err(|e| AppError::Browser(e.to_string()))?;
        page.execute(RuntimeEnable::default()).await.map_err(|e| AppError::Browser(e.to_string()))?;
        
        let mut request_events = page.event_listener::<EventRequestWillBeSent>().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let mut response_events = page.event_listener::<EventResponseReceived>().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let mut finished_events = page.event_listener::<EventLoadingFinished>().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let mut console_events = page.event_listener::<EventConsoleApiCalled>().await.map_err(|e| AppError::Browser(e.to_string()))?;
        
        let tid_inner = tab_id.clone();
        let page_arc = std::sync::Arc::new(page);

        tokio::spawn(async move {
            let _browser_keepalive = browser;
            let mut pending_responses = std::collections::HashMap::new();
            loop {
                tokio::select! {
                    Some(e) = request_events.next() => {
                        let req = crate::state::NetworkRequest { request_id: e.request_id.as_ref().to_string(), url: e.request.url.clone(), method: e.request.method.clone(), resource_type: format!("{:?}", e.r#type), status: None, request_body: None, response_body: None };
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
                            if mime.contains("image") || mime.contains("video") {
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
                        let msg = e.args.iter().map(|v| {
                            match &v.value {
                                Some(serde_json::Value::String(s)) => s.clone(),
                                Some(serde_json::Value::Null) => "null".to_string(),
                                Some(other) => other.to_string(),
                                None => "undefined".to_string(),
                            }
                        }).collect::<Vec<_>>().join(" ");
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
        let pages = browser.pages().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let page = pages.into_iter().find(|p| p.target_id().as_ref() == tab_id).ok_or_else(|| AppError::NotFound("Page not found".into()))?;
        page.execute(chromiumoxide::cdp::browser_protocol::page::ReloadParams::builder().ignore_cache(true).build()).await.map_err(|e| AppError::Browser(e.to_string()))?;
        Ok(())
    }

    pub async fn execute_script(port: u16, tab_id: String, script: String) -> AppResult<String> {
        let (browser, _handler) = Self::connect_robust(port).await?;
        let pages = browser.pages().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let page = pages.into_iter().find(|p| p.target_id().as_ref() == tab_id).ok_or_else(|| AppError::NotFound("Page not found".into()))?;
        
        // Wrap script to catch JS errors and return them to Rust
        let wrapped_js = format!(
            "(() => {{ try {{ \
                const result = {}; \
                return JSON.stringify({{ success: true, data: result }}); \
            }} catch (e) {{ \
                return JSON.stringify({{ success: false, error: e.message }}); \
            }} }})()", script
        );

        let result = page.evaluate(wrapped_js).await.map_err(|e| AppError::Browser(e.to_string()))?;
        let val_str = result.value().clone().cloned().unwrap_or_default().to_string();
        
        // Check if JS reported an error
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&val_str) {
            if json["success"].as_bool() == Some(false) {
                let err_msg = json["error"].as_str().unwrap_or("Unknown JS error");
                return Err(AppError::Browser(format!("JS Error: {}", err_msg)));
            }
            return Ok(json["data"].to_string());
        }
        
        Ok(val_str)
    }

    pub async fn capture_html(port: u16, tab_id: String, save_root: PathBuf, mirror_mode: bool, asset_folder: bool) -> AppResult<PathBuf> {
        use chromiumoxide::cdp::browser_protocol::page::CaptureSnapshotParams;
        let (browser, _handler) = Self::connect_robust(port).await?;
        let pages = browser.pages().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let page = pages.into_iter().find(|p| p.target_id().as_ref() == tab_id).ok_or_else(|| AppError::NotFound("Page not found".into()))?;
        let url_str = page.url().await.map_err(|e| AppError::Browser(e.to_string()))?.unwrap_or_default();
        
        if mirror_mode {
            let snapshot = page.execute(CaptureSnapshotParams::default()).await.map_err(|e| AppError::Browser(e.to_string()))?;
            let dir = Self::get_output_path(save_root, "MIRROR", &url_str)?;
            let final_path = dir.join("index.mhtml");
            std::fs::write(&final_path, snapshot.result.data.as_bytes()).map_err(AppError::Io)?;
            return Ok(final_path);
        }

        let mut content = page.content().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let dir = Self::get_output_path(save_root, if asset_folder { "COMPLETE" } else { "HTML" }, &url_str)?;
        
        if asset_folder {
            let assets_dir = dir.join("assets");
            let _ = std::fs::create_dir_all(&assets_dir);
            
            // Extract all src/href with a simple regex for speed, download them
            let re = regex::Regex::new(r#"(?i)(src|href)\s*=\s*['"]([^'"]+)['"]"#).unwrap();
            let base = url::Url::parse(&url_str).ok();
            let client = rquest::Client::builder().timeout(Duration::from_secs(5)).build().unwrap_or_default();

            let mut downloads = Vec::new();
            for cap in re.captures_iter(&content) {
                let attr_found = cap[1].to_string();
                let found_url = cap[2].to_string();
                if found_url.starts_with("data:") || found_url.starts_with("blob:") || found_url.starts_with("#") { continue; }
                
                if let Some(base) = &base {
                    if let Ok(abs_url) = base.join(&found_url) {
                        let filename = abs_url.path().split('/').last().unwrap_or("asset").to_string();
                        let filename = if filename.is_empty() { "index".to_string() } else { filename };
                        let final_name = format!("{}_{}", &chrono::Local::now().timestamp_nanos_opt().unwrap_or(0), filename);
                        let save_path = assets_dir.join(&final_name);
                        
                        let url_to_get = abs_url.clone();
                        let client_clone = client.clone();
                        let original_url = found_url.clone();
                        downloads.push(async move {
                            if let Ok(resp) = client_clone.get(url_to_get).send().await {
                                if let Ok(bytes) = resp.bytes().await {
                                    let _ = std::fs::write(save_path, bytes);
                                    return Some((original_url, format!("assets/{}", final_name)));
                                }
                            }
                            None
                        });
                    }
                }
            }

            let results = futures::future::join_all(downloads).await;
            for res in results.into_iter().flatten() {
                content = content.replace(&res.0, &res.1);
            }
        }

        let final_path = dir.join("index.html");
        std::fs::write(&final_path, content.as_bytes()).map_err(AppError::Io)?;
        Ok(final_path)
    }

    pub async fn get_cookies(port: u16, tab_id: String) -> AppResult<Vec<crate::state::ChromeCookie>> {
        let (browser, _handler) = Self::connect_robust(port).await?;
        let pages = browser.pages().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let page = pages.into_iter().find(|p| p.target_id().as_ref() == tab_id).ok_or_else(|| AppError::NotFound("Page not found".into()))?;
        let cookies = page.get_cookies().await.map_err(|e| AppError::Browser(e.to_string()))?;
        Ok(cookies.into_iter().map(|c| crate::state::ChromeCookie { name: c.name.clone(), value: c.value.clone(), domain: c.domain.clone(), path: c.path.clone(), expires: c.expires, secure: c.secure, http_only: c.http_only }).collect())
    }

    pub async fn delete_cookie(port: u16, tab_id: String, name: String, domain: String) -> AppResult<()> {
        use chromiumoxide::cdp::browser_protocol::network::DeleteCookiesParams;
        let (browser, _handler) = Self::connect_robust(port).await?;
        let pages = browser.pages().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let page = pages.into_iter().find(|p| p.target_id().as_ref() == tab_id).ok_or_else(|| AppError::NotFound("Page not found".into()))?;
        let cmd = DeleteCookiesParams::builder().name(name).domain(domain).build().map_err(|e| AppError::Browser(e))?;
        page.execute(cmd).await.map_err(|e| AppError::Browser(e.to_string()))?;
        Ok(())
    }

    pub async fn add_cookie(port: u16, tab_id: String, cookie: crate::state::ChromeCookie) -> AppResult<()> {
        use chromiumoxide::cdp::browser_protocol::network::{SetCookieParams, TimeSinceEpoch};
        let (browser, _handler) = Self::connect_robust(port).await?;
        let pages = browser.pages().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let page = pages.into_iter().find(|p| p.target_id().as_ref() == tab_id).ok_or_else(|| AppError::NotFound("Page not found".into()))?;
        
        let mut builder = SetCookieParams::builder()
            .name(cookie.name)
            .value(cookie.value)
            .domain(cookie.domain)
            .path(cookie.path)
            .secure(cookie.secure)
            .http_only(cookie.http_only);
            
        if cookie.expires > 0.0 {
            builder = builder.expires(TimeSinceEpoch::new(cookie.expires));
        }

        let cmd = builder.build().map_err(|e| AppError::Browser(e))?;
        page.execute(cmd).await.map_err(|e| AppError::Browser(e.to_string()))?;
        Ok(())
    }

    pub async fn set_url_blocking(port: u16, tab_id: String, blocked_urls: Vec<String>) -> AppResult<()> {
        use chromiumoxide::cdp::browser_protocol::network::{SetBlockedUrLsParams, BlockPattern};
        let (browser, _handler) = Self::connect_robust(port).await?;
        let pages = browser.pages().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let page = pages.into_iter().find(|p| p.target_id().as_ref() == tab_id).ok_or_else(|| AppError::NotFound("Page not found".into()))?;
        
        let url_patterns = blocked_urls.into_iter().map(|url| BlockPattern { url_pattern: url, block: true }).collect();
        let cmd = SetBlockedUrLsParams { url_patterns: Some(url_patterns) };
        page.execute(cmd).await.map_err(|e| AppError::Browser(e.to_string()))?;
        Ok(())
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
        // More robust regex for url(), supporting quotes and whitespace variations
        let re = regex::Regex::new(r#"(?i)url\s*\(\s*['"]?([^'")]*)['"]?\s*\)"#).unwrap();
        let base = url::Url::parse(base_url).ok();

        for cap in re.captures_iter(css_content) {
            let found_url = cap[1].trim();
            if found_url.is_empty() || found_url.starts_with("data:") || found_url.starts_with("blob:") { continue; }
            
            if let Some(base) = &base {
                if let Ok(abs_url) = base.join(found_url) {
                    let url_str = abs_url.to_string();
                    if !urls.contains(&url_str) {
                        urls.push(url_str);
                    }
                }
            } else if !urls.contains(&found_url.to_string()) {
                urls.push(found_url.to_string());
            }
        }
        urls
    }
}
