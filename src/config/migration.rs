use super::{AppConfig, CURRENT_CONFIG_VERSION};
use tracing::info;

pub fn migrate(mut config: AppConfig) -> AppConfig {
    let mut version = config.config_version;

    // Örnek Migration Akışı
    if version == 0 {
        info!("Migrating config from v0 to v1...");
        // v1'e özel başlangıç ayarları burada yapılabilir
        version = 1;
    }

    // Gelecekteki versiyonlar için:
    /*
    if version == 1 {
        info!("Migrating config from v1 to v2...");
        // Yeni eklenen alanlar config.new_field = ... şeklinde doldurulur
        version = 2;
    }
    */

    config.config_version = CURRENT_CONFIG_VERSION;
    config
}
