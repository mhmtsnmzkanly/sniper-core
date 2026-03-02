pub mod loader;
use std::path::PathBuf;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub config_version: u32,
    pub output_dir: PathBuf,
    pub default_profile_dir: PathBuf,
    pub remote_debug_port: u16,
    pub default_launch_url: String,
    pub download_timeout: u64,
    pub max_concurrent_download: usize,
    pub gemini_api_key: String,
    pub gemini_api_url: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            config_version: 1,
            output_dir: PathBuf::from("studio_output"),
            default_profile_dir: PathBuf::from("chrome_profile"),
            remote_debug_port: 9222,
            default_launch_url: "https://www.google.com".to_string(),
            download_timeout: 30,
            max_concurrent_download: 8,
            gemini_api_key: String::new(),
            gemini_api_url: "https://generativelanguage.googleapis.com/v1beta/models/gemini-pro:generateContent".to_string(),
        }
    }
}
