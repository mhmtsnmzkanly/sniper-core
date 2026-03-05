use crate::core::automation::dsl::AutomationDsl;
use crate::core::automation::dsl::Step;
use crate::core::automation::engine::ExecutionConfig;
use crate::core::automation::runtime::run_dsl_on_tab;
use crate::core::error::{AppError, AppResult};
use crate::core::events::AppEvent;
use crate::core::scripting::types::{ScriptExecutionRequest, ScriptPackage};
use rhai::{Engine, EvalAltResult, Scope};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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
    Log(String),
}

#[derive(Default)]
struct ScriptBuildState {
    next_token: i64,
    actions: Vec<ScriptAction>,
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

/// KOD NOTU: Rhai script'i sync fazda sadece action listesine çevrilir; async browser işlemleri ikinci fazda çalıştırılır.
fn collect_actions(package: &ScriptPackage) -> AppResult<Vec<ScriptAction>> {
    let build = Arc::new(Mutex::new(ScriptBuildState::default()));
    let mut engine = Engine::new();
    let mut scope = Scope::new();

    engine.register_type::<TabRef>();
    engine.register_type::<ElementRef>();
    engine.register_type::<CaptureApi>();
    engine.register_type::<ConsoleApi>();
    engine.register_type::<NetworkApi>();
    engine.register_type::<CookiesApi>();

    {
        let build = build.clone();
        engine.register_fn("Tab", move |url: &str| -> TabRef {
            let token = new_token(&build);
            push_action(&build, ScriptAction::NewTab { token, url: Some(url.to_string()) });
            TabRef { token }
        });
    }
    {
        let build = build.clone();
        engine.register_fn("Tab", move || -> TabRef {
            let token = new_token(&build);
            push_action(&build, ScriptAction::NewTab { token, url: None });
            TabRef { token }
        });
    }
    {
        let build = build.clone();
        engine.register_fn("TabCatch", move || -> TabRef {
            let token = new_token(&build);
            push_action(&build, ScriptAction::CatchTab { token });
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
    engine.register_fn("find_el", move |tab: &mut TabRef, selector: &str| -> ElementRef {
        ElementRef { token: tab.token, selector: selector.to_string() }
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
    engine.register_fn("logs", |_cons: &mut ConsoleApi| -> rhai::Array { rhai::Array::new() });

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
    engine.register_fn("get_all", |_cookies: &mut CookiesApi| -> rhai::Map { rhai::Map::new() });

    let ast = engine
        .compile(&package.code)
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
    let actions = collect_actions(&req.package)?;
    execute_actions(req, actions).await
}

async fn flush_token_steps(
    req: &ScriptExecutionRequest,
    token: i64,
    token_to_tab: &HashMap<i64, String>,
    step_batches: &mut HashMap<i64, Vec<Step>>,
) -> AppResult<()> {
    let Some(tab_id) = token_to_tab.get(&token).cloned() else {
        return Err(AppError::Internal(format!("Script tab token {} is not bound", token)));
    };

    let steps = step_batches.remove(&token).unwrap_or_default();
    if steps.is_empty() {
        return Ok(());
    }

    let dsl = AutomationDsl {
        dsl_version: 1,
        metadata: None,
        functions: HashMap::new(),
        steps,
    };

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
    .await
}

async fn resolve_cookie_domain(port: u16, tab_id: &str) -> Option<String> {
    let tabs = crate::core::browser::BrowserManager::list_tabs(port).await.ok()?;
    let url = tabs.into_iter().find(|t| t.id == tab_id)?.url;
    let parsed = url::Url::parse(&url).ok()?;
    parsed.host_str().map(|h| h.to_string())
}

async fn execute_actions(req: ScriptExecutionRequest, actions: Vec<ScriptAction>) -> AppResult<()> {
    let mut token_to_tab: HashMap<i64, String> = HashMap::new();
    let mut step_batches: HashMap<i64, Vec<Step>> = HashMap::new();

    for action in actions {
        match action {
            ScriptAction::NewTab { token, url } => {
                let created = crate::core::browser::BrowserManager::create_tab(req.port, url.as_deref()).await?;
                token_to_tab.insert(token, created.id.clone());
                crate::ui::scrape::emit(AppEvent::ScriptingOutput(format!(
                    "Tab[{}] created: {}",
                    token, created.url
                )));
            }
            ScriptAction::CatchTab { token } => {
                let selected = req
                    .selected_tab_id
                    .clone()
                    .ok_or_else(|| AppError::Internal("TabCatch failed: no selected tab in UI".to_string()))?;
                token_to_tab.insert(token, selected);
                crate::ui::scrape::emit(AppEvent::ScriptingOutput(format!(
                    "Tab[{}] attached to selected target",
                    token
                )));
            }
            ScriptAction::Navigate { token, url } => {
                step_batches.entry(token).or_default().push(Step::Navigate { url });
            }
            ScriptAction::Click { token, selector } => {
                step_batches.entry(token).or_default().push(Step::Click { selector });
            }
            ScriptAction::Type {
                token,
                selector,
                value,
            } => {
                step_batches.entry(token).or_default().push(Step::Type {
                    selector,
                    value,
                    is_variable: false,
                });
            }
            ScriptAction::WaitMs { token, ms } => {
                let secs = std::cmp::max(1, (ms + 999) / 1000);
                step_batches
                    .entry(token)
                    .or_default()
                    .push(Step::Wait { seconds: secs });
            }
            ScriptAction::Screenshot { token, filename } => {
                step_batches
                    .entry(token)
                    .or_default()
                    .push(Step::Screenshot { filename });
            }
            ScriptAction::Capture { token, mode } => {
                flush_token_steps(&req, token, &token_to_tab, &mut step_batches).await?;
                let tab_id = token_to_tab
                    .get(&token)
                    .cloned()
                    .ok_or_else(|| AppError::Internal(format!("Capture failed: token {} not bound", token)))?;
                let path = match mode.as_str() {
                    "html" => {
                        crate::core::browser::BrowserManager::capture_html(req.port, tab_id, req.output_dir.clone())
                            .await?
                    }
                    "mirror" => {
                        crate::core::browser::BrowserManager::capture_mirror(req.port, tab_id, req.output_dir.clone())
                            .await?
                    }
                    "complete" => {
                        crate::core::browser::BrowserManager::capture_complete(req.port, tab_id, req.output_dir.clone())
                            .await?
                    }
                    _ => return Err(AppError::Internal(format!("Unsupported capture mode: {}", mode))),
                };
                crate::ui::scrape::emit(AppEvent::ScriptingOutput(format!("Capture({mode}) -> {:?}", path)));
            }
            ScriptAction::ConsoleInject { token, js } => {
                flush_token_steps(&req, token, &token_to_tab, &mut step_batches).await?;
                let tab_id = token_to_tab
                    .get(&token)
                    .cloned()
                    .ok_or_else(|| AppError::Internal(format!("Console inject failed: token {} not bound", token)))?;
                let _ = crate::core::browser::BrowserManager::execute_script(req.port, tab_id, js).await?;
            }
            ScriptAction::NetworkToggle { token, active } => {
                flush_token_steps(&req, token, &token_to_tab, &mut step_batches).await?;
                let tab_id = token_to_tab
                    .get(&token)
                    .cloned()
                    .ok_or_else(|| AppError::Internal(format!("Network toggle failed: token {} not bound", token)))?;
                crate::ui::scrape::emit(AppEvent::RequestNetworkToggle(tab_id, active));
            }
            ScriptAction::CookieSet {
                token,
                name,
                value,
                overwrite,
            } => {
                flush_token_steps(&req, token, &token_to_tab, &mut step_batches).await?;
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
            ScriptAction::CookieDelete {
                token,
                name,
                domain,
            } => {
                flush_token_steps(&req, token, &token_to_tab, &mut step_batches).await?;
                let tab_id = token_to_tab
                    .get(&token)
                    .cloned()
                    .ok_or_else(|| AppError::Internal(format!("Cookie delete failed: token {} not bound", token)))?;
                crate::core::browser::BrowserManager::delete_cookie(req.port, tab_id, name, domain).await?;
            }
            ScriptAction::RunDsl { token, json } => {
                flush_token_steps(&req, token, &token_to_tab, &mut step_batches).await?;
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
            ScriptAction::Log(message) => {
                crate::ui::scrape::emit(AppEvent::ScriptingOutput(message));
            }
        }
    }

    for token in token_to_tab.keys().copied().collect::<Vec<_>>() {
        flush_token_steps(&req, token, &token_to_tab, &mut step_batches).await?;
    }

    Ok(())
}
