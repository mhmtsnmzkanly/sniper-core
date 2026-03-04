use crate::core::automation::driver::AutomationDriver;
use crate::core::error::{AppError, AppResult};
use async_trait::async_trait;
use chromiumoxide::Page;
use std::sync::Arc;

pub struct CdpDriver {
    pub page: Arc<Page>,
}

impl CdpDriver {
    pub fn new(page: Page) -> Self {
        Self {
            page: Arc::new(page),
        }
    }

    async fn run_js_internal(&self, script: &str) -> AppResult<String> {
        let recursive_helper = r#"
            const queryRecursive = (selector, root = document) => {
                const parts = selector.split(' >> ').map(s => s.trim());
                let current = root;
                for (const part of parts) {
                    if (current.contentDocument) current = current.contentDocument;
                    if (current.shadowRoot) current = current.shadowRoot;
                    const found = current.querySelector(part);
                    if (!found) return null;
                    current = found;
                }
                return current;
            };
        "#;

        let wrapped_js = format!(
            "(() => {{
                {}
                try {{
                    const result = (async () => {{ {} }})();
                    return Promise.resolve(result).then(r => JSON.stringify({{ success: true, data: r }}));
                }} catch (e) {{
                    return JSON.stringify({{ success: false, error: e.message }});
                }}
            }})()", recursive_helper, script
        );

        let result = self
            .page
            .evaluate(wrapped_js)
            .await
            .map_err(|e| AppError::Browser(e.to_string()))?;
        let val_str = result
            .value()
            .clone()
            .cloned()
            .unwrap_or_default()
            .to_string();

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&val_str) {
            if json["success"].as_bool() == Some(false) {
                let err_msg = json["error"].as_str().unwrap_or("Unknown JS error");
                return Err(AppError::Browser(format!("JS Error: {}", err_msg)));
            }
            let data = json["data"].to_string();
            return Ok(data.trim_matches('"').to_string());
        } else {
            tracing::debug!("[DRIVER] JS returned non-JSON result: {}", val_str);
        }
        Ok(val_str)
    }
}

pub trait ToJsString {
    fn to_js_string(&self) -> String;
}

impl ToJsString for &str {
    fn to_js_string(&self) -> String {
        self.replace("'", "\\'")
    }
}

#[async_trait]
impl AutomationDriver for CdpDriver {
    async fn navigate(&self, url: &str) -> AppResult<()> {
        self.page
            .goto(url)
            .await
            .map_err(|e| AppError::Browser(e.to_string()))?;
        Ok(())
    }

    async fn click(&self, selector: &str) -> AppResult<()> {
        let final_sel = selector.to_js_string();
        let js = format!(
            "const el = queryRecursive('{}'); \
             if (!el) throw new Error('Element not found'); \
             el.style.outline = '3px solid #ff00ff'; \
             el.scrollIntoView({{behavior: 'instant', block: 'center'}}); \
             el.click(); \
             return true;",
            final_sel
        );
        self.run_js_internal(&js).await?;
        Ok(())
    }

    async fn type_text(&self, selector: &str, value: &str) -> AppResult<()> {
        let final_sel = selector.to_js_string();
        let final_val = value.to_js_string();
        let js = format!(
            "const el = queryRecursive('{}'); \
             if (!el) throw new Error('Input not found'); \
             el.style.outline = '3px solid #00ffff'; \
             el.scrollIntoView({{behavior: 'instant', block: 'center'}}); \
             el.focus(); \
             el.value = '{}'; \
             el.dispatchEvent(new Event('input', {{ bubbles: true }})); \
             el.dispatchEvent(new Event('change', {{ bubbles: true }})); \
             return true;",
            final_sel, final_val
        );
        self.run_js_internal(&js).await?;
        Ok(())
    }

    async fn hover(&self, selector: &str) -> AppResult<()> {
        let final_sel = selector.to_js_string();
        let js = format!(
            "const el = queryRecursive('{}'); \
             if (!el) throw new Error('Element not found'); \
             const ev = new MouseEvent('mouseover', {{ bubbles: true }}); \
             el.dispatchEvent(ev); \
             return true;",
            final_sel
        );
        let _ = self.run_js_internal(&js).await?;
        Ok(())
    }

    async fn eval(&self, js: &str) -> AppResult<String> {
        self.run_js_internal(js).await
    }

    async fn screenshot(&self) -> AppResult<Vec<u8>> {
        self.page
            .screenshot(
                chromiumoxide::page::ScreenshotParams::builder()
                    .full_page(true)
                    .build(),
            )
            .await
            .map_err(|e| AppError::Browser(e.to_string()))
    }

    async fn wait_for_navigation(&self) -> AppResult<()> {
        let _ = self.page.wait_for_navigation().await;
        Ok(())
    }
}
