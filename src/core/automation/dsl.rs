use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationDsl {
    pub dsl_version: u32,
    pub metadata: Option<ScriptMetadata>,
    pub functions: HashMap<String, Vec<Step>>,
    pub steps: Vec<Step>,
}

impl Default for AutomationDsl {
    fn default() -> Self {
        Self {
            dsl_version: 1,
            metadata: None,
            functions: HashMap::new(),
            steps: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptMetadata {
    pub name: String,
    pub description: String,
    pub author: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Step {
    Navigate { url: String },
    Click { selector: String },
    RightClick { selector: String },
    Hover { selector: String },
    Type { selector: String, value: String, is_variable: bool },
    Wait { seconds: u64 },
    WaitSelector { selector: String, timeout_ms: u64 },
    WaitUntilIdle { timeout_ms: u64 },
    WaitNetworkIdle { timeout_ms: u64, min_idle_ms: u64 },
    Extract { selector: String, as_key: String, add_to_row: bool },
    NewRow,
    Export { filename: String },
    Screenshot { filename: String },
    SetVariable { key: String, value: String },
    ScrollBottom,
    /// KOD NOTU: Lazy-load sayfalarda kontrollü kaydırma için akıllı scroll adımı.
    SmartScroll {
        until_selector: Option<String>,
        max_rounds: u32,
        settle_ms: u64,
    },
    SwitchFrame { selector: String },
    If { selector: String, then_steps: Vec<Step> },
    ForEach { selector: String, body: Vec<Step> },
    CallFunction { name: String },
    /// Import a dataset (CSV/JSON) and run subsequent steps for each row
    ImportDataset { filename: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Condition {
    ElementExists { selector: String },
    TextContains { selector: String, text: String },
}
