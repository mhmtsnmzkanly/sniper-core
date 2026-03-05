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

    out
}
