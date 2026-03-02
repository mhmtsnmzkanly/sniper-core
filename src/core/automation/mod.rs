use crate::core::error::{AppError, AppResult};
use chromiumoxide::Browser;
use crate::state::AutomationStep;
use crate::core::events::AppEvent;
use crate::ui::scrape::emit;
use std::time::Duration;
use tokio::time::sleep;
use futures::StreamExt;

pub struct AutomationEngine;

impl AutomationEngine {
    pub async fn run_pipeline(port: u16, tab_id: String, steps: Vec<AutomationStep>) -> AppResult<()> {
        let ws_url = crate::core::browser::BrowserManager::get_ws_url(port).await?;
        let (browser, mut handler) = Browser::connect(ws_url).await
            .map_err(|e| AppError::Browser(e.to_string()))?;
        
        tokio::spawn(async move {
            while let Some(_) = handler.next().await {}
        });

        let pages = browser.pages().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let page = pages.into_iter()
            .find(|p| p.target_id().as_ref() == tab_id)
            .ok_or_else(|| AppError::NotFound(format!("Tab {} not found", tab_id)))?;

        for (index, step) in steps.into_iter().enumerate() {
            emit(AppEvent::AutomationProgress(index));
            
            match step {
                AutomationStep::Navigate(url) => {
                    page.goto(url).await.map_err(|e| AppError::Automation(index, e.to_string()))?;
                }
                AutomationStep::Click(selector) => {
                    page.find_element(&selector).await
                        .map_err(|e| AppError::Automation(index, e.to_string()))?
                        .click().await
                        .map_err(|e| AppError::Automation(index, e.to_string()))?;
                }
                AutomationStep::Wait(secs) => {
                    sleep(Duration::from_secs(secs)).await;
                }
                AutomationStep::WaitSelector(selector) => {
                    page.wait_for_navigation().await.map_err(|e| AppError::Automation(index, e.to_string()))?;
                    let _ = page.find_element(&selector).await.map_err(|e| AppError::Automation(index, e.to_string()))?;
                }
                AutomationStep::ScrollBottom => {
                    page.evaluate("window.scrollTo(0, document.body.scrollHeight)").await
                        .map_err(|e| AppError::Automation(index, e.to_string()))?;
                }
                AutomationStep::ExtractText(selector) => {
                    let text = page.find_element(&selector).await
                        .map_err(|e| AppError::Automation(index, e.to_string()))?
                        .inner_text().await
                        .map_err(|e| AppError::Automation(index, e.to_string()))?;
                    tracing::info!("Extracted [{}]: {}", selector, text.unwrap_or_default());
                }
                AutomationStep::InjectJS(script) => {
                    page.evaluate(script).await.map_err(|e| AppError::Automation(index, e.to_string()))?;
                }
            }
            sleep(Duration::from_millis(500)).await;
        }

        emit(AppEvent::AutomationFinished);
        Ok(())
    }
}
