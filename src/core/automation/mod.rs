use crate::core::error::{AppError, AppResult};
use crate::state::AutomationStep;
use crate::ui::scrape::emit;
use crate::core::events::AppEvent;
use chromiumoxide::Browser;
use futures::StreamExt;
use std::time::Duration;

pub struct AutomationEngine;

impl AutomationEngine {
    pub async fn run_pipeline(port: u16, tab_id: String, steps: Vec<AutomationStep>) -> AppResult<()> {
        let ws_url = crate::core::browser::BrowserManager::get_ws_url(port).await?;
        let (browser, mut handler) = Browser::connect(ws_url).await.map_err(|e| AppError::Browser(e.to_string()))?;
        tokio::spawn(async move { while let Some(_) = handler.next().await {} });

        let pages = browser.pages().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let page = pages.into_iter()
            .find(|p| p.target_id().as_ref() == tab_id)
            .ok_or_else(|| AppError::NotFound("Tab not found".into()))?;

        for (index, step) in steps.iter().enumerate() {
            emit(AppEvent::AutomationProgress(tab_id.clone(), index));
            
            match step {
                AutomationStep::Navigate(url) => {
                    page.goto(url).await.map_err(|e| AppError::Browser(e.to_string()))?;
                }
                AutomationStep::Click(selector) => {
                    page.find_element(selector).await
                        .map_err(|e| AppError::Browser(e.to_string()))?
                        .click().await
                        .map_err(|e| AppError::Browser(e.to_string()))?;
                }
                AutomationStep::Wait(secs) => {
                    tokio::time::sleep(Duration::from_secs(*secs)).await;
                }
                AutomationStep::WaitSelector(sel) => {
                    // Simple poll for selector
                    let mut found = false;
                    for _ in 0..30 {
                        if page.find_element(sel).await.is_ok() {
                            found = true;
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                    if !found { return Err(AppError::Browser(format!("Timeout waiting for {}", sel))); }
                }
                AutomationStep::ScrollBottom => {
                    page.evaluate("window.scrollTo(0, document.body.scrollHeight)").await
                        .map_err(|e| AppError::Browser(e.to_string()))?;
                }
                AutomationStep::ExtractText(sel) => {
                    let text = page.evaluate(format!("document.querySelector('{}').innerText", sel)).await
                        .map_err(|e| AppError::Browser(e.to_string()))?;
                    tracing::info!("[AUTO <-> EXTRACT] {}: {:?}", sel, text.value());
                }
                AutomationStep::InjectJS(js) => {
                    page.evaluate(js.clone()).await.map_err(|e| AppError::Browser(e.to_string()))?;
                }
            }
        }

        emit(AppEvent::AutomationFinished(tab_id.clone()));
        Ok(())
    }
}
