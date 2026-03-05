use crate::core::automation::dsl::AutomationDsl;
use crate::core::automation::engine::{AutomationEngine, ExecutionConfig};
use crate::core::error::AppResult;
use std::path::PathBuf;

/// KOD NOTU: Automation ve Scripting tek yürütme yolu kullansın diye merkezi runtime helper eklendi.
pub async fn run_dsl_on_tab(
    port: u16,
    tab_id: String,
    output_dir: PathBuf,
    config: ExecutionConfig,
    dsl: AutomationDsl,
) -> AppResult<()> {
    let mut engine = AutomationEngine::new(port, tab_id, output_dir);
    engine.config = config;
    engine.run(dsl).await
}
