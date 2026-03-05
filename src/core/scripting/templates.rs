use crate::core::scripting::types::{ScriptPackage, ScriptTemplate};

/// KOD NOTU: Template listesi UI'da hızlı başlangıç için sabit ve deterministik tutulur.
pub fn library() -> Vec<ScriptTemplate> {
    let now = chrono::Local::now().timestamp();
    vec![
        ScriptTemplate {
            id: "quick_capture".to_string(),
            title: "Quick Capture".to_string(),
            description: "Open page, wait, and save HTML + screenshot.".to_string(),
            package: ScriptPackage {
                version: 1,
                name: "quick_capture".to_string(),
                description: "Capture html and screenshot".to_string(),
                created_at: now,
                updated_at: now,
                entry: "main".to_string(),
                code: "fn main() {\n    let tab = Tab(\"https://example.com\");\n    tab.wait_for_ms(1200);\n    tab.capture.html();\n    tab.screenshot(\"quick_capture.png\");\n    log(\"quick capture finished\");\n}\n".to_string(),
                tags: vec!["template".to_string(), "capture".to_string()],
            },
        },
        ScriptTemplate {
            id: "search_flow".to_string(),
            title: "Search Flow".to_string(),
            description: "Navigate to search page, type query, click submit.".to_string(),
            package: ScriptPackage {
                version: 1,
                name: "search_flow".to_string(),
                description: "Simple form automation".to_string(),
                created_at: now,
                updated_at: now,
                entry: "main".to_string(),
                code: "fn main() {\n    let tab = Tab(\"https://duckduckgo.com\");\n    tab.wait_for_ms(1000);\n    let q = tab.find_el(\"input[name='q']\");\n    q.type(\"sniper studio scripting\");\n    let submit = tab.find_el(\"button[type='submit']\");\n    submit.click();\n}\n".to_string(),
                tags: vec!["template".to_string(), "form".to_string()],
            },
        },
        ScriptTemplate {
            id: "automation_bridge".to_string(),
            title: "Automation Bridge".to_string(),
            description: "Run Automation DSL JSON from Rhai script.".to_string(),
            package: ScriptPackage {
                version: 1,
                name: "automation_bridge".to_string(),
                description: "Run DSL from scripting".to_string(),
                created_at: now,
                updated_at: now,
                entry: "main".to_string(),
                code: "fn main() {\n    let tab = Tab.catch();\n    let dsl = `{\n      \"dsl_version\": 1,\n      \"metadata\": null,\n      \"functions\": {},\n      \"steps\": [\n        { \"type\": \"Wait\", \"seconds\": 1 },\n        { \"type\": \"SmartScroll\", \"until_selector\": \"#results\", \"max_rounds\": 8, \"settle_ms\": 400 }\n      ]\n    }`;\n    tab.run_automation_json(dsl);\n}\n".to_string(),
                tags: vec!["template".to_string(), "automation".to_string()],
            },
        },
    ]
}
