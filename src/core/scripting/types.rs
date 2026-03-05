use serde::{Deserialize, Serialize};

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
    pub port: u16,
    pub output_dir: std::path::PathBuf,
}
