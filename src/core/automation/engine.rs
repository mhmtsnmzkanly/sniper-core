use crate::core::automation::dsl::{AutomationDsl, Step};
use crate::core::automation::context::AutomationContext;
use crate::core::error::AppResult;
use crate::core::events::AppEvent;
use crate::ui::scrape::emit;

pub struct AutomationEngine {
    pub context: AutomationContext,
}

impl AutomationEngine {
    pub fn new(port: u16, tab_id: String) -> Self {
        Self {
            context: AutomationContext::new(port, tab_id),
        }
    }

    pub async fn run(&mut self, dsl: AutomationDsl) -> AppResult<()> {
        tracing::info!("[AUTO-ENGINE] Starting execution of DSL v{}", dsl.dsl_version);
        
        for (idx, step) in dsl.steps.iter().enumerate() {
            self.context.current_step = idx;
            emit(AppEvent::AutomationProgress(self.context.tab_id.clone(), idx));
            
            match self.execute_step(step).await {
                Ok(_) => {
                    tracing::debug!("[AUTO-ENGINE] Step {} completed.", idx + 1);
                }
                Err(e) => {
                    tracing::error!("[AUTO-ENGINE] Step {} failed: {}", idx + 1, e);
                    emit(AppEvent::AutomationError(self.context.tab_id.clone(), e.to_string()));
                    return Err(e);
                }
            }
        }

        emit(AppEvent::AutomationFinished(self.context.tab_id.clone()));
        tracing::info!("[AUTO-ENGINE] Pipeline finished successfully.");
        Ok(())
    }

    fn execute_step<'a>(&'a self, step: &'a Step) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<()>> + Send + 'a>> {
        Box::pin(async move {
            use crate::core::browser::BrowserManager;
            let port = self.context.port;
            let tid = self.context.tab_id.clone();

            match step {
                Step::Navigate { url } => {
                    let js = format!("window.location.href = '{}'", url);
                    BrowserManager::execute_script(port, tid, js).await?;
                }
                Step::Click { selector } => {
                    let js = format!("document.querySelector('{}').click()", selector);
                    BrowserManager::execute_script(port, tid, js).await?;
                }
                Step::Type { selector, value } => {
                    let js = format!("document.querySelector('{}').value = '{}'", selector, value);
                    BrowserManager::execute_script(port, tid, js).await?;
                }
                Step::WaitFor { selector, timeout_ms } => {
                    let timeout = timeout_ms.unwrap_or(5000);
                    let js = format!(
                        "new Promise((resolve, reject) => {{ 
                            const start = Date.now(); 
                            const timer = setInterval(() => {{ 
                                if (document.querySelector('{}')) {{ clearInterval(timer); resolve(true); }} 
                                if (Date.now() - start > {}) {{ clearInterval(timer); reject('Timeout waiting for {}'); }} 
                            }}, 100); 
                        }})", selector, timeout, selector
                    );
                    BrowserManager::execute_script(port, tid, js).await?;
                }
                Step::Extract { selector, as_key } => {
                    let js = format!(
                        "(() => {{ \
                            const el = document.querySelector('{}'); \
                            return el ? el.innerText : ''; \
                        }})()", selector
                    );
                    match BrowserManager::execute_script(port, tid.clone(), js).await {
                        Ok(text) => {
                            let clean_text = text.trim_matches('"').to_string();
                            tracing::info!("[AUTO-ENGINE] Extracted '{}': {}", as_key, clean_text);
                            emit(AppEvent::ConsoleLogAdded(tid, format!("[DATA] {}: {}", as_key, clean_text)));
                        },
                        Err(e) => tracing::warn!("[AUTO-ENGINE] Extraction failed: {}", e),
                    }
                }
                Step::ScrollBottom => {
                    let js = "window.scrollTo(0, document.body.scrollHeight)".to_string();
                    BrowserManager::execute_script(port, tid, js).await?;
                }
                Step::If { condition, then_steps, else_steps } => {
                    if self.evaluate_condition(condition).await? {
                        tracing::info!("[AUTO-ENGINE] Condition matched. Executing 'THEN' branch.");
                        for s in then_steps { self.execute_step(s).await?; }
                    } else if let Some(steps) = else_steps {
                        tracing::info!("[AUTO-ENGINE] Condition failed. Executing 'ELSE' branch.");
                        for s in steps { self.execute_step(s).await?; }
                    }
                }
            }
            
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            Ok(())
        })
    }

    async fn evaluate_condition(&self, condition: &crate::core::automation::dsl::Condition) -> AppResult<bool> {
        use crate::core::browser::BrowserManager;
        let port = self.context.port;
        let tid = self.context.tab_id.clone();

        match condition {
            crate::core::automation::dsl::Condition::Exists { selector } => {
                let js = format!("!!document.querySelector('{}')", selector);
                let res = BrowserManager::execute_script(port, tid, js).await?;
                Ok(res == "true")
            }
            crate::core::automation::dsl::Condition::TextContains { selector, value } => {
                let js = format!(
                    "(() => {{ \
                        const el = document.querySelector('{}'); \
                        return el && el.innerText.includes('{}'); \
                    }})()", selector, value
                );
                let res = BrowserManager::execute_script(port, tid, js).await?;
                Ok(res == "true")
            }
        }
    }
}
