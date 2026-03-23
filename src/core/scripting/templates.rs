use crate::core::scripting::types::{ScriptPackage, ScriptTemplate};

/// KOD NOTU: Template listesi UI'da hızlı başlangıç için sabit ve deterministik tutulur.
pub fn library() -> Vec<ScriptTemplate> {
    let now = chrono::Local::now().timestamp();
    let mk = |id: &str, title: &str, description: &str, code: &str, tags: &[&str]| ScriptTemplate {
        id: id.to_string(),
        title: title.to_string(),
        description: description.to_string(),
        package: ScriptPackage {
            version: 1,
            name: id.to_string(),
            description: description.to_string(),
            created_at: now,
            updated_at: now,
            entry: "main".to_string(),
            code: code.to_string(),
            tags: tags.iter().map(|tag| tag.to_string()).collect(),
        },
    };

    vec![
        mk(
            "quick_capture",
            "Quick Capture",
            "Open page, wait, save HTML, and take named/default screenshots.",
            "fn main() {\n    let tab = Tab(\"https://example.com\");\n    tab.wait_for_ms(1200);\n    tab.capture.html();\n    tab.screenshot();\n    tab.screenshot(\"quick_capture.png\");\n    log(\"quick capture finished\");\n}\n",
            &["template", "capture", "starter"],
        ),
        mk(
            "search_flow",
            "Search Flow",
            "Navigate to a search page, type into an input, and click submit.",
            "fn main() {\n    let tab = Tab(\"https://duckduckgo.com\");\n    tab.wait_for_ms(1000);\n    let q = tab.find_el(\"input[name='q']\");\n    q.type(\"sniper studio scripting\");\n    let submit = tab.find_el(\"button[type='submit']\");\n    submit.click();\n}\n",
            &["template", "form", "elements"],
        ),
        mk(
            "query_filters_showcase",
            "Query Filters Showcase",
            "Demonstrates find_el, filter_id/class/attr, first_or_none, all, and both query/element actions.",
            "fn main() {\n    let tab = Tab(\"https://example.com\");\n    tab.wait_for_ms(800);\n\n    let query = tab\n        .find_el(\"form\")\n        .filter_id(\"search-form\")\n        .filter_class(\"primary\")\n        .filter_attr(\"data-role\", \"search\");\n\n    query.click();\n    query.type(\"typed through ElementQuery\");\n\n    let first = query.first_or_none();\n    first.click();\n    first.type(\"typed through ElementRef\");\n\n    let items = query.all();\n    log(\"query.all() returns selector-mapped handles in current host\");\n    if items.len > 0 {\n        items[0].click();\n    }\n}\n",
            &["template", "elements", "selectors"],
        ),
        mk(
            "tab_alias_matrix",
            "Tab Alias Matrix",
            "Shows every tab constructor/binding alias exposed by the host.",
            "fn main() {\n    let a = Tab();\n    a.navigate(\"https://example.com\");\n\n    let b = TabNew();\n    b.navigate(\"https://example.org\");\n\n    let c = tab_new();\n    c.navigate(\"https://example.net\");\n\n    let selected_a = TabCatch();\n    selected_a.wait_for_ms(1000);\n\n    let selected_b = TabCurrent();\n    selected_b.wait_for_ms(1000);\n\n    let selected_c = tab_catch();\n    selected_c.wait_for_ms(1000);\n\n    let preferred_new = Tab.new();\n    preferred_new.navigate(\"https://example.edu\");\n\n    let preferred_current = Tab.catch();\n    preferred_current.wait_for_ms(1000);\n\n    log(\"All tab constructor/binding aliases were exercised\");\n}\n",
            &["template", "aliases", "tabs"],
        ),
        mk(
            "capture_console_network_cookies",
            "Capture + Console + Network + Cookies",
            "Exercises service objects: capture, console.inject/logs, network.start/stop, cookies.set/get_all/delete.",
            "fn main() {\n    let tab = Tab.catch();\n    tab.network.start();\n    tab.navigate(\"https://example.com\");\n    tab.wait_for_ms(1500);\n\n    tab.console.inject(\"console.log('sniper scripting inject ok')\");\n\n    let logs = tab.console.logs();\n    log(\"Selected tab console snapshot count: \" + logs.len);\n\n    tab.cookies.set(\"sniper_demo\", \"active\", true);\n    let cookie_map = tab.cookies.get_all();\n    if cookie_map.contains(\"sniper_demo\") {\n        log(\"cookie sniper_demo is visible in current tab snapshot\");\n    }\n    tab.cookies.delete(\"sniper_demo\", \"example.com\");\n\n    tab.capture.html();\n    tab.capture.complete();\n    tab.capture.mirror();\n    tab.network.stop();\n}\n",
            &["template", "services", "network", "cookies"],
        ),
        mk(
            "filesystem_helpers",
            "Filesystem Helpers",
            "Shows fs_mkdir_all, fs_write_text, fs_append_text, fs_exists, and telemetry logging.",
            "fn main() {\n    let root = \"script_outputs/api_surface\";\n    let file = root + \"/notes.txt\";\n\n    fs_mkdir_all(root);\n    fs_write_text(file, \"line 1\");\n    fs_append_text(file, \"\\nline 2\");\n\n    if fs_exists(file) {\n        log(\"output file created successfully: \" + file);\n    } else {\n        exit(\"expected output file was not created\");\n    }\n}\n",
            &["template", "filesystem", "output"],
        ),
        mk(
            "automation_bridge",
            "Automation Bridge",
            "Run Automation DSL JSON from Rhai script.",
            "fn main() {\n    let tab = Tab.catch();\n    let dsl = `{\n      \"dsl_version\": 1,\n      \"metadata\": null,\n      \"functions\": {},\n      \"steps\": [\n        { \"type\": \"Wait\", \"seconds\": 1 },\n        { \"type\": \"SmartScroll\", \"until_selector\": \"#results\", \"max_rounds\": 8, \"settle_ms\": 400 },\n        { \"type\": \"Screenshot\", \"filename\": \"automation_bridge.png\" }\n      ]\n    }`;\n    tab.run_automation_json(dsl);\n    log(\"Automation DSL invoked from script\");\n}\n",
            &["template", "automation", "dsl"],
        ),
        mk(
            "current_tab_introspection",
            "Current Tab Introspection",
            "Reads current-tab console and cookie snapshots and writes them to output files.",
            "fn main() {\n    let tab = Tab.catch();\n    fs_mkdir_all(\"script_outputs/current_tab\");\n\n    let logs = tab.console.logs();\n    fs_write_text(\"script_outputs/current_tab/console_count.txt\", logs.len + \"\");\n\n    let cookies = tab.cookies.get_all();\n    let has_session = cookies.contains(\"session\");\n    fs_write_text(\"script_outputs/current_tab/has_session.txt\", has_session + \"\");\n\n    log(\"Current tab introspection snapshot exported\");\n}\n",
            &["template", "current-tab", "introspection"],
        ),
        mk(
            "fail_fast_exit",
            "Fail Fast Exit",
            "Minimal example for the exit() helper when a required condition is missing.",
            "fn main() {\n    if !fs_exists(\"script_outputs/api_surface/notes.txt\") {\n        exit(\"notes.txt missing; run the Filesystem Helpers template first\");\n    }\n    log(\"Fail-fast guard passed\");\n}\n",
            &["template", "exit", "guard"],
        ),
        mk(
            "api_surface_reference",
            "API Surface Reference",
            "One larger reference script that touches nearly every host-exposed Rhai helper in a single flow.",
            "fn main() {\n    fs_mkdir_all(\"script_outputs/reference\");\n    fs_write_text(\"script_outputs/reference/run.txt\", \"start\");\n    fs_append_text(\"script_outputs/reference/run.txt\", \"\\ncontinue\");\n\n    let selected = Tab.catch();\n    selected.network.start();\n    selected.console.inject(\"console.log('reference template booted')\");\n    let selected_logs = selected.console.logs();\n    log(\"selected log count: \" + selected_logs.len);\n\n    let selected_cookies = selected.cookies.get_all();\n    if selected_cookies.contains(\"locale\") {\n        log(\"locale cookie exists on selected tab\");\n    }\n\n    let form_query = selected\n        .find_el(\"form\")\n        .filter_class(\"search\")\n        .filter_attr(\"role\", \"search\");\n    let first = form_query.first_or_none();\n    first.click();\n    form_query.type(\"typed by query handle\");\n    let handles = form_query.all();\n    if handles.len > 0 {\n        handles[0].type(\"typed by element handle\");\n    }\n\n    selected.capture.html();\n    selected.capture.mirror();\n    selected.network.stop();\n\n    let fresh = Tab.new();\n    fresh.navigate(\"https://example.com\");\n    fresh.wait_for_ms(1200);\n    fresh.screenshot(\"reference_new_tab.png\");\n\n    let alias_new = TabNew();\n    alias_new.navigate(\"https://example.org\");\n\n    let alias_new_lower = tab_new();\n    alias_new_lower.navigate(\"https://example.net\");\n\n    let attached_a = TabCatch();\n    attached_a.wait_for_ms(1000);\n    let attached_b = TabCurrent();\n    attached_b.wait_for_ms(1000);\n    let attached_c = tab_catch();\n    attached_c.wait_for_ms(1000);\n\n    selected.cookies.set(\"sniper_reference\", \"1\", true);\n    selected.cookies.delete(\"sniper_reference\", \"example.com\");\n\n    let dsl = `{\n      \"dsl_version\": 1,\n      \"metadata\": null,\n      \"functions\": {},\n      \"steps\": [\n        { \"type\": \"Wait\", \"seconds\": 1 },\n        { \"type\": \"Screenshot\", \"filename\": \"reference_dsl.png\" }\n      ]\n    }`;\n    selected.run_automation_json(dsl);\n\n    if !fs_exists(\"script_outputs/reference/run.txt\") {\n        exit(\"reference output file missing\");\n    }\n\n    log(\"API surface reference completed\");\n}\n",
            &["template", "reference", "all-features"],
        ),
    ]
}
