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
    pub fn new(port: u16, tab_id: String) -> Self {
        Self {
            context: Arc::new(Mutex::new(AutomationContext::new(port, tab_id))),
        }
    }

    pub async fn run(&mut self, dsl: AutomationDsl) -> AppResult<()> {
        let (port, tid) = {
            let ctx = self.context.lock().unwrap();
            (ctx.port, ctx.tab_id.clone())
        };

        tracing::info!("[AUTO-ENGINE] Connecting to browser for pipeline...");
        
        // Single persistent connection for the whole pipeline
        let ws_url = crate::core::browser::BrowserManager::get_ws_url(port).await?;
        let (browser, mut handler) = Browser::connect(ws_url).await.map_err(|e| AppError::Browser(e.to_string()))?;
        let _handler_job = tokio::spawn(async move { while let Some(_) = handler.next().await {} });
        
        // Wait for page attachment
        let mut page = None;
        for _ in 0..10 {
            if let Ok(pages) = browser.pages().await {
                if let Some(p) = pages.into_iter().find(|p| p.target_id().as_ref() == tid) {
                    page = Some(p);
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        let page = page.ok_or_else(|| AppError::NotFound(format!("Target page {} not found", tid)))?;
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
        // Robust JS runner with error handling
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
            let tid = { self.context.lock().unwrap().tab_id.clone() };

            match step {
                Step::Navigate { url } => {
                    let final_url = self.interpolate(url);
                    page.goto(final_url).await.map_err(|e| AppError::Browser(e.to_string()))?;
                }
                Step::Click { selector } => {
                    let final_sel = self.interpolate(selector);
                    let js = format!(
                        "const el = document.querySelector('{}'); \
                         if (!el) throw new Error('Element not found: {}'); \
                         el.scrollIntoView({{behavior: 'smooth', block: 'center'}}); \
                         el.click();", final_sel, final_sel
                    );
                    self.run_js(page, js).await?;
                }
                Step::Type { selector, value } => {
                    let final_sel = self.interpolate(selector);
                    let final_val = self.interpolate(value);
                    
                    // Most robust way to type in modern frameworks (React/Vue/Angular)
                    let js = format!(
                        "(() => {{ \
                            const el = document.querySelector('{}'); \
                            if (!el) throw new Error('Element not found: {}'); \
                            el.focus(); \
                            const nativeValueSetter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value').set; \
                            const textAreaValueSetter = Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, 'value').set; \
                            if (el.tagName === 'TEXTAREA' && textAreaValueSetter) {{ \
                                textAreaValueSetter.call(el, '{}'); \
                            }} else if (nativeValueSetter) {{ \
                                nativeValueSetter.call(el, '{}'); \
                            }} else {{ \
                                el.value = '{}'; \
                            }} \
                            el.dispatchEvent(new Event('input', {{ bubbles: true }})); \
                            el.dispatchEvent(new Event('change', {{ bubbles: true }})); \
                            el.dispatchEvent(new Event('blur', {{ bubbles: true }})); \
                            return true; \
                        }})()", 
                        final_sel, final_sel, 
                        final_val.replace("'", "\\'"), 
                        final_val.replace("'", "\\'"), 
                        final_val.replace("'", "\\'")
                    );
                    self.run_js(page, js).await?;
                }
                Step::WaitFor { selector, timeout_ms } => {
                    let final_sel = self.interpolate(selector);
                    let timeout = timeout_ms.unwrap_or(5000);
                    let js = format!(
                        "return new Promise((resolve, reject) => {{ \
                            const check = () => {{ \
                                const el = document.querySelector('{}'); \
                                if (el) {{ resolve('found'); return true; }} \
                                return false; \
                            }}; \
                            if (check()) return; \
                            const start = Date.now(); \
                            const timer = setInterval(() => {{ \
                                if (check()) {{ clearInterval(timer); }} \
                                if (Date.now() - start > {}) {{ clearInterval(timer); reject('Timeout waiting for: {}'); }} \
                            }}, 200); \
                        }})", final_sel, timeout, final_sel
                    );
                    self.run_js(page, js).await?;
                }
                Step::Extract { selector, as_key, add_to_row } => {
                    let final_sel = self.interpolate(selector);
                    let js = format!(
                        "const el = document.querySelector('{}'); \
                         return el ? (el.innerText || el.value || '') : 'NOT_FOUND';", final_sel
                    );
                    let text = self.run_js(page, js).await?;
                    if text != "NOT_FOUND" {
                        let mut ctx = self.context.lock().unwrap();
                        ctx.variables.insert(as_key.clone(), text.clone());
                        if add_to_row.unwrap_or(true) {
                            ctx.current_row.insert(as_key.clone(), text.clone());
                        }
                        emit(AppEvent::ConsoleLogAdded(tid, format!("[DATA] {}: {}", as_key, text)));
                    } else {
                        return Err(AppError::Browser(format!("Element not found for extraction: {}", final_sel)));
                    }
                }
                Step::NewRow => {
                    let mut ctx = self.context.lock().unwrap();
                    ctx.push_current_row();
                }
                Step::Export { filename } => {
                    let final_name = self.interpolate(filename);
                    let data = {
                        let mut ctx = self.context.lock().unwrap();
                        ctx.push_current_row();
                        ctx.extracted_data.clone()
                    };
                    if !data.is_empty() {
                        if let Ok(json) = serde_json::to_string_pretty(&data) {
                            let _ = std::fs::write(&final_name, json);
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
                Step::If { condition, then_steps, else_steps } => {
                    if self.evaluate_condition_internal(condition, page).await? {
                        for s in then_steps { self.execute_step_internal(s, page).await?; }
                    } else if let Some(steps) = else_steps {
                        for s in steps { self.execute_step_internal(s, page).await?; }
                    }
                }
                Step::ForEach { selector, body } => {
                    let final_sel = self.interpolate(selector);
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

    async fn evaluate_condition_internal(&self, condition: &crate::core::automation::dsl::Condition, page: &Page) -> AppResult<bool> {
        match condition {
            crate::core::automation::dsl::Condition::Exists { selector } => {
                let final_sel = self.interpolate(selector);
                let res = self.run_js(page, format!("!!document.querySelector('{}')", final_sel)).await?;
                Ok(res == "true")
            }
            crate::core::automation::dsl::Condition::TextContains { selector, value } => {
                let final_sel = self.interpolate(selector);
                let final_val = self.interpolate(value);
                let js = format!(
                    "(() => {{ \
                        const el = document.querySelector('{}'); \
                        return el && el.innerText.includes('{}'); \
                    }})()", final_sel, final_val
                );
                let res = self.run_js(page, js).await?;
                Ok(res == "true")
            }
        }
    }
}
