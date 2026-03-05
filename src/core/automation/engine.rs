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

/// ExecutionConfig: Parameters controlling the behavior of step execution.
pub struct ExecutionConfig {
    /// Max time allowed for a single step to complete.
    pub step_timeout: std::time::Duration,
    /// Number of times to retry a failed step.
    pub retry_attempts: u32,
    /// Whether to capture a screenshot when a step eventually fails.
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

/// AutomationEngine: The core executor for automation pipelines.
/// It processes DSL steps asynchronously using an abstracted driver.
pub struct AutomationEngine {
    /// Shared context containing variables and extracted data.
    pub context: Arc<Mutex<AutomationContext>>,
    /// Current execution configuration.
    pub config: ExecutionConfig,
    /// Registry of reusable functions defined in the script.
    /// KOD NOTU: Gereksiz klonlamayı önlemek için fonksiyonlar Arc içinde saklanır.
    pub functions: Arc<HashMap<String, Arc<[Step]>>>,
}

impl AutomationEngine {
    pub fn new(port: u16, tab_id: String, output_dir: std::path::PathBuf) -> Self {
        Self {
            context: Arc::new(Mutex::new(AutomationContext::new(port, tab_id, output_dir))),
            config: ExecutionConfig::default(),
            functions: Arc::new(HashMap::new()),
        }
    }

    /// Entry point for running an automation pipeline.
    pub async fn run(&mut self, dsl: AutomationDsl) -> AppResult<()> {
        // Load function definitions into the engine registry.
        let mut funcs = HashMap::new();
        for (name, steps) in dsl.functions {
            funcs.insert(name, Arc::from(steps));
        }
        self.functions = Arc::new(funcs);
        
        let (port, tid, _output_dir) = {
            let ctx = self.context.lock().unwrap();
            (ctx.port, ctx.tab_id.clone(), ctx.output_dir.clone())
        };

        tracing::info!("[ENGINE] Pipeline started. DSL Version: {}, Functions: {}, Steps: {}", dsl.dsl_version, self.functions.len(), dsl.steps.len());
        
        // Establish connection to the browser instance.
        let ws_url = crate::core::browser::BrowserManager::get_ws_url(port).await?;
        let (browser, mut handler) = Browser::connect(ws_url).await.map_err(|e| AppError::Browser(e.to_string()))?;
        
        // Monitor the CDP connection in the background.
        let _handler_job = tokio::spawn(async move { while let Some(_) = handler.next().await {} });
        
        // Locate the target tab within the connected instance.
        let page = crate::core::browser::BrowserManager::find_tab(&browser, &tid).await?;
        let _ = page.execute(chromiumoxide::cdp::browser_protocol::page::EnableParams::default()).await;
        
        // Initialize the CDP-based driver abstraction.
        let driver = Box::new(CdpDriver::new(page));
        let steps: Arc<[Step]> = Arc::from(dsl.steps);

        // Start recursive step execution.
        let result = self.execute_steps_recursive(steps, driver.as_ref()).await;
        
        match &result {
            Ok(_) => {
                // KOD NOTU: Başarılı bitişte UI durumunu güncellemek için explicit finish eventi yayınlıyoruz.
                emit(AppEvent::AutomationFinished(tid.clone()));
                tracing::info!("[ENGINE] Full pipeline finished successfully.");
            }
            Err(e) => tracing::error!("[ENGINE] Pipeline aborted due to error: {}", e),
        }
        result
    }

    /// Recursively executes a list of steps, handling loops, conditionals, and dataset imports.
    /// Returns a Pinned Boxed Future to allow safe async recursion.
    fn execute_steps_recursive<'a>(&'a self, steps: Arc<[Step]>, driver: &'a dyn AutomationDriver) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + 'a>> {
        Box::pin(async move {
            let (tid, _output_dir) = {
                let ctx = self.context.lock().unwrap();
                (ctx.tab_id.clone(), ctx.output_dir.clone())
            };

            for idx in 0..steps.len() {
                let step = &steps[idx];
                // SPECIAL CASE: ImportDataset triggers a data-driven loop.
                if let Step::ImportDataset { filename } = step {
                    let final_path = self.interpolate(filename);
                    tracing::info!("[ENGINE] Importing dataset from: {}", final_path);
                    let rows = self.load_dataset(&final_path)?;
                    let total_rows = rows.len();
                    tracing::info!("[ENGINE] Dataset loaded with {} rows. Entering Data Pipeline Mode.", total_rows);
                    
                    let remaining_steps: Arc<[Step]> = Arc::from(&steps[idx+1..]);
                    for (row_idx, row) in rows.into_iter().enumerate() {
                        // KOD NOTU: Log satırındaki toplam satır sayısı (row_idx+1 / total_rows) olarak düzeltildi.
                        tracing::info!("[ENGINE] Processing Row {}/{}", row_idx + 1, total_rows);
                        {
                            let mut ctx = self.context.lock().unwrap();
                            ctx.push_scope(); // Create a new scope for the current row.
                            for (k, v) in &row { ctx.set_variable(k.clone(), v.clone()); }
                        }
                        // Execute all subsequent steps for each row in the dataset.
                        self.execute_steps_recursive(remaining_steps.clone(), driver).await?;
                        {
                            let mut ctx = self.context.lock().unwrap();
                            ctx.pop_scope(); // Cleanup scope after row processing.
                        }
                    }
                    return Ok(()); // Pipeline continues through the data loop.
                }

                let _start_time = Instant::now();
                {
                    let mut ctx = self.context.lock().unwrap();
                    ctx.current_step = idx;
                    emit(AppEvent::AutomationProgress(ctx.tab_id.clone(), idx));
                }

                let mut attempts = 0;
                let max_attempts = self.config.retry_attempts + 1;
                let mut last_error = None;

                // RETRY LOOP: Attempts to execute the step multiple times if configured.
                while attempts < max_attempts {
                    tracing::info!("[ENGINE][Step {}] Executing: {:?}", idx + 1, step);
                    
                    // KOD NOTU: step_timeout konfigürasyonu burada zorunlu kılınmıştır.
                    // Bir adım belirlenen sürede bitmezse timeout hatası fırlatılır.
                    let step_result = tokio::time::timeout(
                        self.config.step_timeout,
                        self.execute_step_internal(step, driver)
                    ).await;

                    match step_result {
                        Ok(Ok(_)) => {
                            tracing::info!("[ENGINE][Step {}] Success.", idx + 1);
                            last_error = None;
                            break;
                        }
                        Ok(Err(e)) => {
                            attempts += 1;
                            tracing::warn!("[ENGINE][Step {}] Attempt {} failed: {}", idx + 1, attempts, e);
                            last_error = Some(e);
                        }
                        Err(_) => {
                            attempts += 1;
                            let err_msg = format!("Step timed out after {:?}", self.config.step_timeout);
                            tracing::warn!("[ENGINE][Step {}] Attempt {} failed: {}", idx + 1, attempts, err_msg);
                            last_error = Some(AppError::Internal(err_msg));
                        }
                    }

                    if last_error.is_some() && attempts < max_attempts {
                        // Exponential backoff for retries.
                        tokio::time::sleep(std::time::Duration::from_millis(500 * attempts as u64)).await;
                    }
                }

                // If all attempts failed, handle the terminal error.
                if let Some(e) = last_error {
                    if self.config.screenshot_on_error {
                        if let Ok(data) = driver.screenshot().await {
                            // KOD NOTU: Hata anındaki ekran görüntüsü de yeni hiyerarşik yapıda saklanır.
                            if let Ok(path) = self.resolve_capture_path(driver, "failures", "png").await {
                                let _ = std::fs::write(&path, data);
                                tracing::error!("[ENGINE] Step failed. Screenshot saved to: {:?}", path);
                            }
                        }
                    }
                    emit(AppEvent::AutomationError(tid.clone(), e.to_string()));
                    return Err(e);
                }
            }
            Ok(())
        })
    }

    /// Loads and parses a dataset file (CSV or JSON).
    fn load_dataset(&self, path: &str) -> AppResult<Vec<HashMap<String, String>>> {
        let content = std::fs::read_to_string(path).map_err(|e| AppError::Internal(format!("Dataset read error: {}", e)))?;
        if path.ends_with(".csv") {
            let mut reader = csv::ReaderBuilder::new().from_reader(content.as_bytes());
            let mut results = Vec::new();
            let headers = reader.headers().map_err(|e| AppError::Internal(format!("CSV Header error: {}", e)))?.clone();
            for result in reader.records() {
                let record = result.map_err(|e| AppError::Internal(format!("CSV Row error: {}", e)))?;
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
            let data: Vec<HashMap<String, String>> = serde_json::from_str(&content).map_err(|e| AppError::Internal(format!("JSON Parse error: {}", e)))?;
            Ok(data)
        } else {
            Err(AppError::Internal("Unsupported dataset format. Use .csv or .json".into()))
        }
    }

    /// Interpolates variables in a string using {{variable_name}} syntax.
    fn interpolate(&self, text: &str) -> String {
        let ctx = self.context.lock().unwrap();
        let re = regex::Regex::new(r"\{\{([^}]+)\}\}").unwrap();
        let mut final_result = text.to_string();
        for cap in re.captures_iter(text) {
            let full_match = &cap[0];
            let var_name = cap[1].trim();
            if let Some(val) = ctx.get_variable(var_name) {
                tracing::debug!("[ENGINE] Resolving variable: {} -> {}", var_name, val);
                final_result = final_result.replace(full_match, &val);
            } else {
                tracing::warn!("[ENGINE] Variable not found in scope: {}", var_name);
            }
        }
        final_result
    }

    /// KOD NOTU: Gelişmiş çıktı yapısı için hiyerarşik dosya yolu oluşturur.
    /// Yapı: OUTPUT_DIR/TARIH/TIP/DOMAIN/SAYFA_BASLIGI.ext
    async fn resolve_capture_path(&self, driver: &dyn AutomationDriver, category: &str, ext: &str) -> AppResult<std::path::PathBuf> {
        let url_str = driver.eval("window.location.href").await?;
        let title = driver.eval("document.title").await?;
        
        let date_str = chrono::Local::now().format("%Y%m%d").to_string();
        let domain = url::Url::parse(&url_str).ok()
            .and_then(|u| u.host_str().map(|h| h.to_string()))
            .unwrap_or_else(|| "unknown_domain".to_string());

        // Slugify title
        let safe_title = title.chars()
            .map(|c: char| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>()
            .trim_matches('_')
            .to_string();
        
        let final_title = if safe_title.is_empty() { "index".to_string() } else { safe_title };
        let timestamp = chrono::Local::now().format("%H%M%S").to_string();

        let output_dir = self.context.lock().unwrap().output_dir.clone();
        let final_dir = output_dir.join(date_str).join(category).join(domain);
        std::fs::create_dir_all(&final_dir).map_err(AppError::Io)?;
        
        Ok(final_dir.join(format!("{}_{}.{}", final_title, timestamp, ext)))
    }

    /// Executes a single atomic DSL step using the driver.
    fn execute_step_internal<'a>(&'a self, step: &'a Step, driver: &'a dyn AutomationDriver) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + 'a>> {
        Box::pin(async move {
            let (_tid, _output_dir) = { 
                let ctx = self.context.lock().unwrap();
                (ctx.tab_id.clone(), ctx.output_dir.clone())
            };

            match step {
                Step::Navigate { url } => { 
                    let u = self.interpolate(url);
                    tracing::info!("[ENGINE] Navigating to: {}", u);
                    driver.navigate(&u).await?; 
                }
                Step::Click { selector } => { 
                    let s = self.interpolate(selector);
                    tracing::info!("[ENGINE] Clicking selector: {}", s);
                    driver.click(&s).await?; 
                }
                Step::RightClick { selector } => {
                    let s = self.interpolate(selector);
                    tracing::info!("[ENGINE] Right-clicking selector: {}", s);
                    let sel_esc = s.replace("'", "\\'");
                    let js = format!("const el = queryRecursive('{}'); if (!el) throw new Error('Not found'); \
                                     el.dispatchEvent(new MouseEvent('contextmenu', {{ bubbles: true, button: 2 }}));", sel_esc);
                    let _: String = driver.eval(&js).await?;
                }
                Step::Hover { selector } => { 
                    let s = self.interpolate(selector);
                    tracing::info!("[ENGINE] Hovering over: {}", s);
                    driver.hover(&s).await?; 
                }
                Step::Type { selector, value, .. } => { 
                    let s = self.interpolate(selector);
                    let v = self.interpolate(value);
                    tracing::info!("[ENGINE] Typing '{}' into {}", v, s);
                    driver.type_text(&s, &v).await?; 
                }
                Step::Wait { seconds } => { 
                    tracing::info!("[ENGINE] Waiting for {} seconds...", seconds);
                    tokio::time::sleep(std::time::Duration::from_secs(*seconds)).await; 
                }
                Step::WaitSelector { selector, timeout_ms } => {
                    let s = self.interpolate(selector);
                    tracing::info!("[ENGINE] Polling for selector: {} (Timeout: {}ms)", s, timeout_ms);
                    let sel_esc = s.replace("'", "\\'");
                    let js = format!("return new Promise((res, rej) => {{ \
                        const start = Date.now(); \
                        const t = setInterval(() => {{ \
                            if (queryRecursive('{}')) {{ clearInterval(t); res(true); }} \
                            if (Date.now() - start > {}) {{ clearInterval(t); rej('Timeout'); }} \
                        }}, 200); \
                    }})", sel_esc, timeout_ms);
                    let _: String = driver.eval(&js).await?;
                }
                Step::Extract { selector, as_key, add_to_row } => {
                    let s = self.interpolate(selector);
                    tracing::info!("[ENGINE] Extracting text from: {}", s);
                    let sel_esc = s.replace("'", "\\'");
                    let js = format!("const el = queryRecursive('{}'); if (!el) return 'NOT_FOUND'; return el.innerText || el.value || '';", sel_esc);
                    let text: String = driver.eval(&js).await?;
                    if text != "NOT_FOUND" {
                        tracing::info!("[ENGINE] Extracted [{}]: {}", as_key, text);
                        let (tid_clone, current_rows) = {
                            let mut ctx = self.context.lock().unwrap();
                            ctx.set_variable(as_key.clone(), text.clone());
                            if *add_to_row { ctx.current_row.insert(as_key.clone(), text.clone()); }
                            (ctx.tab_id.clone(), ctx.extracted_data.clone())
                        };
                        emit(AppEvent::ConsoleLogAdded(tid_clone.clone(), format!("[EXTRACT] {}: {}", as_key, text)));
                        emit(AppEvent::AutomationDatasetUpdated(tid_clone, current_rows));
                    } else { 
                        tracing::error!("[ENGINE] Extraction failed. Selector {} not found.", s);
                        return Err(AppError::Browser(format!("Not found: {}", s))); 
                    }
                }
                Step::NewRow => {
                    tracing::info!("[ENGINE] Committing current row to dataset.");
                    let (tid_clone, current_rows) = {
                        let mut ctx = self.context.lock().unwrap();
                        ctx.push_current_row();
                        (ctx.tab_id.clone(), ctx.extracted_data.clone())
                    };
                    emit(AppEvent::AutomationDatasetUpdated(tid_clone, current_rows));
                }
                Step::Export { .. } => {
                    let path = self.resolve_capture_path(driver, "datasets", "json").await?;
                    tracing::info!("[ENGINE] Exporting dataset to: {:?}", path);
                    let data = { let mut ctx = self.context.lock().unwrap(); ctx.push_current_row(); ctx.extracted_data.clone() };
                    if !data.is_empty() { 
                        if let Ok(json) = serde_json::to_string_pretty(&data) { let _ = std::fs::write(&path, json); } 
                    } else {
                        tracing::warn!("[ENGINE] Export skipped. Dataset is empty.");
                    }
                }
                Step::Screenshot { .. } => {
                    let path = self.resolve_capture_path(driver, "screenshots", "png").await?;
                    tracing::info!("[ENGINE] Taking screenshot: {:?}", path);
                    let data = driver.screenshot().await?;
                    let _ = std::fs::write(path, data);
                }
                Step::WaitUntilIdle { timeout_ms } => { 
                    tracing::info!("[ENGINE] Waiting for network/navigation idle ({}ms)", timeout_ms);
                    driver.wait_for_navigation().await?; 
                    // Fallback to network idle check if navigation is already done
                    driver.wait_for_network_idle(*timeout_ms, 500).await?;
                }
                Step::WaitNetworkIdle { timeout_ms, min_idle_ms } => { 
                    // KOD NOTU: min_idle_ms parametresi artık ignore edilmiyor, 
                    // PerformanceObserver ile gerçek boşta kalma süresi takip ediliyor.
                    tracing::info!("[ENGINE] Waiting for network silence ({}ms, min idle: {}ms)", timeout_ms, min_idle_ms);
                    driver.wait_for_network_idle(*timeout_ms, *min_idle_ms).await?; 
                }
                Step::SetVariable { key, value } => { 
                    let v = self.interpolate(value);
                    tracing::info!("[ENGINE] Setting variable: {} = {}", key, v);
                    self.context.lock().unwrap().set_variable(key.clone(), v); 
                }
                Step::ScrollBottom => { 
                    tracing::info!("[ENGINE] Scrolling to page bottom.");
                    let _: String = driver.eval("window.scrollTo(0, document.body.scrollHeight)").await?; 
                }
                Step::SmartScroll { until_selector, max_rounds, settle_ms } => {
                    let target_selector = until_selector
                        .as_ref()
                        .map(|s| self.interpolate(s))
                        .unwrap_or_default();
                    let selector_js = target_selector.replace('\'', "\\'");
                    let rounds = (*max_rounds).max(1);
                    let settle = (*settle_ms).max(50);
                    tracing::info!(
                        "[ENGINE] SmartScroll started (selector: '{}', max_rounds: {}, settle_ms: {})",
                        target_selector,
                        rounds,
                        settle
                    );
                    // KOD NOTU: Akıllı scroll, hedef selector görünene veya sayfa yüksekliği artmayı bırakana kadar ilerler.
                    let js = format!(
                        "return new Promise((resolve) => {{
                            const target = '{selector}';
                            const maxRounds = {rounds};
                            const settleMs = {settle};
                            let stagnantRounds = 0;
                            let round = 0;
                            let lastHeight = Math.max(document.body.scrollHeight, document.documentElement.scrollHeight);
                            const done = (reason) => resolve(`smart_scroll:${{reason}}:${{round}}`);
                            const loop = () => {{
                                round += 1;
                                if (target && document.querySelector(target)) return done('selector_found');
                                window.scrollTo(0, document.body.scrollHeight);
                                setTimeout(() => {{
                                    if (target && document.querySelector(target)) return done('selector_found');
                                    const currentHeight = Math.max(document.body.scrollHeight, document.documentElement.scrollHeight);
                                    if (currentHeight <= lastHeight) stagnantRounds += 1;
                                    else stagnantRounds = 0;
                                    lastHeight = currentHeight;
                                    if (stagnantRounds >= 2) return done('end_of_feed');
                                    if (round >= maxRounds) return done('max_rounds');
                                    loop();
                                }}, settleMs);
                            }};
                            loop();
                        }})",
                        selector = selector_js,
                        rounds = rounds,
                        settle = settle
                    );
                    let status: String = driver.eval(&js).await?;
                    tracing::info!("[ENGINE] SmartScroll finished: {}", status);
                }
                Step::SwitchFrame { selector } => {
                    let s = self.interpolate(selector);
                    if !s.is_empty() { 
                        tracing::info!("[ENGINE] Switching focus to frame: {}", s);
                        let js = format!("queryRecursive('{}').contentWindow.focus()", s.replace("'", "\\'")); 
                        let _: String = driver.eval(&js).await?; 
                    } else { 
                        tracing::info!("[ENGINE] Switching focus to main window.");
                        let _: String = driver.eval("window.focus()").await?; 
                    }
                }
                Step::If { selector, then_steps } => {
                    let s = self.interpolate(selector);
                    tracing::info!("[ENGINE] Checking condition: Element exists ({})", s);
                    let res: String = driver.eval(&format!("!!queryRecursive('{}')", s.replace("'", "\\'"))).await?;
                    if res == "true" { 
                        tracing::info!("[ENGINE] Condition MET. Executing nested steps.");
                        self.execute_steps_recursive(Arc::from(then_steps.as_slice()), driver).await?; 
                    } else {
                        tracing::info!("[ENGINE] Condition NOT met. Skipping nested steps.");
                    }
                }
                Step::ForEach { selector, body } => {
                    let s = self.interpolate(selector);
                    let sel_esc = s.replace("'", "\\'");
                    let count_str = driver.eval(&format!("document.querySelectorAll('{}').length", sel_esc)).await?;
                    let count: usize = count_str.parse().unwrap_or(0);
                    tracing::info!("[ENGINE] ForEach: Found {} elements matching {}", count, s);
                    let body_arc: Arc<[Step]> = Arc::from(body.as_slice());
                    for i in 0..count {
                        tracing::info!("[ENGINE] ForEach iteration {}/{}", i + 1, count);
                        { 
                            let mut ctx = self.context.lock().unwrap(); 
                            ctx.push_scope(); 
                            ctx.set_variable("index".into(), i.to_string()); 
                            ctx.set_variable("item".into(), format!("{}:nth-child({})", s, i + 1)); 
                        }
                        self.execute_steps_recursive(body_arc.clone(), driver).await?;
                        { self.context.lock().unwrap().pop_scope(); }
                    }
                }
                Step::CallFunction { name } => {
                    tracing::info!("[ENGINE] Calling function: {}", name);
                    let steps = self.functions.get(name).cloned().ok_or_else(|| AppError::Internal(format!("Function '{}' not found", name)))?;
                    self.execute_steps_recursive(steps, driver).await?;
                }
                Step::ImportDataset { .. } => { /* Handled in recursive loop */ }
            }
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            Ok(())
        })
    }
}
