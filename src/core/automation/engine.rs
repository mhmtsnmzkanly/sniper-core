use crate::core::automation::dsl::{AutomationDsl, Step};
use crate::core::automation::context::AutomationContext;
use crate::core::automation::driver::{AutomationDriver};
use crate::core::automation::cdp_driver::CdpDriver;
use crate::core::error::{AppError, AppResult};
use crate::core::events::AppEvent;
use crate::ui::scrape::emit;
use std::sync::{Arc, Mutex};
use chromiumoxide::{Browser};
use futures::StreamExt;
use std::time::Instant;

pub struct ExecutionConfig {
    pub step_timeout: std::time::Duration,
    pub retry_attempts: u32,
    pub screenshot_on_error: bool,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            step_timeout: std::time::Duration::from_secs(30),
            retry_attempts: 0,
            screenshot_on_error: true,
        }
    }
}

pub struct AutomationEngine {
    pub context: Arc<Mutex<AutomationContext>>,
    pub config: ExecutionConfig,
}

impl AutomationEngine {
    pub fn new(port: u16, tab_id: String, output_dir: std::path::PathBuf) -> Self {
        Self {
            context: Arc::new(Mutex::new(AutomationContext::new(port, tab_id, output_dir))),
            config: ExecutionConfig::default(),
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
        
        // Setup page
        page.execute(chromiumoxide::cdp::browser_protocol::page::EnableParams::default()).await.map_err(|e| AppError::Browser(e.to_string()))?;
        let mut dialog_events = page.event_listener::<chromiumoxide::cdp::browser_protocol::page::EventJavascriptDialogOpening>().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let page_for_dialog = page.clone();
        tokio::spawn(async move {
            while let Some(_) = dialog_events.next().await {
                let _ = page_for_dialog.execute(chromiumoxide::cdp::browser_protocol::page::HandleJavaScriptDialogParams::builder().accept(true).build().unwrap()).await;
            }
        });

        // Initialize Driver
        let driver = Box::new(CdpDriver::new(page));

        let steps = dsl.steps.clone();

        for (idx, step) in steps.iter().enumerate() {
            let start_time = Instant::now();
            {
                let mut ctx = self.context.lock().unwrap();
                ctx.current_step = idx;
                emit(AppEvent::AutomationProgress(ctx.tab_id.clone(), idx));
            }
            
            tracing::info!("[AUTO-ENGINE] Executing Step {}: {:?}", idx + 1, step);

            let mut attempts = 0;
            let max_attempts = self.config.retry_attempts + 1;
            let mut last_error = None;

            while attempts < max_attempts {
                match self.execute_step_internal(step, driver.as_ref()).await {
                    Ok(_) => {
                        let duration = start_time.elapsed();
                        tracing::info!("[AUTO-ENGINE] Step {} completed in {:?} (Attempts: {})", idx + 1, duration, attempts + 1);
                        last_error = None;
                        break;
                    }
                    Err(e) => {
                        attempts += 1;
                        tracing::warn!("[AUTO-ENGINE] Step {} attempt {} failed: {}", idx + 1, attempts, e);
                        last_error = Some(e);
                        if attempts < max_attempts {
                            tokio::time::sleep(std::time::Duration::from_millis(500 * attempts as u64)).await;
                        }
                    }
                }
            }

            if let Some(e) = last_error {
                tracing::error!("[AUTO-ENGINE] Step {} failed after {} attempts: {}", idx + 1, max_attempts, e);
                
                if self.config.screenshot_on_error {
                    let ts = chrono::Local::now().format("%H%M%S").to_string();
                    let filename = format!("FAIL_STEP_{}_{}.png", idx + 1, ts);
                    let full_path = output_dir.join(&filename);
                    
                    if let Ok(data) = driver.screenshot().await {
                        let _ = std::fs::write(&full_path, data);
                        tracing::warn!("[AUTO-ENGINE] Failure screenshot saved to {:?}", full_path);
                        emit(AppEvent::OperationError(format!("Failure! Saved to: {}", filename)));
                    }
                }

                emit(AppEvent::AutomationError(tid.clone(), e.to_string()));
                return Err(e);
            }
        }

        emit(AppEvent::AutomationFinished(tid));
        tracing::info!("[AUTO-ENGINE] Pipeline finished successfully.");
        Ok(())
    }

    fn interpolate(&self, text: &str) -> String {
        let ctx = self.context.lock().unwrap();
        
        // Find all {{variable}} patterns
        let re = regex::Regex::new(r"\{\{([^}]+)\}\}").unwrap();
        
        let mut final_result = text.to_string();
        for cap in re.captures_iter(text) {
            let full_match = &cap[0];
            let var_name = cap[1].trim();
            if let Some(val) = ctx.get_variable(var_name) {
                final_result = final_result.replace(full_match, &val);
            }
        }
        final_result
    }

    fn execute_step_internal<'a>(&'a self, step: &'a Step, driver: &'a dyn AutomationDriver) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<()>> + Send + 'a>> {
        Box::pin(async move {
            let (_tid, output_dir) = { 
                let ctx = self.context.lock().unwrap();
                (ctx.tab_id.clone(), ctx.output_dir.clone())
            };

            match step {
                Step::Navigate { url } => {
                    let final_url = self.interpolate(url);
                    driver.navigate(&final_url).await?;
                }
                Step::Click { selector } => {
                    let final_sel = self.interpolate(selector);
                    driver.click(&final_sel).await?;
                }
                Step::RightClick { selector } => {
                    let final_sel = self.interpolate(selector);
                    let js = format!(
                        "const el = document.querySelector('{}'); \
                         if (!el) throw new Error('Element not found'); \
                         const ev = new MouseEvent('contextmenu', {{ bubbles: true, cancelable: true, view: window, button: 2 }}); \
                         el.dispatchEvent(ev); \
                         return true;", final_sel.replace("'", "\\'")
                    );
                    let _: String = driver.eval(&js).await?;
                }
                Step::Hover { selector } => {
                    let final_sel = self.interpolate(selector);
                    driver.hover(&final_sel).await?;
                }
                Step::Type { selector, value, .. } => {
                    let final_sel = self.interpolate(selector);
                    let final_val = self.interpolate(value);
                    driver.type_text(&final_sel, &final_val).await?;
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
                    let _: String = driver.eval(&js).await?;
                }
                Step::Extract { selector, as_key, add_to_row } => {
                    let final_sel = self.interpolate(selector).replace("'", "\\'");
                    let js = format!(
                        "const el = document.querySelector('{}'); \
                         if (!el) return 'NOT_FOUND'; \
                         el.style.backgroundColor = 'rgba(0, 255, 0, 0.2)'; \
                         return el.innerText || el.value || '';", final_sel
                    );
                    let text: String = driver.eval(&js).await?;
                    if text != "NOT_FOUND" {
                        let (tid_clone, current_rows) = {
                            let mut ctx = self.context.lock().unwrap();
                            ctx.set_variable(as_key.clone(), text.clone());
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
                    if let Ok(data) = driver.screenshot().await {
                        let _ = std::fs::write(full_path, data);
                    }
                }
                Step::WaitUntilIdle { timeout_ms } => {
                    driver.wait_for_navigation().await?;
                    tokio::time::sleep(std::time::Duration::from_millis(*timeout_ms)).await;
                }
                Step::WaitNetworkIdle { timeout_ms, .. } => {
                    tokio::time::sleep(std::time::Duration::from_millis(*timeout_ms)).await;
                }
                Step::SetVariable { key, value } => {
                    let final_val = self.interpolate(value);
                    let mut ctx = self.context.lock().unwrap();
                    ctx.set_variable(key.clone(), final_val);
                }
                Step::ScrollBottom => {
                    let _: String = driver.eval("window.scrollTo(0, document.body.scrollHeight)").await?;
                }
                Step::SwitchFrame { selector } => {
                    let final_sel = self.interpolate(selector);
                    if !final_sel.is_empty() {
                        let js = format!("document.querySelector('{}').contentWindow.focus()", final_sel.replace("'", "\\'"));
                        let _: String = driver.eval(&js).await?;
                    }
                }
                Step::If { selector, then_steps } => {
                    let final_sel = self.interpolate(selector).replace("'", "\\'");
                    let res: String = driver.eval(&format!("!!document.querySelector('{}')", final_sel)).await?;
                    if res == "true" {
                        for s in then_steps { self.execute_step_internal(s, driver).await?; }
                    }
                }
                Step::ForEach { selector, body } => {
                    let final_sel = self.interpolate(selector).replace("'", "\\'");
                    let count_str: String = driver.eval(&format!("document.querySelectorAll('{}').length", final_sel)).await?;
                    let count = count_str.parse::<usize>().unwrap_or(0);
                    for i in 0..count {
                        {
                            let mut ctx = self.context.lock().unwrap();
                            ctx.push_scope();
                            ctx.set_variable("index".into(), i.to_string());
                            ctx.set_variable("item".into(), format!("{}:nth-child({})", final_sel, i + 1));
                        }
                        for s in body { self.execute_step_internal(s, driver).await?; }
                        {
                            let mut ctx = self.context.lock().unwrap();
                            ctx.pop_scope();
                        }
                    }
                }
            }
            
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            Ok(())
        })
    }
}
