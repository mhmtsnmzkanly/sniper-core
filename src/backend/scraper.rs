use anyhow::{anyhow, Result};
use tracing::info;
use std::path::PathBuf;
use std::process::{Stdio, Child};
use std::os::unix::process::CommandExt;
use headless_chrome::{Browser, Tab};
use crate::state::ChromeTabInfo;

pub struct Scraper;

impl Scraper {
    /// 1. Tarayıcıyı Başlat (Bağımsız Grup Olarak)
    pub async fn launch(target_url: &str, profile_path: Option<PathBuf>, port: u16, timestamp: String) -> Result<Child> {
        let final_profile = profile_path.unwrap_or_else(|| {
            std::env::current_dir().unwrap().join("chrome_profile")
        });

        let _ = std::fs::create_dir_all("logs");
        let chrome_log_path = format!("logs/chrome.{}.log", timestamp);
        let chrome_log_file = std::fs::File::create(&chrome_log_path)?;

        std::process::Command::new("/usr/bin/chromium")
            .arg("--no-sandbox")
            .arg(format!("--remote-debugging-port={}", port))
            .arg("--remote-allow-origins=*")
            .arg("--disable-features=OptimizationGuideModelDownloading,OnDeviceModel")
            .arg("--disable-background-networking")
            .arg("--disable-sync")
            .arg("--no-first-run")
            .arg(format!("--user-data-dir={}", final_profile.display()))
            .arg(target_url)
            .stdout(Stdio::from(chrome_log_file.try_clone()?))
            .stderr(Stdio::from(chrome_log_file))
            .process_group(0) 
            .spawn()
            .map_err(|e| anyhow!("Chromium failed to start: {}", e))
    }

    /// 2. Sekmeleri Listele (Hızlı HTTP)
    pub async fn list_tabs(port: u16) -> Result<Vec<ChromeTabInfo>> {
        let client = rquest::Client::builder()
            .timeout(std::time::Duration::from_millis(800))
            .build()?;
        
        let url = format!("http://127.0.0.1:{}/json/list", port);
        let tabs: Vec<ChromeTabInfo> = client.get(url).send().await?.json().await?;

        // Sadece gerçek sayfaları döndür
        Ok(tabs.into_iter()
            .filter(|t| t.tab_type == "page" && !t.url.is_empty() && t.url != "about:blank")
            .collect())
    }

    /// 3. Doğrudan Sekme İçeriğini Yakala (Secure Connection via Port + ID)
    pub async fn capture_tab_content(port: u16, tab_id: String, save_root: PathBuf) -> Result<PathBuf> {
        let ws_url = Self::get_ws_url(port).await?;
        let browser = Browser::connect(ws_url)?;
        
        let tabs_mutex = browser.get_tabs();
        let tabs = tabs_mutex.lock().map_err(|_| anyhow!("Tabs locked"))?;
        
        // Kullanıcının tıkladığı ID'ye sahip sekmeyi bul
        let tab = tabs.iter()
            .find(|t| t.get_target_id().to_string() == tab_id)
            .ok_or(anyhow!("Selected tab not found or closed."))?
            .clone();
        
        drop(tabs);

        let current_url = tab.get_url();
        let parsed_url = url::Url::parse(&current_url)?;
        
        // Klasörleme Mantığı (Domain)
        let domain = parsed_url.host_str().unwrap_or("unknown_site");
        let folder_path = save_root.join(domain);
        let _ = std::fs::create_dir_all(&folder_path);

        // Path temizleme: /user/profil -> user.profil.html
        let path_clean = parsed_url.path()
            .trim_matches('/')
            .replace(|c: char| !c.is_alphanumeric() && c != '-', ".");
        
        let file_name = if path_clean.is_empty() {
            "index.html".to_string()
        } else {
            format!("{}.html", path_clean)
        };

        let final_path = folder_path.join(file_name);
        let html = tab.get_content()?;
        
        std::fs::write(&final_path, html.as_bytes())?;
        Ok(final_path)
    }
}
