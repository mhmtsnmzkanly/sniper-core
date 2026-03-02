use super::{AppConfig, migration, CURRENT_CONFIG_VERSION};
use std::env;
use std::fs;
use std::path::Path;
use crate::core::error::{AppError, AppResult};
use tracing::{info, warn};

pub fn load_config() -> AppResult<AppConfig> {
    let env_path = Path::new(".env");
    let mut config = AppConfig::default();
    let mut needs_save = false;

    if env_path.exists() {
        dotenv::dotenv().ok();
        
        let file_version = env::var("CONFIG_VERSION")
            .unwrap_or_else(|_| "0".to_string())
            .parse::<u32>()
            .map_err(|e| AppError::Config(format!("Invalid version format: {}", e)))?;

        if let Ok(v) = env::var("DEFAULT_LAUNCH_URL") { config.default_launch_url = v; }
        if let Ok(v) = env::var("DEFAULT_PROFILE_DIR") { config.default_profile_dir = v.into(); }
        if let Ok(v) = env::var("REMOTE_DEBUG_PORT") { 
            config.remote_debug_port = v.parse().map_err(|e| AppError::Config(format!("Invalid port: {}", e)))?; 
        }
        if let Ok(v) = env::var("HEADLESS") { 
            config.headless = v.parse().map_err(|e| AppError::Config(format!("Invalid headless flag: {}", e)))?; 
        }
        if let Ok(v) = env::var("GEMINI_API_KEY") { config.gemini_api_key = v; }
        
        config.config_version = file_version;

        if file_version < CURRENT_CONFIG_VERSION {
            info!("Config migration needed: v{} -> v{}", file_version, CURRENT_CONFIG_VERSION);
            config = migration::migrate(config);
            needs_save = true;
        }
    } else {
        warn!(".env file not found, creating default config v{}", CURRENT_CONFIG_VERSION);
        needs_save = true;
    }

    if needs_save {
        save_config(&config)?;
    }

    Ok(config)
}

pub fn save_config(config: &AppConfig) -> AppResult<()> {
    let content = format!(
        "CONFIG_VERSION={}\n\
         DEFAULT_LAUNCH_URL={}\n\
         DEFAULT_PROFILE_DIR={}\n\
         REMOTE_DEBUG_PORT={}\n\
         HEADLESS={}\n\
         DOWNLOAD_TIMEOUT={}\n\
         MAX_CONCURRENT_DOWNLOAD={}\n\
         RAW_OUTPUT_DIR={}\n\
         TRANSLATOR_OUTPUT_DIR={}\n\
         LOG_OUTPUT_DIR={}\n\
         WRITE_LOG_TO_FILE={}\n\
         GEMINI_API={}\n\
         GEMINI_API_KEY={}\n",
        config.config_version,
        config.default_launch_url,
        config.default_profile_dir.display(),
        config.remote_debug_port,
        config.headless,
        config.download_timeout,
        config.max_concurrent_download,
        config.raw_output_dir.display(),
        config.translator_output_dir.display(),
        config.log_output_dir.display(),
        config.write_log_to_file,
        config.gemini_api_url,
        config.gemini_api_key
    );

    fs::write(".env", content).map_err(AppError::Io)?;
    info!("Config saved to .env (v{})", config.config_version);
    Ok(())
}
