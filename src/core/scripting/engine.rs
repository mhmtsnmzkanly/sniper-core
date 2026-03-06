use crate::core::automation::dsl::AutomationDsl;
use crate::core::automation::dsl::Step;
use crate::core::automation::engine::ExecutionConfig;
use crate::core::automation::runtime::run_dsl_on_tab;
use crate::core::error::{AppError, AppResult};
use crate::core::events::AppEvent;
use crate::core::scripting::types::{
    DiagnosticSeverity, DiagnosticStage, ScriptDiagnostic, ScriptExecutionRequest, ScriptPackage,
    ScriptingCheckReport,
};
use rhai::{Engine, EvalAltResult, Scope};
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone)]
struct TabRef {
    token: i64,
}

#[derive(Clone)]
struct ElementRef {
    token: i64,
    selector: String,
}

#[derive(Clone)]
struct ElementQuery {
    token: i64,
    selector: String,
}

#[derive(Clone)]
struct CaptureApi {
    token: i64,
}

#[derive(Clone)]
struct ConsoleApi {
    token: i64,
}

#[derive(Clone)]
struct NetworkApi {
    token: i64,
}

#[derive(Clone)]
struct CookiesApi {
    token: i64,
}

#[derive(Debug, Clone)]
enum ScriptAction {
    NewTab { token: i64, url: Option<String> },
    CatchTab { token: i64 },
    Navigate { token: i64, url: String },
    Click { token: i64, selector: String },
    Type { token: i64, selector: String, value: String },
    WaitMs { token: i64, ms: u64 },
    Screenshot { token: i64, filename: String },
    Capture { token: i64, mode: String },
    ConsoleInject { token: i64, js: String },
    NetworkToggle { token: i64, active: bool },
    CookieSet { token: i64, name: String, value: String, overwrite: bool },
    CookieDelete { token: i64, name: String, domain: String },
    RunDsl { token: i64, json: String },
    FsWriteText { rel_path: String, content: String },
    FsAppendText { rel_path: String, content: String },
    FsMkdirAll { rel_dir: String },
    Log(String),
}

#[derive(Default)]
struct ScriptBuildState {
    next_token: i64,
    actions: Vec<ScriptAction>,
    token_bindings: HashMap<i64, TokenBinding>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TokenBinding {
    New,
    Current,
}

struct ScriptStaticContext {
    output_dir: PathBuf,
    selected_tab_console_logs: Vec<String>,
    selected_tab_cookies: HashMap<String, String>,
}

fn new_token(state: &Arc<Mutex<ScriptBuildState>>) -> i64 {
    let mut lock = state.lock().unwrap();
    lock.next_token += 1;
    lock.next_token
}

fn push_action(state: &Arc<Mutex<ScriptBuildState>>, action: ScriptAction) {
    let mut lock = state.lock().unwrap();
    lock.actions.push(action);
}

fn file_in_scope(root: &Path, rel: &str) -> AppResult<PathBuf> {
    let rel_path = Path::new(rel);
    if rel_path.is_absolute() {
        return Err(AppError::Internal("Absolute paths are not allowed for script fs helpers".to_string()));
    }
    let joined = root.join(rel_path);
    let root_canon = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let parent = joined.parent().unwrap_or(root);
    let parent_canon = std::fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
    if !parent_canon.starts_with(&root_canon) {
        return Err(AppError::Internal(format!("Path escapes output_dir scope: {}", rel)));
    }
    Ok(joined)
}

/// KOD NOTU: Dot-style constructor kullanımlarını mevcut alias fonksiyonlarına dönüştürür.
fn normalize_scripting_code(raw: &str) -> String {
    raw.replace("Tab.new(", "TabNew(")
        .replace("Tab.catch(", "TabCatch(")
}

fn extract_line_col(message: &str) -> (Option<usize>, Option<usize>) {
    let Ok(re) = Regex::new(r"line\s+(\d+),\s*position\s+(\d+)") else {
        return (None, None);
    };
    if let Some(caps) = re.captures(message) {
        let line = caps.get(1).and_then(|m| m.as_str().parse::<usize>().ok());
        let col = caps.get(2).and_then(|m| m.as_str().parse::<usize>().ok());
        (line, col)
    } else {
        (None, None)
    }
}

fn push_diag(
    diagnostics: &mut Vec<ScriptDiagnostic>,
    code: &str,
    stage: DiagnosticStage,
    severity: DiagnosticSeverity,
    message: impl Into<String>,
    line: Option<usize>,
    column: Option<usize>,
    hint: Option<String>,
) {
    diagnostics.push(ScriptDiagnostic {
        code: code.to_string(),
        stage,
        severity,
        message: message.into(),
        line,
        column,
        hint,
    });
}

fn selectors_from_actions(actions: &[ScriptAction]) -> Vec<String> {
    let mut out = Vec::new();
    for action in actions {
        match action {
            ScriptAction::Click { selector, .. } | ScriptAction::Type { selector, .. } => {
                if !out.contains(selector) {
                    out.push(selector.clone());
                }
            }
            _ => {}
        }
    }
    out
}

async fn preflight_selectors(port: u16, tab_id: &str, selectors: &[String]) -> AppResult<Vec<(String, bool, bool)>> {
    let mut results = Vec::new();
    for selector in selectors {
        let escaped = selector.replace('\\', "\\\\").replace('\'', "\\'");
        let script = format!(
            "(() => {{
                try {{
                    const s = '{}';
                    const exists = !!document.querySelector(s);
                    return JSON.stringify({{ valid: true, exists }});
                }} catch (_e) {{
                    return JSON.stringify({{ valid: false, exists: false }});
                }}
            }})()",
            escaped
        );
        let raw = crate::core::browser::BrowserManager::execute_script(port, tab_id.to_string(), script).await?;
        let decoded: String = serde_json::from_str(&raw).unwrap_or(raw);
        let parsed = serde_json::from_str::<serde_json::Value>(&decoded).unwrap_or_default();
        let valid = parsed.get("valid").and_then(|v| v.as_bool()).unwrap_or(false);
        let exists = parsed.get("exists").and_then(|v| v.as_bool()).unwrap_or(false);
        results.push((selector.clone(), valid, exists));
    }
    Ok(results)
}

pub async fn check_script(
    package: &ScriptPackage,
    selected_tab_id: Option<String>,
    port: Option<u16>,
    run_preflight: bool,
) -> ScriptingCheckReport {
    let mut diagnostics: Vec<ScriptDiagnostic> = Vec::new();
    let mut ok = true;
    let mut selectors_for_preflight: Vec<String> = Vec::new();

    {
        let engine = Engine::new();
        let normalized_code = normalize_scripting_code(&package.code);
        match engine.compile(&normalized_code) {
            Ok(_) => push_diag(
                &mut diagnostics,
                "SC-COMPILE-OK",
                DiagnosticStage::Compile,
                DiagnosticSeverity::Info,
                "Rhai compile success",
                None,
                None,
                None,
            ),
            Err(e) => {
                let msg = format!("Rhai compile error: {}", e);
                let (line, column) = extract_line_col(&msg);
                push_diag(
                    &mut diagnostics,
                    "SC-COMPILE-ERR",
                    DiagnosticStage::Compile,
                    DiagnosticSeverity::Error,
                    msg,
                    line,
                    column,
                    Some("Fix syntax and run Check again.".to_string()),
                );
                ok = false;
            }
        }

        if package.entry.trim().is_empty() {
            push_diag(
                &mut diagnostics,
                "SC-ENTRY-EMPTY",
                DiagnosticStage::Entry,
                DiagnosticSeverity::Error,
                "Entry function name is empty",
                None,
                None,
                Some("Set `entry` to a function name like `main`.".to_string()),
            );
            ok = false;
        } else {
            let pattern = format!("fn {}", package.entry.trim());
            if !package.code.contains(&pattern) {
                push_diag(
                    &mut diagnostics,
                    "SC-ENTRY-MISSING",
                    DiagnosticStage::Entry,
                    DiagnosticSeverity::Error,
                    format!("Entry function '{}' not found in script text", package.entry),
                    None,
                    None,
                    Some("Define the entry function or update `entry`.".to_string()),
                );
                ok = false;
            }
        }

        if package.code.contains("r#\"") {
            push_diag(
                &mut diagnostics,
                "SC-LINT-RAWSTR",
                DiagnosticStage::Lint,
                DiagnosticSeverity::Warn,
                "Rust raw-string syntax detected; Rhai expects backtick strings (`...`)",
                None,
                None,
                Some("Replace r#\"...\"# with `...`".to_string()),
            );
        }
        if package.code.contains("TabCatch(") {
            push_diag(
                &mut diagnostics,
                "SC-LINT-DEPRECATED-TABCATCH",
                DiagnosticStage::Lint,
                DiagnosticSeverity::Warn,
                "Deprecated API: prefer Tab.catch() alias style",
                None,
                None,
                Some("Use Tab.catch() for new scripts.".to_string()),
            );
        }
        if package.code.contains("inject(") && package.code.contains('`') {
            push_diag(
                &mut diagnostics,
                "SC-LINT-INJECT-QUOTE",
                DiagnosticStage::Lint,
                DiagnosticSeverity::Warn,
                "Potential quote mismatch around inject() arguments",
                None,
                None,
                Some("Use inject(\"...\") and escape nested quotes.".to_string()),
            );
        }

        if ok {
            let static_ctx = ScriptStaticContext {
                output_dir: std::env::temp_dir(),
                selected_tab_console_logs: Vec::new(),
                selected_tab_cookies: HashMap::new(),
            };
            match collect_actions(package, &static_ctx) {
                Ok(actions) => {
                    push_diag(
                        &mut diagnostics,
                        "SC-APIGUARD-OK",
                        DiagnosticStage::ApiGuard,
                        DiagnosticSeverity::Info,
                        format!("API guard passed. Planned actions: {}", actions.len()),
                        None,
                        None,
                        None,
                    );
                    selectors_for_preflight = selectors_from_actions(&actions);
                }
                Err(e) => {
                    let msg = e.to_string();
                    let (line, column) = extract_line_col(&msg);
                    push_diag(
                        &mut diagnostics,
                        "SC-APIGUARD-ERR",
                        DiagnosticStage::ApiGuard,
                        DiagnosticSeverity::Error,
                        format!("API guard failed: {}", msg),
                        line,
                        column,
                        Some("Check function names and argument counts/types.".to_string()),
                    );
                    ok = false;
                }
            }
        }
    }

    if run_preflight && ok {
        if let (Some(tab_id), Some(real_port)) = (selected_tab_id.clone(), port) {
            if !selectors_for_preflight.is_empty() {
                match preflight_selectors(real_port, &tab_id, &selectors_for_preflight).await {
                    Ok(results) => {
                        for (selector, valid, exists) in results {
                            if !valid {
                                push_diag(
                                    &mut diagnostics,
                                    "SC-PREFLIGHT-SELECTOR-INVALID",
                                    DiagnosticStage::Preflight,
                                    DiagnosticSeverity::Error,
                                    format!("Invalid selector syntax: {}", selector),
                                    None,
                                    None,
                                    Some("Fix CSS selector syntax.".to_string()),
                                );
                                ok = false;
                            } else if !exists {
                                push_diag(
                                    &mut diagnostics,
                                    "SC-PREFLIGHT-SELECTOR-NOTFOUND",
                                    DiagnosticStage::Preflight,
                                    DiagnosticSeverity::Warn,
                                    format!("Selector not found on selected tab: {}", selector),
                                    None,
                                    None,
                                    Some("Page may not be ready; add wait or update selector.".to_string()),
                                );
                            }
                        }
                    }
                    Err(e) => {
                        push_diag(
                            &mut diagnostics,
                            "SC-PREFLIGHT-ERR",
                            DiagnosticStage::Preflight,
                            DiagnosticSeverity::Warn,
                            format!("Preflight failed: {}", e),
                            None,
                            None,
                            Some("Ensure selected tab is still open and browser is online.".to_string()),
                        );
                    }
                }
            }
        } else {
            push_diag(
                &mut diagnostics,
                "SC-PREFLIGHT-SKIP",
                DiagnosticStage::Preflight,
                DiagnosticSeverity::Info,
                "Preflight skipped: no selected tab context",
                None,
                None,
                Some("Select an execution target tab to enable selector preflight.".to_string()),
            );
        }
    }

    ScriptingCheckReport { ok, diagnostics }
}

/// KOD NOTU: Rhai script'i sync fazda sadece action listesine çevrilir; async browser işlemleri ikinci fazda çalıştırılır.
fn collect_actions(package: &ScriptPackage, static_ctx: &ScriptStaticContext) -> AppResult<Vec<ScriptAction>> {
    let build = Arc::new(Mutex::new(ScriptBuildState::default()));
    let mut engine = Engine::new();
    let mut scope = Scope::new();

    engine.register_type::<TabRef>();
    engine.register_type::<ElementRef>();
    engine.register_type::<ElementQuery>();
    engine.register_type::<CaptureApi>();
    engine.register_type::<ConsoleApi>();
    engine.register_type::<NetworkApi>();
    engine.register_type::<CookiesApi>();

    {
        let build = build.clone();
        engine.register_fn("Tab", move |url: &str| -> TabRef {
            let token = new_token(&build);
            push_action(&build, ScriptAction::NewTab { token, url: Some(url.to_string()) });
            if let Ok(mut lock) = build.lock() {
                lock.token_bindings.insert(token, TokenBinding::New);
            }
            TabRef { token }
        });
    }
    {
        let build = build.clone();
        engine.register_fn("Tab", move || -> TabRef {
            let token = new_token(&build);
            push_action(&build, ScriptAction::NewTab { token, url: None });
            if let Ok(mut lock) = build.lock() {
                lock.token_bindings.insert(token, TokenBinding::New);
            }
            TabRef { token }
        });
    }
    {
        let build = build.clone();
        engine.register_fn("TabNew", move || -> TabRef {
            let token = new_token(&build);
            push_action(&build, ScriptAction::NewTab { token, url: None });
            if let Ok(mut lock) = build.lock() {
                lock.token_bindings.insert(token, TokenBinding::New);
            }
            TabRef { token }
        });
    }
    {
        let build = build.clone();
        engine.register_fn("tab_new", move || -> TabRef {
            let token = new_token(&build);
            push_action(&build, ScriptAction::NewTab { token, url: None });
            if let Ok(mut lock) = build.lock() {
                lock.token_bindings.insert(token, TokenBinding::New);
            }
            TabRef { token }
        });
    }
    {
        let build = build.clone();
        engine.register_fn("TabCatch", move || -> TabRef {
            let token = new_token(&build);
            push_action(&build, ScriptAction::CatchTab { token });
            if let Ok(mut lock) = build.lock() {
                lock.token_bindings.insert(token, TokenBinding::Current);
            }
            TabRef { token }
        });
    }
    {
        let build = build.clone();
        engine.register_fn("TabCurrent", move || -> TabRef {
            let token = new_token(&build);
            push_action(&build, ScriptAction::CatchTab { token });
            if let Ok(mut lock) = build.lock() {
                lock.token_bindings.insert(token, TokenBinding::Current);
            }
            TabRef { token }
        });
    }
    {
        let build = build.clone();
        engine.register_fn("tab_catch", move || -> TabRef {
            let token = new_token(&build);
            push_action(&build, ScriptAction::CatchTab { token });
            if let Ok(mut lock) = build.lock() {
                lock.token_bindings.insert(token, TokenBinding::Current);
            }
            TabRef { token }
        });
    }
    {
        let build = build.clone();
        engine.register_fn("log", move |msg: &str| {
            push_action(&build, ScriptAction::Log(msg.to_string()));
        });
    }
    engine.register_fn("exit", |msg: &str| -> Result<(), Box<EvalAltResult>> { Err(msg.to_string().into()) });

    // FS helpers
    {
        let build = build.clone();
        engine.register_fn("fs_write_text", move |rel_path: &str, content: &str| {
            push_action(&build, ScriptAction::FsWriteText { rel_path: rel_path.to_string(), content: content.to_string() });
        });
    }
    {
        let build = build.clone();
        engine.register_fn("fs_append_text", move |rel_path: &str, content: &str| {
            push_action(&build, ScriptAction::FsAppendText { rel_path: rel_path.to_string(), content: content.to_string() });
        });
    }
    {
        let build = build.clone();
        engine.register_fn("fs_mkdir_all", move |rel_dir: &str| {
            push_action(&build, ScriptAction::FsMkdirAll { rel_dir: rel_dir.to_string() });
        });
    }
    let exists_root = static_ctx.output_dir.clone();
    engine.register_fn("fs_exists", move |rel_path: &str| -> bool {
        file_in_scope(&exists_root, rel_path)
            .map(|p| p.exists())
            .unwrap_or(false)
    });

    {
        let build = build.clone();
        engine.register_fn("navigate", move |tab: &mut TabRef, url: &str| {
            push_action(&build, ScriptAction::Navigate { token: tab.token, url: url.to_string() });
        });
    }
    {
        let build = build.clone();
        engine.register_fn("wait_for_ms", move |tab: &mut TabRef, ms: i64| {
            push_action(&build, ScriptAction::WaitMs { token: tab.token, ms: ms.max(0) as u64 });
        });
    }
    {
        let build = build.clone();
        engine.register_fn("screenshot", move |tab: &mut TabRef| {
            push_action(&build, ScriptAction::Screenshot { token: tab.token, filename: "script_capture.png".to_string() });
        });
    }
    {
        let build = build.clone();
        engine.register_fn("screenshot", move |tab: &mut TabRef, name: &str| {
            push_action(&build, ScriptAction::Screenshot { token: tab.token, filename: name.to_string() });
        });
    }
    engine.register_fn("find_el", move |tab: &mut TabRef, selector: &str| -> ElementQuery {
        ElementQuery { token: tab.token, selector: selector.to_string() }
    });
    {
        let build = build.clone();
        engine.register_fn("run_automation_json", move |tab: &mut TabRef, json: &str| {
            push_action(&build, ScriptAction::RunDsl { token: tab.token, json: json.to_string() });
        });
    }

    {
        let build = build.clone();
        engine.register_fn("click", move |el: &mut ElementRef| {
            push_action(&build, ScriptAction::Click { token: el.token, selector: el.selector.clone() });
        });
    }
    {
        let build = build.clone();
        engine.register_fn("type", move |el: &mut ElementRef, value: &str| {
            push_action(&build, ScriptAction::Type { token: el.token, selector: el.selector.clone(), value: value.to_string() });
        });
    }

    engine.register_fn("filter_id", |query: &mut ElementQuery, id: &str| -> ElementQuery {
        if !id.trim().is_empty() {
            query.selector = format!("{}#{}", query.selector, id.trim());
        }
        query.clone()
    });
    engine.register_fn("filter_class", |query: &mut ElementQuery, class_name: &str| -> ElementQuery {
        if !class_name.trim().is_empty() {
            query.selector = format!("{}.{}", query.selector, class_name.trim());
        }
        query.clone()
    });
    engine.register_fn("filter_attr", |query: &mut ElementQuery, key: &str, value: &str| -> ElementQuery {
        if !key.trim().is_empty() {
            query.selector = format!("{}[{}='{}']", query.selector, key.trim(), value.replace('\'', "\\'"));
        }
        query.clone()
    });
    engine.register_fn("first_or_none", |query: &mut ElementQuery| -> ElementRef {
        ElementRef {
            token: query.token,
            selector: query.selector.clone(),
        }
    });
    engine.register_fn("all", |query: &mut ElementQuery| -> Vec<ElementRef> {
        vec![ElementRef {
            token: query.token,
            selector: query.selector.clone(),
        }]
    });
    {
        let build = build.clone();
        engine.register_fn("click", move |query: &mut ElementQuery| {
            push_action(&build, ScriptAction::Click { token: query.token, selector: query.selector.clone() });
        });
    }
    {
        let build = build.clone();
        engine.register_fn("type", move |query: &mut ElementQuery, value: &str| {
            push_action(&build, ScriptAction::Type { token: query.token, selector: query.selector.clone(), value: value.to_string() });
        });
    }

    engine.register_get("capture", |tab: &mut TabRef| CaptureApi { token: tab.token });
    engine.register_get("console", |tab: &mut TabRef| ConsoleApi { token: tab.token });
    engine.register_get("network", |tab: &mut TabRef| NetworkApi { token: tab.token });
    engine.register_get("cookies", |tab: &mut TabRef| CookiesApi { token: tab.token });

    {
        let build = build.clone();
        engine.register_fn("html", move |cap: &mut CaptureApi| {
            push_action(&build, ScriptAction::Capture { token: cap.token, mode: "html".to_string() });
        });
    }
    {
        let build = build.clone();
        engine.register_fn("mirror", move |cap: &mut CaptureApi| {
            push_action(&build, ScriptAction::Capture { token: cap.token, mode: "mirror".to_string() });
        });
    }
    {
        let build = build.clone();
        engine.register_fn("complete", move |cap: &mut CaptureApi| {
            push_action(&build, ScriptAction::Capture { token: cap.token, mode: "complete".to_string() });
        });
    }

    {
        let build = build.clone();
        engine.register_fn("inject", move |cons: &mut ConsoleApi, js: &str| {
            push_action(&build, ScriptAction::ConsoleInject { token: cons.token, js: js.to_string() });
        });
    }
    {
        let build = build.clone();
        let selected_logs = static_ctx.selected_tab_console_logs.clone();
        engine.register_fn("logs", move |cons: &mut ConsoleApi| -> rhai::Array {
            let is_current = build
                .lock()
                .ok()
                .and_then(|s| s.token_bindings.get(&cons.token).copied())
                == Some(TokenBinding::Current);
            if is_current {
                selected_logs
                    .iter()
                    .cloned()
                    .map(rhai::Dynamic::from)
                    .collect::<rhai::Array>()
            } else {
                rhai::Array::new()
            }
        });
    }

    {
        let build = build.clone();
        engine.register_fn("start", move |net: &mut NetworkApi| {
            push_action(&build, ScriptAction::NetworkToggle { token: net.token, active: true });
        });
    }
    {
        let build = build.clone();
        engine.register_fn("stop", move |net: &mut NetworkApi| {
            push_action(&build, ScriptAction::NetworkToggle { token: net.token, active: false });
        });
    }

    {
        let build = build.clone();
        engine.register_fn("set", move |cookies: &mut CookiesApi, name: &str, value: &str, overwrite: bool| {
            push_action(&build, ScriptAction::CookieSet {
                token: cookies.token,
                name: name.to_string(),
                value: value.to_string(),
                overwrite,
            });
        });
    }
    {
        let build = build.clone();
        engine.register_fn("delete", move |cookies: &mut CookiesApi, name: &str, domain: &str| {
            push_action(&build, ScriptAction::CookieDelete {
                token: cookies.token,
                name: name.to_string(),
                domain: domain.to_string(),
            });
        });
    }
    {
        let build = build.clone();
        let selected_cookies = static_ctx.selected_tab_cookies.clone();
        engine.register_fn("get_all", move |cookies: &mut CookiesApi| -> rhai::Map {
            let is_current = build
                .lock()
                .ok()
                .and_then(|s| s.token_bindings.get(&cookies.token).copied())
                == Some(TokenBinding::Current);
            if is_current {
                selected_cookies
                    .iter()
                    .map(|(k, v)| (k.clone().into(), rhai::Dynamic::from(v.clone())))
                    .collect::<rhai::Map>()
            } else {
                rhai::Map::new()
            }
        });
    }

    let normalized_code = normalize_scripting_code(&package.code);
    let ast = engine
        .compile(&normalized_code)
        .map_err(|e| AppError::Internal(format!("Rhai compile error: {e}")))?;

    engine
        .run_ast_with_scope(&mut scope, &ast)
        .map_err(|e| AppError::Internal(format!("Rhai runtime error: {e}")))?;

    engine
        .call_fn::<()>(&mut scope, &ast, &package.entry, ())
        .map_err(|e| AppError::Internal(format!("Rhai entry error: {e}")))?;

    let actions = {
        let lock = build.lock().unwrap();
        lock.actions.clone()
    };

    Ok(actions)
}

/// KOD NOTU: Script execution iki fazlıdır: action toplama (sync) + action yürütme (async).
pub async fn run_script(req: ScriptExecutionRequest) -> AppResult<()> {
    let static_ctx = ScriptStaticContext {
        output_dir: req.output_dir.clone(),
        selected_tab_console_logs: req.selected_tab_console_logs.clone(),
        selected_tab_cookies: req.selected_tab_cookies.clone(),
    };
    let actions = collect_actions(&req.package, &static_ctx)?;
    execute_actions(req, actions).await
}

/// KOD NOTU: Dry-Run gerçek browser çağrısı yapmadan üretilecek action sırasını çıkarır.
pub fn dry_run_script(req: ScriptExecutionRequest) -> AppResult<Vec<String>> {
    let static_ctx = ScriptStaticContext {
        output_dir: req.output_dir.clone(),
        selected_tab_console_logs: req.selected_tab_console_logs.clone(),
        selected_tab_cookies: req.selected_tab_cookies.clone(),
    };
    let actions = collect_actions(&req.package, &static_ctx)?;
    Ok(actions
        .into_iter()
        .enumerate()
        .map(|(i, action)| format!("[{:03}] {:?}", i + 1, action))
        .collect())
}

async fn flush_token_steps(
    req: &ScriptExecutionRequest,
    token: i64,
    token_to_tab: &HashMap<i64, String>,
    step_batches: &mut HashMap<i64, Vec<Step>>,
) -> AppResult<usize> {
    let Some(tab_id) = token_to_tab.get(&token).cloned() else {
        return Err(AppError::Internal(format!("Script tab token {} is not bound", token)));
    };

    let steps = step_batches.remove(&token).unwrap_or_default();
    if steps.is_empty() {
        return Ok(0);
    }
    let step_count = steps.len();

    crate::ui::scrape::emit(AppEvent::ScriptingOutput(format!(
        "[SCRIPT -> ENGINE] Executing {} batched automation step(s)...",
        step_count
    )));

    run_dsl_on_tab(
        req.port,
        tab_id,
        req.output_dir.clone(),
        ExecutionConfig {
            step_timeout: std::time::Duration::from_millis(30_000),
            retry_attempts: 0,
            screenshot_on_error: true,
        },
        AutomationDsl {
            dsl_version: 1,
            metadata: None,
            functions: HashMap::new(),
            steps,
        },
    )
    .await?;

    Ok(step_count)
}

async fn resolve_cookie_domain(port: u16, tab_id: &str) -> Option<String> {
    let tabs = crate::core::browser::BrowserManager::list_tabs(port).await.ok()?;
    let url = tabs.into_iter().find(|t| t.id == tab_id)?.url;
    let parsed = url::Url::parse(&url).ok()?;
    parsed.host_str().map(|h| h.to_string())
}

fn ensure_not_cancelled(req: &ScriptExecutionRequest) -> AppResult<()> {
    if !req.cancel_token.load(Ordering::Relaxed) {
        return Err(AppError::Internal("Script cancelled by user".to_string()));
    }
    Ok(())
}

async fn execute_actions(req: ScriptExecutionRequest, actions: Vec<ScriptAction>) -> AppResult<()> {
    let mut token_to_tab: HashMap<i64, String> = HashMap::new();
    let mut step_batches: HashMap<i64, Vec<Step>> = HashMap::new();
    let break_condition = req.break_condition.clone().map(|s| s.to_ascii_lowercase());

    for (idx, action) in actions.into_iter().enumerate() {
        ensure_not_cancelled(&req)?;
        let action_debug = format!("{:?}", action);
        if let Some(cond) = &break_condition {
            if action_debug.to_ascii_lowercase().contains(cond) {
                crate::ui::scrape::emit(AppEvent::ScriptingOutput(format!(
                    "[BREAK] Step {:03} matched condition '{}': {}",
                    idx + 1,
                    cond,
                    action_debug
                )));
                break;
            }
        }
        let started = Instant::now();
        match action {
            ScriptAction::NewTab { token, url } => {
                crate::ui::scrape::emit(AppEvent::ScriptingOutput(format!(
                    "[SCRIPT] Creating new tab..."
                )));
                let created = crate::core::browser::BrowserManager::create_tab(req.port, url.as_deref()).await?;
                if req.apply_stealth {
                    let _ = crate::core::browser::BrowserManager::apply_stealth_on_tab(req.port, created.id.clone()).await;
                }
                token_to_tab.insert(token, created.id.clone());
                crate::ui::scrape::emit(AppEvent::ScriptingOutput(format!(
                    "[SCRIPT] Tab[{}] created: {}",
                    token, created.url
                )));
            }
            ScriptAction::CatchTab { token } => {
                let selected = req
                    .selected_tab_id
                    .clone()
                    .ok_or_else(|| AppError::Internal("TabCatch failed: no selected tab in UI".to_string()))?;
                if req.apply_stealth {
                    let _ = crate::core::browser::BrowserManager::apply_stealth_on_tab(req.port, selected.clone()).await;
                }
                token_to_tab.insert(token, selected);
                crate::ui::scrape::emit(AppEvent::ScriptingOutput(format!(
                    "[SCRIPT] Tab[{}] attached to selected target",
                    token
                )));
            }
            ScriptAction::Navigate { token, url } => {
                step_batches.entry(token).or_default().push(Step::Navigate { url });
            }
            ScriptAction::Click { token, selector } => {
                step_batches.entry(token).or_default().push(Step::Click { selector });
            }
            ScriptAction::Type { token, selector, value } => {
                step_batches.entry(token).or_default().push(Step::Type {
                    selector,
                    value,
                    is_variable: false,
                });
            }
            ScriptAction::WaitMs { token, ms } => {
                let secs = std::cmp::max(1, (ms + 999) / 1000);
                step_batches.entry(token).or_default().push(Step::Wait { seconds: secs });
            }
            ScriptAction::Screenshot { token, filename } => {
                step_batches.entry(token).or_default().push(Step::Screenshot { filename });
            }
            ScriptAction::Capture { token, mode } => {
                let flush_started = Instant::now();
                let flushed = flush_token_steps(&req, token, &token_to_tab, &mut step_batches).await?;
                if req.emit_step_timing && flushed > 0 {
                    crate::ui::scrape::emit(AppEvent::ScriptingOutput(format!(
                        "[TIMING] step {:03} pre-capture flush {} batched step(s) in {} ms",
                        idx + 1,
                        flushed,
                        flush_started.elapsed().as_millis()
                    )));
                }
                let tab_id = token_to_tab
                    .get(&token)
                    .cloned()
                    .ok_or_else(|| AppError::Internal(format!("Capture failed: token {} not bound", token)))?;
                let path = match mode.as_str() {
                    "html" => crate::core::browser::BrowserManager::capture_html(req.port, tab_id, req.output_dir.clone()).await?,
                    "mirror" => crate::core::browser::BrowserManager::capture_mirror(req.port, tab_id, req.output_dir.clone()).await?,
                    "complete" => crate::core::browser::BrowserManager::capture_complete(req.port, tab_id, req.output_dir.clone()).await?,
                    _ => return Err(AppError::Internal(format!("Unsupported capture mode: {}", mode))),
                };
                crate::ui::scrape::emit(AppEvent::ScriptingOutput(format!("Capture({mode}) -> {:?}", path)));
            }
            ScriptAction::ConsoleInject { token, js } => {
                let flush_started = Instant::now();
                let flushed = flush_token_steps(&req, token, &token_to_tab, &mut step_batches).await?;
                if req.emit_step_timing && flushed > 0 {
                    crate::ui::scrape::emit(AppEvent::ScriptingOutput(format!(
                        "[TIMING] step {:03} pre-inject flush {} batched step(s) in {} ms",
                        idx + 1,
                        flushed,
                        flush_started.elapsed().as_millis()
                    )));
                }
                let tab_id = token_to_tab
                    .get(&token)
                    .cloned()
                    .ok_or_else(|| AppError::Internal(format!("Console inject failed: token {} not bound", token)))?;
                let _ = crate::core::browser::BrowserManager::execute_script(req.port, tab_id, js).await?;
            }
            ScriptAction::NetworkToggle { token, active } => {
                let flush_started = Instant::now();
                let flushed = flush_token_steps(&req, token, &token_to_tab, &mut step_batches).await?;
                if req.emit_step_timing && flushed > 0 {
                    crate::ui::scrape::emit(AppEvent::ScriptingOutput(format!(
                        "[TIMING] step {:03} pre-network flush {} batched step(s) in {} ms",
                        idx + 1,
                        flushed,
                        flush_started.elapsed().as_millis()
                    )));
                }
                let tab_id = token_to_tab
                    .get(&token)
                    .cloned()
                    .ok_or_else(|| AppError::Internal(format!("Network toggle failed: token {} not bound", token)))?;
                crate::ui::scrape::emit(AppEvent::RequestNetworkToggle(tab_id, active));
            }
            ScriptAction::CookieSet { token, name, value, overwrite } => {
                let flush_started = Instant::now();
                let flushed = flush_token_steps(&req, token, &token_to_tab, &mut step_batches).await?;
                if req.emit_step_timing && flushed > 0 {
                    crate::ui::scrape::emit(AppEvent::ScriptingOutput(format!(
                        "[TIMING] step {:03} pre-cookie-set flush {} batched step(s) in {} ms",
                        idx + 1,
                        flushed,
                        flush_started.elapsed().as_millis()
                    )));
                }
                let tab_id = token_to_tab
                    .get(&token)
                    .cloned()
                    .ok_or_else(|| AppError::Internal(format!("Cookie set failed: token {} not bound", token)))?;

                if !overwrite {
                    crate::ui::scrape::emit(AppEvent::ScriptingOutput(format!(
                        "Cookie '{}' skipped because overwrite=false path is read-only in v1",
                        name
                    )));
                    continue;
                }

                let domain = resolve_cookie_domain(req.port, &tab_id)
                    .await
                    .unwrap_or_else(|| "localhost".to_string());
                let cookie = crate::state::ChromeCookie {
                    name,
                    value,
                    domain,
                    path: "/".to_string(),
                    expires: 0.0,
                    secure: false,
                    http_only: false,
                };
                crate::core::browser::BrowserManager::add_cookie(req.port, tab_id, cookie).await?;
            }
            ScriptAction::CookieDelete { token, name, domain } => {
                let flush_started = Instant::now();
                let flushed = flush_token_steps(&req, token, &token_to_tab, &mut step_batches).await?;
                if req.emit_step_timing && flushed > 0 {
                    crate::ui::scrape::emit(AppEvent::ScriptingOutput(format!(
                        "[TIMING] step {:03} pre-cookie-delete flush {} batched step(s) in {} ms",
                        idx + 1,
                        flushed,
                        flush_started.elapsed().as_millis()
                    )));
                }
                let tab_id = token_to_tab
                    .get(&token)
                    .cloned()
                    .ok_or_else(|| AppError::Internal(format!("Cookie delete failed: token {} not bound", token)))?;
                crate::core::browser::BrowserManager::delete_cookie(req.port, tab_id, name, domain).await?;
            }
            ScriptAction::RunDsl { token, json } => {
                let flush_started = Instant::now();
                let flushed = flush_token_steps(&req, token, &token_to_tab, &mut step_batches).await?;
                if req.emit_step_timing && flushed > 0 {
                    crate::ui::scrape::emit(AppEvent::ScriptingOutput(format!(
                        "[TIMING] step {:03} pre-dsl flush {} batched step(s) in {} ms",
                        idx + 1,
                        flushed,
                        flush_started.elapsed().as_millis()
                    )));
                }
                let tab_id = token_to_tab
                    .get(&token)
                    .cloned()
                    .ok_or_else(|| AppError::Internal(format!("run_automation_json failed: token {} not bound", token)))?;
                let dsl: AutomationDsl = serde_json::from_str(&json)
                    .map_err(|e| AppError::Internal(format!("DSL parse error: {}", e)))?;
                run_dsl_on_tab(
                    req.port,
                    tab_id,
                    req.output_dir.clone(),
                    ExecutionConfig {
                        step_timeout: std::time::Duration::from_millis(30_000),
                        retry_attempts: 0,
                        screenshot_on_error: true,
                    },
                    dsl,
                )
                .await?;
            }
            ScriptAction::FsWriteText { rel_path, content } => {
                let out = file_in_scope(&req.output_dir, &rel_path)?;
                if let Some(parent) = out.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&out, content).map_err(|e| AppError::Internal(format!("fs_write_text failed: {}", e)))?;
                crate::ui::scrape::emit(AppEvent::ScriptingOutput(format!("fs_write_text -> {:?}", out)));
            }
            ScriptAction::FsAppendText { rel_path, content } => {
                let out = file_in_scope(&req.output_dir, &rel_path)?;
                if let Some(parent) = out.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&out)
                    .map_err(|e| AppError::Internal(format!("fs_append_text failed: {}", e)))?;
                use std::io::Write;
                writeln!(file, "{}", content).map_err(|e| AppError::Internal(format!("fs_append_text write failed: {}", e)))?;
                crate::ui::scrape::emit(AppEvent::ScriptingOutput(format!("fs_append_text -> {:?}", out)));
            }
            ScriptAction::FsMkdirAll { rel_dir } => {
                let dir = file_in_scope(&req.output_dir, &rel_dir)?;
                std::fs::create_dir_all(&dir).map_err(|e| AppError::Internal(format!("fs_mkdir_all failed: {}", e)))?;
                crate::ui::scrape::emit(AppEvent::ScriptingOutput(format!("fs_mkdir_all -> {:?}", dir)));
            }
            ScriptAction::Log(message) => {
                crate::ui::scrape::emit(AppEvent::ScriptingOutput(message));
            }
        }
        if req.emit_step_timing {
            crate::ui::scrape::emit(AppEvent::ScriptingOutput(format!(
                "[TIMING] step {:03} completed in {} ms | {}",
                idx + 1,
                started.elapsed().as_millis(),
                action_debug
            )));
        }
    }

    for token in token_to_tab.keys().copied().collect::<Vec<_>>() {
        let flush_started = Instant::now();
        let flushed = flush_token_steps(&req, token, &token_to_tab, &mut step_batches).await?;
        if req.emit_step_timing && flushed > 0 {
            crate::ui::scrape::emit(AppEvent::ScriptingOutput(format!(
                "[TIMING] final flush token {} executed {} batched step(s) in {} ms",
                token,
                flushed,
                flush_started.elapsed().as_millis()
            )));
        }
    }

    Ok(())
}
