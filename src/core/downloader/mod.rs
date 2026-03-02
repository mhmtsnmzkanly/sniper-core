use anyhow::{anyhow, Result};
use std::path::PathBuf;
use tracing::info;

pub struct Downloader;

impl Downloader {
    /// Bir URL'yi belirtilen yerel yola indirir
    pub async fn download_asset(url: &str, target_path: PathBuf) -> Result<()> {
        let client = rquest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let resp = client.get(url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("Failed to download: {} (Status: {})", url, resp.status()));
        }

        let bytes = resp.bytes().await?;
        
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        std::fs::write(&target_path, bytes)?;
        debug!("Asset saved: {:?}", target_path);
        Ok(())
    }
}

use tracing::debug;
