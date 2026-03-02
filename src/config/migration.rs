use super::AppConfig;
use tracing::info;

pub fn migrate(mut config: AppConfig) -> AppConfig {
    let current_version = config.config_version;
    let target_version = AppConfig::default().config_version;

    if current_version < target_version {
        info!("Migrating config from version {} to {}", current_version, target_version);
        // Migration logic for future versions:
        // if current_version == 1 { ... config.new_field = default; config.version = 2; }
    }

    config.config_version = target_version;
    config
}
