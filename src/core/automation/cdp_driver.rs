use crate::core::automation::driver::AutomationDriver;
use crate::core::error::{AppError, AppResult};
use chromiumoxide::Page;
use std::sync::Arc;
use async_trait::async_trait;

/// CdpDriver: A CDP-specific implementation of the AutomationDriver trait.
/// It uses chromiumoxide to send low-level commands to the browser.
pub struct CdpDriver {
    /// Arc-wrapped Page handle for thread-safe CDP interaction.
    pub page: Arc<Page>,
}

impl CdpDriver {
    pub fn new(page: Page) -> Self {
        Self {
            page: Arc::new(page),
        }
    }

    /// Internal helper to execute and wrap JS in a try-catch block for safe evaluation.
    /// It ensures that recursive frame queries (>>) are resolved correctly.
    async fn run_js_internal(&self, script: &str) -> AppResult<String> {
        // queryRecursive: Helper function injected into the page to handle deep selector resolution.
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

        // Wrap the user script in an async sandbox with structured JSON error reporting.
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

        let result = self.page.evaluate(wrapped_js).await.map_err(|e| AppError::Browser(e.to_string()))?;
        let val_str = result.value().clone().cloned().unwrap_or_default().to_string();
        
        // Parse the sandbox response.
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&val_str) {
            if json["success"].as_bool() == Some(false) {
                let err_msg = json["error"].as_str().unwrap_or("Unknown JS error");
                return Err(AppError::Browser(format!("JS Error: {}", err_msg)));
            }
            let data = json["data"].to_string();
            return Ok(data.trim_matches('"').to_string());
        } else {
            // Result was not JSON-wrapped (e.g., raw string result)
            tracing::debug!("[DRIVER] JS returned non-JSON result: {}", val_str);
        }
        Ok(val_str)
    }
}

/// Helper trait to safely escape strings for JS injection.
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
    /// Navigates the page to a specific URL.
    async fn navigate(&self, url: &str) -> AppResult<()> {
        self.page.goto(url).await.map_err(|e| AppError::Browser(e.to_string()))?;
        Ok(())
    }

    /// Highlights and clicks an element using recursive query resolution.
    async fn click(&self, selector: &str) -> AppResult<()> {
        let final_sel = selector.to_js_string();
        let js = format!(
            "const el = queryRecursive('{}'); \
             if (!el) throw new Error('Element not found'); \
             el.style.outline = '3px solid #ff00ff'; \
             el.scrollIntoView({{behavior: 'instant', block: 'center'}}); \
             el.click(); \
             return true;", final_sel
        );
        self.run_js_internal(&js).await?;
        Ok(())
    }

    /// Highlights, focuses, and sets the value of an input element.
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
             return true;", final_sel, final_val
        );
        self.run_js_internal(&js).await?;
        Ok(())
    }

    /// Triggers a mouseover event on an element.
    async fn hover(&self, selector: &str) -> AppResult<()> {
        let final_sel = selector.to_js_string();
        let js = format!(
            "const el = queryRecursive('{}'); \
             if (!el) throw new Error('Element not found'); \
             const ev = new MouseEvent('mouseover', {{ bubbles: true }}); \
             el.dispatchEvent(ev); \
             return true;", final_sel
        );
        let _ = self.run_js_internal(&js).await?;
        Ok(())
    }

    /// Evaluates a raw JS snippet in the page context.
    async fn eval(&self, js: &str) -> AppResult<String> {
        self.run_js_internal(js).await
    }

    /// Captures a full-page screenshot as a PNG.
    async fn screenshot(&self) -> AppResult<Vec<u8>> {
        self.page.screenshot(chromiumoxide::page::ScreenshotParams::builder().full_page(true).build())
            .await
            .map_err(|e| AppError::Browser(e.to_string()))
    }

    /// Waits until the page navigation is committed.
    async fn wait_for_navigation(&self) -> AppResult<()> {
        let _ = self.page.wait_for_navigation().await;
        Ok(())
    }
}
