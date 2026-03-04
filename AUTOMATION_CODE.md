## src/core/automation/mod.rs
```
pub mod dsl;
pub mod engine;
pub mod context;

pub use engine::AutomationEngine;
pub use dsl::{AutomationDsl, Step, Condition};

```

## src/core/automation/dsl.rs
```
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationDsl {
    pub dsl_version: u32,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Step {
    Navigate { url: String },
    Click { selector: String },
    RightClick { selector: String },
    Hover { selector: String },
    Type { selector: String, value: String, is_variable: bool },
    Wait { seconds: u64 },
    WaitSelector { selector: String, timeout_ms: u64 },
    WaitUntilIdle { timeout_ms: u64 },
    WaitNetworkIdle { timeout_ms: u64, min_idle_ms: u64 },
    Extract { selector: String, as_key: String, add_to_row: bool },
    SetVariable { key: String, value: String },
    NewRow,
    Export { filename: String },
    Screenshot { filename: String },
    ScrollBottom,
    SwitchFrame { selector: String }, // Empty for main frame
    If {
        selector: String,
        then_steps: Vec<Step>,
    },
    ForEach {
        selector: String,
        body: Vec<Step>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Condition {
    Exists { selector: String },
    TextContains { selector: String, value: String },
}

```

## src/core/automation/context.rs
```
use std::collections::HashMap;

pub struct AutomationContext {
    pub variables: HashMap<String, String>,
    pub current_row: HashMap<String, String>,
    pub extracted_data: Vec<HashMap<String, String>>,
    pub current_step: usize,
    pub port: u16,
    pub tab_id: String,
    pub output_dir: std::path::PathBuf,
}

impl AutomationContext {
    pub fn new(port: u16, tab_id: String, output_dir: std::path::PathBuf) -> Self {
        Self {
            variables: HashMap::new(),
            current_row: HashMap::new(),
            extracted_data: Vec::new(),
            current_step: 0,
            port,
            tab_id,
            output_dir,
        }
    }

    pub fn push_current_row(&mut self) {
        if !self.current_row.is_empty() {
            self.extracted_data.push(self.current_row.clone());
            self.current_row.clear();
        }
    }
}

```

## src/core/automation/engine.rs
```
use crate::core::automation::dsl::{AutomationDsl, Step};
use crate::core::automation::context::AutomationContext;
use crate::core::error::{AppError, AppResult};
use crate::core::events::AppEvent;
use crate::ui::scrape::emit;
use std::sync::{Arc, Mutex};
use chromiumoxide::{Browser, Page};
use futures::StreamExt;

pub struct AutomationEngine {
    pub context: Arc<Mutex<AutomationContext>>,
}

impl AutomationEngine {
    pub fn new(port: u16, tab_id: String, output_dir: std::path::PathBuf) -> Self {
        Self {
            context: Arc::new(Mutex::new(AutomationContext::new(port, tab_id, output_dir))),
        }
    }

    pub async fn run(&mut self, dsl: AutomationDsl) -> AppResult<()> {
        let (port, tid, output_dir) = {
            let ctx = self.context.lock().unwrap();
            (ctx.port, ctx.tab_id.clone(), ctx.output_dir.clone())
        };

        tracing::info!("[AUTO-ENGINE] Connecting to browser for pipeline...");
        
        let ws_url = crate::core::browser::BrowserManager::get_ws_url(port).await?;
        let (browser, mut handler) = Browser::connect(ws_url).await.map_err(|e| AppError::Browser(e.to_string()))?;
        let _handler_job = tokio::spawn(async move { while let Some(_) = handler.next().await {} });
        
        let mut page = None;
        for _ in 0..15 {
            if let Ok(pages) = browser.pages().await {
                if let Some(p) = pages.into_iter().find(|p| p.target_id().as_ref() == tid) {
                    page = Some(p);
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }

        let page = page.ok_or_else(|| AppError::NotFound(format!("Target page {} not found", tid)))?;
        
        page.execute(chromiumoxide::cdp::browser_protocol::page::EnableParams::default()).await.map_err(|e| AppError::Browser(e.to_string()))?;
        let mut dialog_events = page.event_listener::<chromiumoxide::cdp::browser_protocol::page::EventJavascriptDialogOpening>().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let page_for_dialog = page.clone();
        tokio::spawn(async move {
            while let Some(_) = dialog_events.next().await {
                let _ = page_for_dialog.execute(chromiumoxide::cdp::browser_protocol::page::HandleJavaScriptDialogParams::builder().accept(true).build().unwrap()).await;
            }
        });

        let steps = dsl.steps.clone();

        for (idx, step) in steps.iter().enumerate() {
            {
                let mut ctx = self.context.lock().unwrap();
                ctx.current_step = idx;
                emit(AppEvent::AutomationProgress(ctx.tab_id.clone(), idx));
            }
            
            match self.execute_step_internal(step, &page).await {
                Ok(_) => {
                    tracing::debug!("[AUTO-ENGINE] Step {} completed.", idx + 1);
                }
                Err(e) => {
                    tracing::error!("[AUTO-ENGINE] Step {} failed: {}", idx + 1, e);
                    
                    let ts = chrono::Local::now().format("%H%M%S").to_string();
                    let filename = format!("FAIL_STEP_{}_{}.png", idx + 1, ts);
                    let full_path = output_dir.join(&filename);
                    
                    if let Ok(data) = page.screenshot(chromiumoxide::page::ScreenshotParams::builder().full_page(true).build()).await {
                        let _ = std::fs::write(&full_path, data);
                        tracing::warn!("[AUTO-ENGINE] Failure screenshot saved to {:?}", full_path);
                        emit(AppEvent::OperationError(format!("Failure! Saved to: {}", filename)));
                    }

                    emit(AppEvent::AutomationError(tid.clone(), e.to_string()));
                    return Err(e);
                }
            }
        }

        emit(AppEvent::AutomationFinished(tid));
        tracing::info!("[AUTO-ENGINE] Pipeline finished successfully.");
        Ok(())
    }

    fn interpolate(&self, text: &str) -> String {
        let ctx = self.context.lock().unwrap();
        let mut result = text.to_string();
        for (key, val) in &ctx.variables {
            let placeholder = format!("{{{{{}}}}}", key);
            result = result.replace(&placeholder, val);
        }
        result
    }

    async fn run_js(&self, page: &Page, script: String) -> AppResult<String> {
        let wrapped_js = format!(
            "(() => {{ try {{ \
                const result = (async () => {{ {} }})(); \
                return Promise.resolve(result).then(r => JSON.stringify({{ success: true, data: r }})); \
            }} catch (e) {{ \
                return JSON.stringify({{ success: false, error: e.message }}); \
            }} }})()", script
        );

        let result = page.evaluate(wrapped_js).await.map_err(|e| AppError::Browser(e.to_string()))?;
        let val_str = result.value().clone().cloned().unwrap_or_default().to_string();
        
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&val_str) {
            if json["success"].as_bool() == Some(false) {
                let err_msg = json["error"].as_str().unwrap_or("Unknown JS error");
                return Err(AppError::Browser(format!("JS Error: {}", err_msg)));
            }
            let data = json["data"].to_string();
            return Ok(data.trim_matches('"').to_string());
        }
        Ok(val_str)
    }

    fn execute_step_internal<'a>(&'a self, step: &'a Step, page: &'a Page) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<()>> + Send + 'a>> {
        Box::pin(async move {
            let (_tid, output_dir) = { 
                let ctx = self.context.lock().unwrap();
                (ctx.tab_id.clone(), ctx.output_dir.clone())
            };

            match step {
                Step::Navigate { url } => {
                    let final_url = self.interpolate(url);
                    page.goto(final_url).await.map_err(|e| AppError::Browser(e.to_string()))?;
                }
                Step::Click { selector } => {
                    let final_sel = self.interpolate(selector).replace("'", "\\'");
                    let js = format!(
                        "const el = document.querySelector('{}'); \
                         if (!el) throw new Error('Element not found'); \
                         el.style.outline = '3px solid #ff00ff'; \
                         el.scrollIntoView({{behavior: 'instant', block: 'center'}}); \
                         return true;", final_sel
                    );
                    self.run_js(page, js).await?;
                    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                    let el = page.find_element(&final_sel).await.map_err(|e| AppError::Browser(e.to_string()))?;
                    el.click().await.map_err(|e| AppError::Browser(e.to_string()))?;
                }
                Step::RightClick { selector } => {
                    let final_sel = self.interpolate(selector).replace("'", "\\'");
                    let js = format!(
                        "const el = document.querySelector('{}'); \
                         if (!el) throw new Error('Element not found'); \
                         const ev = new MouseEvent('contextmenu', {{ bubbles: true, cancelable: true, view: window, button: 2 }}); \
                         el.dispatchEvent(ev); \
                         return true;", final_sel
                    );
                    let _ = self.run_js(page, js).await?;
                }
                Step::Hover { selector } => {
                    let final_sel = self.interpolate(selector).replace("'", "\\'");
                    let el = page.find_element(&final_sel).await.map_err(|e| AppError::Browser(e.to_string()))?;
                    el.hover().await.map_err(|e| AppError::Browser(e.to_string()))?;
                }
                Step::Type { selector, value, .. } => {
                    let final_sel = self.interpolate(selector);
                    let final_val = self.interpolate(value);
                    let highlight_js = format!(
                        "(() => {{ \
                            const el = document.querySelector('{}'); \
                            if (!el) throw new Error('Input not found'); \
                            el.style.outline = '3px solid #00ffff'; \
                            el.scrollIntoView({{behavior: 'instant', block: 'center'}}); \
                            return true; \
                        }})()", final_sel.replace("'", "\\'")
                    );
                    self.run_js(page, highlight_js).await?;
                    let el = page.find_element(&final_sel).await.map_err(|e| AppError::Browser(e.to_string()))?;
                    el.click().await.map_err(|e| AppError::Browser(e.to_string()))?;
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    el.type_str(final_val).await.map_err(|e| AppError::Browser(e.to_string()))?;
                }
                Step::Wait { seconds } => {
                    tokio::time::sleep(std::time::Duration::from_secs(*seconds)).await;
                }
                Step::WaitSelector { selector, timeout_ms } => {
                    let final_sel = self.interpolate(selector).replace("'", "\\'");
                    let timeout = *timeout_ms;
                    let js = format!(
                        "return new Promise((resolve, reject) => {{ \
                            const check = () => {{ \
                                const el = document.querySelector('{}'); \
                                if (el) {{ el.style.outline = '2px dashed #00ff00'; resolve(true); return true; }} \
                                return false; \
                            }}; \
                            if (check()) return; \
                            const start = Date.now(); \
                            const timer = setInterval(() => {{ \
                                if (check()) {{ clearInterval(timer); }} \
                                if (Date.now() - start > {}) {{ clearInterval(timer); reject('Timeout waiting for element'); }} \
                            }}, 200); \
                        }})", final_sel, timeout
                    );
                    self.run_js(page, js).await?;
                }
                Step::Extract { selector, as_key, add_to_row } => {
                    let final_sel = self.interpolate(selector).replace("'", "\\'");
                    let js = format!(
                        "const el = document.querySelector('{}'); \
                         if (!el) return 'NOT_FOUND'; \
                         el.style.backgroundColor = 'rgba(0, 255, 0, 0.2)'; \
                         return el.innerText || el.value || '';", final_sel
                    );
                    let text = self.run_js(page, js).await?;
                    if text != "NOT_FOUND" {
                        let (tid_clone, current_rows) = {
                            let mut ctx = self.context.lock().unwrap();
                            ctx.variables.insert(as_key.clone(), text.clone());
                            if *add_to_row {
                                ctx.current_row.insert(as_key.clone(), text.clone());
                            }
                            (ctx.tab_id.clone(), ctx.extracted_data.clone())
                        };
                        emit(AppEvent::ConsoleLogAdded(tid_clone.clone(), format!("[DATA] {}: {}", as_key, text)));
                        emit(AppEvent::AutomationDatasetUpdated(tid_clone, current_rows));
                    } else {
                        return Err(AppError::Browser(format!("Element not found: {}", final_sel)));
                    }
                }
                Step::NewRow => {
                    let (tid_clone, current_rows) = {
                        let mut ctx = self.context.lock().unwrap();
                        ctx.push_current_row();
                        (ctx.tab_id.clone(), ctx.extracted_data.clone())
                    };
                    emit(AppEvent::AutomationDatasetUpdated(tid_clone, current_rows));
                }
                Step::Export { filename } => {
                    let final_name = self.interpolate(filename);
                    let full_path = output_dir.join(&final_name);
                    let data = {
                        let mut ctx = self.context.lock().unwrap();
                        ctx.push_current_row();
                        ctx.extracted_data.clone()
                    };
                    if !data.is_empty() {
                        if let Ok(json) = serde_json::to_string_pretty(&data) {
                            let _ = std::fs::write(full_path, json);
                        }
                    }
                }
                Step::Screenshot { filename } => {
                    let final_name = self.interpolate(filename);
                    let full_path = output_dir.join(&final_name);
                    if let Ok(data) = page.screenshot(chromiumoxide::page::ScreenshotParams::builder().full_page(true).build()).await {
                        let _ = std::fs::write(full_path, data);
                    }
                }
                Step::WaitUntilIdle { timeout_ms } => {
                    let _ = page.wait_for_navigation().await;
                    tokio::time::sleep(std::time::Duration::from_millis(*timeout_ms)).await;
                }
                Step::WaitNetworkIdle { timeout_ms, min_idle_ms } => {
                    use chromiumoxide::cdp::browser_protocol::network::EnableParams;
                    let _ = page.execute(EnableParams::default()).await;
                    let mut request_events = page.event_listener::<chromiumoxide::cdp::browser_protocol::network::EventRequestWillBeSent>().await.map_err(|e| AppError::Browser(e.to_string()))?;
                    let mut response_events = page.event_listener::<chromiumoxide::cdp::browser_protocol::network::EventLoadingFinished>().await.map_err(|e| AppError::Browser(e.to_string()))?;
                    let mut failed_events = page.event_listener::<chromiumoxide::cdp::browser_protocol::network::EventLoadingFailed>().await.map_err(|e| AppError::Browser(e.to_string()))?;

                    let start = std::time::Instant::now();
                    let mut last_activity = std::time::Instant::now();
                    let mut active_requests: i32 = 0;

                    loop {
                        if start.elapsed().as_millis() > *timeout_ms as u128 { break; }
                        if active_requests == 0 && last_activity.elapsed().as_millis() > *min_idle_ms as u128 { break; }

                        tokio::select! {
                            _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {}
                            Some(_) = request_events.next() => { active_requests += 1; last_activity = std::time::Instant::now(); }
                            Some(_) = response_events.next() => { active_requests = active_requests.saturating_sub(1); last_activity = std::time::Instant::now(); }
                            Some(_) = failed_events.next() => { active_requests = active_requests.saturating_sub(1); last_activity = std::time::Instant::now(); }
                        }
                    }
                }
                Step::SetVariable { key, value } => {
                    let final_val = self.interpolate(value);
                    let mut ctx = self.context.lock().unwrap();
                    ctx.variables.insert(key.clone(), final_val);
                }
                Step::ScrollBottom => {
                    self.run_js(page, "window.scrollTo(0, document.body.scrollHeight)".into()).await?;
                }
                Step::SwitchFrame { selector } => {
                    if selector.is_empty() {
                        let _ = page.mainframe();
                    } else {
                        let final_sel = self.interpolate(selector).replace("'", "\\'");
                        let js = format!("document.querySelector('{}').contentWindow.focus()", final_sel);
                        let _ = self.run_js(page, js).await?;
                    }
                }
                Step::If { selector, then_steps } => {
                    let final_sel = self.interpolate(selector).replace("'", "\\'");
                    let res = self.run_js(page, format!("!!document.querySelector('{}')", final_sel)).await?;
                    if res == "true" {
                        for s in then_steps { self.execute_step_internal(s, page).await?; }
                    }
                }
                Step::ForEach { selector, body } => {
                    let final_sel = self.interpolate(selector).replace("'", "\\'");
                    let count_str = self.run_js(page, format!("document.querySelectorAll('{}').length", final_sel)).await?;
                    let count = count_str.parse::<usize>().unwrap_or(0);
                    for i in 0..count {
                        {
                            let mut ctx = self.context.lock().unwrap();
                            ctx.variables.insert("index".into(), i.to_string());
                            ctx.variables.insert("item".into(), format!("{}:nth-child({})", final_sel, i + 1));
                        }
                        for s in body { self.execute_step_internal(s, page).await?; }
                    }
                }
            }
            
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            Ok(())
        })
    }
}

```

