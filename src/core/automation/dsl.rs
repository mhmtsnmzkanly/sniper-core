use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationDsl {
    pub dsl_version: u32,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
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
    SetVariable { key: String, value: String },
    NewRow,
    Export { filename: String },
    Screenshot { filename: String },
    ScrollBottom,
    SwitchFrame { selector: String }, // Empty for main frame
    If {
        selector: String,
        then_steps: Vec<Step>,
    },
    ForEach {
        selector: String,
        body: Vec<Step>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Condition {
    Exists { selector: String },
    TextContains { selector: String, value: String },
}
