use super::{AppConfig, migration};
use std::env;
use std::path::PathBuf;
use anyhow::Result;

pub fn load_config() -> Result<AppConfig> {
    dotenv::dotenv().ok();

    let mut config = AppConfig::default();

    // .env'den gelen verilerle default'u ez
    if let Ok(v) = env::var("CONFIG_VERSION") { config.config_version = v.parse()?; }
    if let Ok(v) = env::var("DEFAULT_LAUNCH_URL") { config.default_launch_url = v; }
    if let Ok(v) = env::var("DEFAULT_PROFILE_DIR") { config.default_profile_dir = PathBuf::from(v); }
    if let Ok(v) = env::var("REMOTE_DEBUG_PORT") { config.remote_debug_port = v.parse()?; }
    if let Ok(v) = env::var("HEADLESS") { config.headless = v.parse()?; }
    if let Ok(v) = env::var("GEMINI_API_KEY") { config.gemini_api_key = v; }
    // ... diğer alanlar buraya eklenebilir

    // Migration çalıştır
    let migrated_config = migration::migrate(config);
    
    Ok(migrated_config)
}
