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
use std::collections::HashMap;
use std::pin::Pin;
use std::future::Future;

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
    pub functions: HashMap<String, Vec<Step>>,
}

impl AutomationEngine {
    pub fn new(port: u16, tab_id: String, output_dir: std::path::PathBuf) -> Self {
        Self {
            context: Arc::new(Mutex::new(AutomationContext::new(port, tab_id, output_dir))),
            config: ExecutionConfig::default(),
            functions: HashMap::new(),
        }
    }

    pub async fn run(&mut self, dsl: AutomationDsl) -> AppResult<()> {
        self.functions = dsl.functions.clone();
        
        let (port, tid, _output_dir) = {
            let ctx = self.context.lock().unwrap();
            (ctx.port, ctx.tab_id.clone(), ctx.output_dir.clone())
        };

        tracing::info!("[AUTO-ENGINE] Connecting to browser for pipeline... Version: {}", dsl.dsl_version);
        
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
        let _ = page.execute(chromiumoxide::cdp::browser_protocol::page::EnableParams::default()).await;
        
        let driver = Box::new(CdpDriver::new(page));
        let steps = dsl.steps.clone();

        self.execute_steps_recursive(&steps, driver.as_ref()).await
    }

    fn execute_steps_recursive<'a>(&'a self, steps: &'a [Step], driver: &'a dyn AutomationDriver) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + 'a>> {
        Box::pin(async move {
            let (tid, output_dir) = {
                let ctx = self.context.lock().unwrap();
                (ctx.tab_id.clone(), ctx.output_dir.clone())
            };

            let mut i = 0;
            while i < steps.len() {
                let step = &steps[i];
                
                if let Step::ImportDataset { filename } = step {
                    let final_path = self.interpolate(filename);
                    let rows = self.load_dataset(&final_path)?;
                    tracing::info!("[AUTO-ENGINE] Dataset loaded: {} rows. Running subsequent steps per row.", rows.len());
                    
                    let remaining_steps = &steps[i+1..];
                    for (row_idx, row) in rows.into_iter().enumerate() {
                        tracing::info!("[AUTO-ENGINE] Pipeline Row {}/{}", row_idx + 1, row_idx + 1);
                        {
                            let mut ctx = self.context.lock().unwrap();
                            ctx.push_scope();
                            for (k, v) in row { ctx.set_variable(k, v); }
                        }
                        self.execute_steps_recursive(remaining_steps, driver).await?;
                        {
                            let mut ctx = self.context.lock().unwrap();
                            ctx.pop_scope();
                        }
                    }
                    return Ok(());
                }

                let _start_time = Instant::now();
                {
                    let mut ctx = self.context.lock().unwrap();
                    ctx.current_step = i;
                    emit(AppEvent::AutomationProgress(ctx.tab_id.clone(), i));
                }

                let mut attempts = 0;
                let max_attempts = self.config.retry_attempts + 1;
                let mut last_error = None;

                while attempts < max_attempts {
                    match self.execute_step_internal(step, driver).await {
                        Ok(_) => {
                            last_error = None;
                            break;
                        }
                        Err(e) => {
                            attempts += 1;
                            last_error = Some(e);
                            if attempts < max_attempts {
                                tokio::time::sleep(std::time::Duration::from_millis(500 * attempts as u64)).await;
                            }
                        }
                    }
                }

                if let Some(e) = last_error {
                    if self.config.screenshot_on_error {
                        if let Ok(data) = driver.screenshot().await {
                            let path = output_dir.join(format!("ERR_STEP_{}.png", i));
                            let _ = std::fs::write(path, data);
                        }
                    }
                    emit(AppEvent::AutomationError(tid.clone(), e.to_string()));
                    return Err(e);
                }
                i += 1;
            }
            Ok(())
        })
    }

    fn load_dataset(&self, path: &str) -> AppResult<Vec<HashMap<String, String>>> {
        let content = std::fs::read_to_string(path).map_err(|e| AppError::Internal(format!("Failed to read dataset: {}", e)))?;
        if path.ends_with(".csv") {
            let mut reader = csv::ReaderBuilder::new().from_reader(content.as_bytes());
            let mut results = Vec::new();
            let headers = reader.headers().map_err(|e| AppError::Internal(e.to_string()))?.clone();
            for result in reader.records() {
                let record = result.map_err(|e| AppError::Internal(e.to_string()))?;
                let mut row = HashMap::new();
                for (i, header) in headers.iter().enumerate() {
                    if let Some(val) = record.get(i) {
                        row.insert(header.to_string(), val.to_string());
                    }
                }
                results.push(row);
            }
            Ok(results)
        } else if path.ends_with(".json") {
            let data: Vec<HashMap<String, String>> = serde_json::from_str(&content).map_err(|e| AppError::Internal(e.to_string()))?;
            Ok(data)
        } else {
            Err(AppError::Internal("Unsupported dataset format".into()))
        }
    }

    fn interpolate(&self, text: &str) -> String {
        let ctx = self.context.lock().unwrap();
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

    fn execute_step_internal<'a>(&'a self, step: &'a Step, driver: &'a dyn AutomationDriver) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + 'a>> {
        Box::pin(async move {
            let (_tid, output_dir) = { 
                let ctx = self.context.lock().unwrap();
                (ctx.tab_id.clone(), ctx.output_dir.clone())
            };

            match step {
                Step::Navigate { url } => { driver.navigate(&self.interpolate(url)).await?; }
                Step::Click { selector } => { driver.click(&self.interpolate(selector)).await?; }
                Step::RightClick { selector } => {
                    let sel = self.interpolate(selector).replace("'", "\\'");
                    let js = format!("const el = queryRecursive('{}'); if (!el) throw new Error('Not found'); \
                                     el.dispatchEvent(new MouseEvent('contextmenu', {{ bubbles: true, button: 2 }}));", sel);
                    let _: String = driver.eval(&js).await?;
                }
                Step::Hover { selector } => { driver.hover(&self.interpolate(selector)).await?; }
                Step::Type { selector, value, .. } => { driver.type_text(&self.interpolate(selector), &self.interpolate(value)).await?; }
                Step::Wait { seconds } => { tokio::time::sleep(std::time::Duration::from_secs(*seconds)).await; }
                Step::WaitSelector { selector, timeout_ms } => {
                    let sel = self.interpolate(selector).replace("'", "\\'");
                    let js = format!("return new Promise((res, rej) => {{ \
                        const start = Date.now(); \
                        const t = setInterval(() => {{ \
                            if (queryRecursive('{}')) {{ clearInterval(t); res(true); }} \
                            if (Date.now() - start > {}) {{ clearInterval(t); rej('Timeout'); }} \
                        }}, 200); \
                    }})", sel, timeout_ms);
                    let _: String = driver.eval(&js).await?;
                }
                Step::Extract { selector, as_key, add_to_row } => {
                    let sel = self.interpolate(selector).replace("'", "\\'");
                    let js = format!("const el = queryRecursive('{}'); if (!el) return 'NOT_FOUND'; return el.innerText || el.value || '';", sel);
                    let text: String = driver.eval(&js).await?;
                    if text != "NOT_FOUND" {
                        let (tid_clone, current_rows) = {
                            let mut ctx = self.context.lock().unwrap();
                            ctx.set_variable(as_key.clone(), text.clone());
                            if *add_to_row { ctx.current_row.insert(as_key.clone(), text.clone()); }
                            (ctx.tab_id.clone(), ctx.extracted_data.clone())
                        };
                        emit(AppEvent::ConsoleLogAdded(tid_clone.clone(), format!("[DATA] {}: {}", as_key, text)));
                        emit(AppEvent::AutomationDatasetUpdated(tid_clone, current_rows));
                    } else { return Err(AppError::Browser(format!("Not found: {}", sel))); }
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
                    let path = output_dir.join(self.interpolate(filename));
                    let data = { let mut ctx = self.context.lock().unwrap(); ctx.push_current_row(); ctx.extracted_data.clone() };
                    if !data.is_empty() { if let Ok(json) = serde_json::to_string_pretty(&data) { let _ = std::fs::write(path, json); } }
                }
                Step::Screenshot { filename } => {
                    let data = driver.screenshot().await?;
                    let _ = std::fs::write(output_dir.join(self.interpolate(filename)), data);
                }
                Step::WaitUntilIdle { timeout_ms } => { driver.wait_for_navigation().await?; tokio::time::sleep(std::time::Duration::from_millis(*timeout_ms)).await; }
                Step::WaitNetworkIdle { timeout_ms, .. } => { tokio::time::sleep(std::time::Duration::from_millis(*timeout_ms)).await; }
                Step::SetVariable { key, value } => { self.context.lock().unwrap().set_variable(key.clone(), self.interpolate(value)); }
                Step::ScrollBottom => { let _: String = driver.eval("window.scrollTo(0, document.body.scrollHeight)").await?; }
                Step::SwitchFrame { selector } => {
                    let sel = self.interpolate(selector);
                    if !sel.is_empty() { let js = format!("queryRecursive('{}').contentWindow.focus()", sel.replace("'", "\\'")); let _: String = driver.eval(&js).await?; }
                    else { let _: String = driver.eval("window.focus()").await?; }
                }
                Step::If { selector, then_steps } => {
                    let res: String = driver.eval(&format!("!!queryRecursive('{}')", self.interpolate(selector).replace("'", "\\'"))).await?;
                    if res == "true" { self.execute_steps_recursive(then_steps, driver).await?; }
                }
                Step::ForEach { selector, body } => {
                    let sel = self.interpolate(selector).replace("'", "\\'");
                    let count: usize = driver.eval(&format!("document.querySelectorAll('{}').length", sel)).await?.parse().unwrap_or(0);
                    for i in 0..count {
                        { let mut ctx = self.context.lock().unwrap(); ctx.push_scope(); ctx.set_variable("index".into(), i.to_string()); ctx.set_variable("item".into(), format!("{}:nth-child({})", sel, i + 1)); }
                        self.execute_steps_recursive(body, driver).await?;
                        { self.context.lock().unwrap().pop_scope(); }
                    }
                }
                Step::CallFunction { name } => {
                    let steps = self.functions.get(name).cloned().ok_or_else(|| AppError::Internal(format!("Function '{}' not found", name)))?;
                    self.execute_steps_recursive(&steps, driver).await?;
                }
                _ => {}
            }
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            Ok(())
        })
    }
}
