use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::collections::HashMap;

/// KOD NOTU: Script import/export için tek bir JSON paket sözleşmesi tutulur.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptPackage {
    pub version: u32,
    pub name: String,
    pub description: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub entry: String,
    pub code: String,
    pub tags: Vec<String>,
}

impl Default for ScriptPackage {
    fn default() -> Self {
        let now = chrono::Local::now().timestamp();
        Self {
            version: 1,
            name: "untitled".to_string(),
            description: String::new(),
            created_at: now,
            updated_at: now,
            entry: "main".to_string(),
            code: "fn main() {\n    log(\"hello scripting\");\n}\n".to_string(),
            tags: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScriptExecutionRequest {
    pub package: ScriptPackage,
    pub selected_tab_id: Option<String>,
    pub selected_tab_console_logs: Vec<String>,
    pub selected_tab_cookies: HashMap<String, String>,
    pub break_condition: Option<String>,
    pub emit_step_timing: bool,
    pub apply_stealth: bool,
    pub port: u16,
    pub output_dir: std::path::PathBuf,
    pub cancel_token: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
pub struct ScriptingCheckReport {
    pub ok: bool,
    pub diagnostics: Vec<ScriptDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warn,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticStage {
    Compile,
    Entry,
    ApiGuard,
    Lint,
    Preflight,
}

#[derive(Debug, Clone)]
pub struct ScriptDiagnostic {
    pub code: String,
    pub stage: DiagnosticStage,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub hint: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ScriptTemplate {
    pub id: String,
    pub title: String,
    pub description: String,
    pub package: ScriptPackage,
}
