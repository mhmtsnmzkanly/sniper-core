/// KOD NOTU: Bilinen hatalara kısa ve uygulanabilir çözüm ipuçları döndürülür.
pub fn hints_for_error(message: &str) -> Vec<&'static str> {
    let lower = message.to_ascii_lowercase();
    let mut out = Vec::new();

    if lower.contains("rhai compile error") && lower.contains("inject") && lower.contains("expecting ','") {
        out.push("Use plain string syntax: tab.console.inject(\"...\") and escape nested quotes.");
    }
    if lower.contains("entry function") && lower.contains("not found") {
        out.push("Set Entry to an existing function name and ensure code has `fn <entry>()`.");
    }
    if lower.contains("script tab token") && lower.contains("not bound") {
        out.push("Use Tab.catch() or choose Execution Target from Scripting tab.");
    }
    if lower.contains("path escapes output_dir scope") || lower.contains("absolute paths are not allowed") {
        out.push("Use relative output paths only, e.g. script_outputs/result.txt.");
    }
    if lower.contains("selector") && lower.contains("not found") {
        out.push("Try fallback chain selector_a || selector_b and add wait_for_ms before interaction.");
    }
    if lower.contains("timed out") {
        out.push("Increase wait time or step timeout; dynamic pages may need WaitSelector/SmartScroll.");
    }
    if lower.contains("api guard failed") {
        out.push("Run Check and inspect function names/argument counts for scripting API mismatches.");
    }
    if lower.contains("entry error") {
        out.push("Entry function may call unavailable APIs; test with a minimal body and add calls incrementally.");
    }
    if lower.contains("dsl parse error") {
        out.push("Validate JSON passed to run_automation_json; trailing commas and quotes are common issues.");
    }
    if lower.contains("tabcatch failed") || lower.contains("no selected tab in ui") {
        out.push("Select a target tab in Scripting Execution Target before using Tab.catch()/TabCatch().");
    }
    if lower.contains("unsupported capture mode") {
        out.push("Use capture.html(), capture.mirror(), or capture.complete() only.");
    }
    if lower.contains("invalid selector syntax") {
        out.push("Fix CSS syntax and test selector in browser devtools querySelector first.");
    }
    if lower.contains("selector not found on selected tab") {
        out.push("Page content may be dynamic; add wait_for_ms or navigate before interaction.");
    }
    if lower.contains("failed to start ffmpeg") {
        out.push("Install ffmpeg and ensure it is available in PATH.");
    }
    if lower.contains("preflight failed") {
        out.push("Keep browser/tab active while running Check preflight or disable preflight option.");
    }
    if lower.contains("cookie") && lower.contains("not bound") {
        out.push("Cookie APIs require a bound tab token; use Tab(url) or Tab.catch() first.");
    }
    if lower.contains("reserved") && lower.contains("var") {
        out.push("In Rhai, use 'let' instead of 'var' to declare variables.");
    }
    if lower.contains("console inject failed") {
        out.push("Ensure target tab is still open; use Tab.catch() to rebind active tab.");
    }

    out
}
