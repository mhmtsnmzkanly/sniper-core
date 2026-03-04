use crate::core::error::AppResult;
use async_trait::async_trait;

#[async_trait]
pub trait AutomationDriver: Send + Sync {
    async fn navigate(&self, url: &str) -> AppResult<()>;
    async fn click(&self, selector: &str) -> AppResult<()>;
    async fn type_text(&self, selector: &str, value: &str) -> AppResult<()>;
    async fn hover(&self, selector: &str) -> AppResult<()>;
    async fn eval(&self, js: &str) -> AppResult<String>;
    async fn screenshot(&self) -> AppResult<Vec<u8>>;
    async fn wait_for_navigation(&self) -> AppResult<()>;
}
