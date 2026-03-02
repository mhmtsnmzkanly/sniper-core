use crate::core::automation::dsl::{AutomationDsl, Step};
use crate::core::automation::context::AutomationContext;
use crate::core::error::AppResult;
use crate::core::events::AppEvent;
use crate::ui::scrape::emit;
use std::sync::{Arc, Mutex};

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
        tracing::info!("[AUTO-ENGINE] Starting execution of DSL v{}", dsl.dsl_version);
        
        let steps = dsl.steps.clone();
        for (idx, step) in steps.iter().enumerate() {
            {
                let mut ctx = self.context.lock().unwrap();
                ctx.current_step = idx;
                emit(AppEvent::AutomationProgress(ctx.tab_id.clone(), idx));
            }
            
            match self.execute_step(step).await {
                Ok(_) => {
                    tracing::debug!("[AUTO-ENGINE] Step {} completed.", idx + 1);
                }
                Err(e) => {
                    let ctx = self.context.lock().unwrap();
                    tracing::error!("[AUTO-ENGINE] Step {} failed: {}", idx + 1, e);
                    emit(AppEvent::AutomationError(ctx.tab_id.clone(), e.to_string()));
                    return Err(e);
                }
            }
        }

        let ctx = self.context.lock().unwrap();
        emit(AppEvent::AutomationFinished(ctx.tab_id.clone()));
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

    fn execute_step<'a>(&'a self, step: &'a Step) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<()>> + Send + 'a>> {
        Box::pin(async move {
            use crate::core::browser::BrowserManager;
            let (port, tid) = {
                let ctx = self.context.lock().unwrap();
                (ctx.port, ctx.tab_id.clone())
            };

            match step {
                Step::Navigate { url } => {
                    let final_url = self.interpolate(url);
                    let js = format!("window.location.href = '{}'", final_url);
                    BrowserManager::execute_script(port, tid, js).await?;
                }
                Step::Click { selector } => {
                    let final_sel = self.interpolate(selector);
                    let js = format!(
                        "(() => {{ \
                            const el = document.querySelector('{}'); \
                            if (!el) throw new Error('Element not found: {}'); \
                            el.click(); \
                            return true; \
                        }})()", final_sel, final_sel
                    );
                    BrowserManager::execute_script(port, tid, js).await?;
                }
                Step::Type { selector, value } => {
                    let final_sel = self.interpolate(selector);
                    let final_val = self.interpolate(value);
                    let js = format!(
                        "(() => {{ \
                            const el = document.querySelector('{}'); \
                            if (!el) throw new Error('Element not found: {}'); \
                            el.value = '{}'; \
                            el.dispatchEvent(new Event('input', {{ bubbles: true }})); \
                            el.dispatchEvent(new Event('change', {{ bubbles: true }})); \
                            return true; \
                        }})()", final_sel, final_sel, final_val
                    );
                    BrowserManager::execute_script(port, tid, js).await?;
                }
                Step::WaitFor { selector, timeout_ms } => {
                    let final_sel = self.interpolate(selector);
                    let timeout = timeout_ms.unwrap_or(5000);
                    let js = format!(
                        "new Promise((resolve, reject) => {{ \
                            const check = () => {{ \
                                const el = document.querySelector('{}'); \
                                if (el) {{ resolve(true); return true; }} \
                                return false; \
                            }}; \
                            if (check()) return; \
                            const start = Date.now(); \
                            const timer = setInterval(() => {{ \
                                if (check()) {{ clearInterval(timer); }} \
                                if (Date.now() - start > {}) {{ clearInterval(timer); reject('Timeout waiting for element: {}'); }} \
                            }}, 200); \
                        }})", final_sel, timeout, final_sel
                    );
                    BrowserManager::execute_script(port, tid, js).await?;
                }
                Step::Extract { selector, as_key, add_to_row } => {
                    let final_sel = self.interpolate(selector);
                    let js = format!(
                        "(() => {{ \
                            const el = document.querySelector('{}'); \
                            if (!el) return 'NOT_FOUND'; \
                            return el.innerText || el.value || ''; \
                        }})()", final_sel
                    );
                    match BrowserManager::execute_script(port, tid.clone(), js).await {
                        Ok(text) => {
                            let clean_text = text.trim_matches('"').to_string();
                            if clean_text == "NOT_FOUND" {
                                tracing::warn!("[AUTO-ENGINE] Element not found for extraction: {}", final_sel);
                            } else {
                                tracing::info!("[AUTO-ENGINE] Extracted '{}': {}", as_key, clean_text);
                                {
                                    let mut ctx = self.context.lock().unwrap();
                                    ctx.variables.insert(as_key.clone(), clean_text.clone());
                                    if add_to_row.unwrap_or(true) {
                                        ctx.current_row.insert(as_key.clone(), clean_text.clone());
                                    }
                                }
                                emit(AppEvent::ConsoleLogAdded(tid, format!("[DATA] {}: {}", as_key, clean_text)));
                            }
                        },
                        Err(e) => tracing::warn!("[AUTO-ENGINE] Extraction failed: {}", e),
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
                        ctx.push_current_row(); // Flush last row if any
                        ctx.extracted_data.clone()
                    };
                    
                    if !data.is_empty() {
                        if let Ok(json) = serde_json::to_string_pretty(&data) {
                            let path = std::path::PathBuf::from(&final_name);
                            let _ = std::fs::write(path, json);
                            tracing::info!("[AUTO-ENGINE] Exported {} rows to {}", data.len(), final_name);
                        }
                    }
                }
                Step::SetVariable { key, value } => {
                    let final_val = self.interpolate(value);
                    let mut ctx = self.context.lock().unwrap();
                    ctx.variables.insert(key.clone(), final_val);
                }
                Step::ScrollBottom => {
                    let js = "window.scrollTo(0, document.body.scrollHeight)".to_string();
                    BrowserManager::execute_script(port, tid, js).await?;
                }
                Step::If { condition, then_steps, else_steps } => {
                    if self.evaluate_condition(condition).await? {
                        for s in then_steps { self.execute_step(s).await?; }
                    } else if let Some(steps) = else_steps {
                        for s in steps { self.execute_step(s).await?; }
                    }
                }
                Step::ForEach { selector, body } => {
                    let final_sel = self.interpolate(selector);
                    // Get element count
                    let count_js = format!("document.querySelectorAll('{}').length", final_sel);
                    let count_str = BrowserManager::execute_script(port, tid.clone(), count_js).await?;
                    let count = count_str.parse::<usize>().unwrap_or(0);
                    
                    for i in 0..count {
                        // For each element, we set a temporary variable 'index' and 'item_selector'
                        {
                            let mut ctx = self.context.lock().unwrap();
                            ctx.variables.insert("index".into(), i.to_string());
                            ctx.variables.insert("item".into(), format!("{}:nth-child({})", final_sel, i + 1));
                        }
                        for s in body { self.execute_step(s).await?; }
                    }
                }
            }
            
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            Ok(())
        })
    }

    async fn evaluate_condition(&self, condition: &crate::core::automation::dsl::Condition) -> AppResult<bool> {
        use crate::core::browser::BrowserManager;
        let (port, tid) = {
            let ctx = self.context.lock().unwrap();
            (ctx.port, ctx.tab_id.clone())
        };

        match condition {
            crate::core::automation::dsl::Condition::Exists { selector } => {
                let final_sel = self.interpolate(selector);
                let js = format!("!!document.querySelector('{}')", final_sel);
                let res = BrowserManager::execute_script(port, tid, js).await?;
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
                let res = BrowserManager::execute_script(port, tid, js).await?;
                Ok(res == "true")
            }
        }
    }
}
