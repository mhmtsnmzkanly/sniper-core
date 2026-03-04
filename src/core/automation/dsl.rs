use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationDsl {
    pub dsl_version: u32,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationScript {
    pub version: u32,
    pub metadata: ScriptMetadata,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptMetadata {
    pub name: String,
    pub description: String,
    pub author: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepErrorStrategy {
    Abort,
    Continue,
    Retry { max_attempts: u32 },
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
    Extract { selector: String, as_key: String, add_to_row: bool },
    NewRow,
    Export { filename: String },
    Screenshot { filename: String },
    WaitUntilIdle { timeout_ms: u64 },
    WaitNetworkIdle { timeout_ms: u64, min_idle_ms: u64 },
    SetVariable { key: String, value: String },
    ScrollBottom,
    SwitchFrame { selector: String },
    If { selector: String, then_steps: Vec<Step> },
    ForEach { selector: String, body: Vec<Step> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Condition {
    ElementExists { selector: String },
    TextContains { selector: String, text: String },
}
