use serde::{Serialize, Deserialize};
use std::path::PathBuf;

pub mod loader;
pub mod migration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub config_version: u32,
    pub default_launch_url: String,
    pub default_profile_dir: PathBuf,
    pub remote_debug_port: u16,
    pub headless: bool,
    pub download_timeout: u64,
    pub max_concurrent_download: usize,
    pub raw_output_dir: PathBuf,
    pub translator_output_dir: PathBuf,
    pub log_output_dir: PathBuf,
    pub write_log_to_file: bool,
    pub gemini_api_url: String,
    pub gemini_api_key: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            config_version: 1,
            default_launch_url: "https://www.google.com".to_string(),
            default_profile_dir: PathBuf::from("chrome_profile"),
            remote_debug_port: 9222,
            headless: false,
            download_timeout: 30,
            max_concurrent_download: 8,
            raw_output_dir: PathBuf::from("raw"),
            translator_output_dir: PathBuf::from("translated"),
            log_output_dir: PathBuf::from("logs"),
            write_log_to_file: true,
            gemini_api_url: "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent".to_string(),
            gemini_api_key: String::new(),
        }
    }
}
