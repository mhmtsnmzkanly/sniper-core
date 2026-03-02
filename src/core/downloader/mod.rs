use crate::core::error::{AppError, AppResult};
use std::path::PathBuf;
use tracing::debug;

pub struct Downloader;

impl Downloader {
    /// Bir URL'yi belirtilen yerel yola indirir
    pub async fn download_asset(url: &str, target_path: PathBuf) -> AppResult<()> {
        let client = rquest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| AppError::Network(format!("Client build error: {}", e)))?;

        let resp = client.get(url).send().await
            .map_err(|e| AppError::Network(format!("Request failed: {}", e)))?;
            
        if !resp.status().is_success() {
            return Err(AppError::Network(format!("Failed to download: {} (Status: {})", url, resp.status())));
        }

        let bytes = resp.bytes().await
            .map_err(|e| AppError::Network(format!("Body fetch failed: {}", e)))?;
        
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent).map_err(AppError::Io)?;
        }
        
        std::fs::write(&target_path, bytes).map_err(AppError::Io)?;
        debug!("Asset saved: {:?}", target_path);
        Ok(())
    }
}
