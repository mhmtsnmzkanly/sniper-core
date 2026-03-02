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

    async fn execute_step(&self, step: &Step) -> AppResult<()> {
        use crate::core::browser::BrowserManager;
        let port = self.context.port;
        let tid = self.context.tab_id.clone();

        match step {
            Step::Navigate { url } => {
                // We'll use a script injection for navigation to be more robust or direct CDP
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
                            if (Date.now() - start > {}) {{ clearInterval(timer); reject('Timeout'); }} 
                        }}, 100); 
                    }})", selector, timeout
                );
                BrowserManager::execute_script(port, tid, js).await?;
            }
            Step::ScrollBottom => {
                let js = "window.scrollTo(0, document.body.scrollHeight)".to_string();
                BrowserManager::execute_script(port, tid, js).await?;
            }
            _ => {
                tracing::warn!("[AUTO-ENGINE] Step type not yet implemented.");
            }
        }
        
        // Brief pause between steps for stability
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        Ok(())
    }
}
