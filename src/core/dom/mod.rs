use scraper::{Html, Selector};
use url::Url;
use std::collections::HashSet;

pub struct DomProcessor;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetType {
    Image,
    Style,
    Script,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiscoveredAsset {
    pub url: String,
    pub asset_type: AssetType,
}

impl DomProcessor {
    /// HTML içindeki tüm dış varlıkları (resim, css, js) bulur
    pub fn discover_assets(html_content: &str, base_url: &str) -> HashSet<DiscoveredAsset> {
        let document = Html::parse_document(html_content);
        let mut assets = HashSet::new();
        let base = Url::parse(base_url).ok();

        // 1. Resimler (img src)
        let img_selector = Selector::parse("img").unwrap();
        for img in document.select(&img_selector) {
            if let Some(src) = img.value().attr("src") {
                if let Some(absolute_url) = Self::make_absolute(src, base.as_ref()) {
                    assets.insert(DiscoveredAsset { url: absolute_url, asset_type: AssetType::Image });
                }
            }
        }

        // 2. CSS (link rel=stylesheet)
        let link_selector = Selector::parse("link[rel='stylesheet']").unwrap();
        for link in document.select(&link_selector) {
            if let Some(href) = link.value().attr("href") {
                if let Some(absolute_url) = Self::make_absolute(href, base.as_ref()) {
                    assets.insert(DiscoveredAsset { url: absolute_url, asset_type: AssetType::Style });
                }
            }
        }

        // 3. Script (script src)
        let script_selector = Selector::parse("script[src]").unwrap();
        for script in document.select(&script_selector) {
            if let Some(src) = script.value().attr("src") {
                if let Some(absolute_url) = Self::make_absolute(src, base.as_ref()) {
                    assets.insert(DiscoveredAsset { url: absolute_url, asset_type: AssetType::Script });
                }
            }
        }

        assets
    }

    fn make_absolute(path: &str, base: Option<&Url>) -> Option<String> {
        if path.starts_with("data:") { return None; }
        if let Ok(url) = Url::parse(path) {
            return Some(url.to_string());
        }
        if let Some(base_url) = base {
            if let Ok(joined) = base_url.join(path) {
                return Some(joined.to_string());
            }
        }
        None
    }
}
