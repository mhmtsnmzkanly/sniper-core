use crate::config::AppConfig;
use crate::core::error::{AppError, AppResult};
use std::path::PathBuf;

pub fn load_config() -> AppResult<AppConfig> {
    let mut config = AppConfig::default();

    if let Ok(val) = std::env::var("CONFIG_VERSION") {
        if let Ok(v) = val.parse() { config.config_version = v; }
    }
    if let Ok(val) = std::env::var("OUTPUT_DIR") {
        config.output_dir = PathBuf::from(val);
    }
    if let Ok(val) = std::env::var("DEFAULT_PROFILE_DIR") {
        config.default_profile_dir = PathBuf::from(val);
    }
    if let Ok(val) = std::env::var("REMOTE_DEBUG_PORT") {
        if let Ok(port) = val.parse() { config.remote_debug_port = port; }
    }
    if let Ok(val) = std::env::var("DEFAULT_LAUNCH_URL") {
        config.default_launch_url = val;
    }
    if let Ok(val) = std::env::var("GEMINI_API_KEY") {
        config.gemini_api_key = val;
    }
    if let Ok(val) = std::env::var("GEMINI_API_URL") {
        config.gemini_api_url = val;
    }

    Ok(config)
}

pub fn save_config(config: &AppConfig) -> AppResult<()> {
    let content = format!(
        "CONFIG_VERSION={}\n\
         OUTPUT_DIR={}\n\
         DEFAULT_PROFILE_DIR={}\n\
         REMOTE_DEBUG_PORT={}\n\
         DEFAULT_LAUNCH_URL={}\n\
         DOWNLOAD_TIMEOUT={}\n\
         MAX_CONCURRENT_DOWNLOAD={}\n\
         GEMINI_API_KEY={}\n\
         GEMINI_API_URL={}\n",
        config.config_version,
        config.output_dir.display(),
        config.default_profile_dir.display(),
        config.remote_debug_port,
        config.default_launch_url,
        config.download_timeout,
        config.max_concurrent_download,
        config.gemini_api_key,
        config.gemini_api_url
    );

    std::fs::write(".env", content).map_err(AppError::Io)
}
