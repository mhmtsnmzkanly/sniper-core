# SCRIPTING - Tutorial and API Reference

This document has two sections:
1. **Tutorial** (hands-on usage)
2. **Scripting API Reference** (current backend behavior)

Scripting in Sniper Studio runs on top of the same runtime used by Automation DSL.
This is intentional: timeout/retry/execution behavior stays consistent.

---

## 1) Tutorial

## 1.1 Quick Start
1. Start the app and launch browser from `Ops`.
2. Select a tab under `Chrome Tabs`.
3. Open `Scripting` tab.
4. Confirm `Execution Target`.
5. Write or import script package (`.json`).
6. Press `Check` first, then `Execute`.

Notes:
- `Check` validates script compile/entry/lint but does **not** execute actions.
- Script output is sent to **System Telemetry**.

## 1.2 Script Package Format (`.json`)
```json
{
  "version": 1,
  "name": "example_script",
  "description": "demo",
  "created_at": 1741140000,
  "updated_at": 1741143600,
  "entry": "main",
  "code": "fn main() { log(\"hello\"); }",
  "tags": ["demo"]
}
```

## 1.3 First Script: Navigate + Capture
```rhai
fn main() {
    let tab = Tab("https://example.com");
    tab.wait_for_ms(1200);
    tab.capture.html();
    log("HTML capture completed");
}
```

## 1.4 Bind to Selected UI Tab
```rhai
fn main() {
    let tab = TabCatch();
    tab.navigate("https://example.org");
    log("Selected tab redirected");
}
```

## 1.5 Element Interaction
```rhai
fn main() {
    let tab = Tab("https://duckduckgo.com");
    tab.wait_for_ms(1000);

    let input = tab.find_el("input[name='q']");
    input.type("sniper scripting");

    let submit = tab.find_el("button[type='submit']");
    submit.click();
}
```

## 1.6 Run Automation DSL from Script
```rhai
fn main() {
    let tab = TabCatch();

    let dsl = `{
      "dsl_version": 1,
      "metadata": null,
      "functions": {},
      "steps": [
        { "type": "Wait", "seconds": 1 },
        { "type": "ScrollBottom" }
      ]
    }`;

    tab.run_automation_json(dsl);
    log("Automation DSL invoked from script");
}
```

## 1.7 File Helpers (Output Scope Only)
```rhai
fn main() {
    fs_mkdir_all("script_outputs/run1");
    fs_write_text("script_outputs/run1/result.txt", "line 1");
    fs_append_text("script_outputs/run1/result.txt", "line 2");
}
```

Rules:
- Paths must be relative.
- Writes are restricted to output directory tree.
- Absolute paths are rejected.

## 1.8 Troubleshooting Workflow
- Run `Check` first.
- If check passes, run `Execute` in small iterations.
- Inspect `System Telemetry` for script/chrome/system lines.
- Verify tab selection if `TabCatch` fails.

---

## 2) Scripting API Reference

This section documents current implemented behavior.

## 2.1 Global Functions

### `log(message: string)`
Emits script output line to System Telemetry.

Common failures:
- none expected.

### `exit(message: string)`
Aborts script with an error.

Common failures:
- triggers runtime error by design.

### `fs_write_text(rel_path: string, content: string)`
Creates/overwrites a text file under output scope.

Common failures:
- path out of scope
- file permission errors

### `fs_append_text(rel_path: string, content: string)`
Appends text line to file under output scope.

Common failures:
- path out of scope
- open/write permission errors

### `fs_mkdir_all(rel_dir: string)`
Creates nested directories under output scope.

Common failures:
- invalid path
- permission denied

### `fs_exists(rel_path: string) -> bool`
Checks if a relative path exists under output scope.

---

## 2.2 Tab Construction / Binding

### `Tab()`
Creates a new blank tab.

### `Tab(url: string)`
Creates a new tab and opens URL.

### `TabNew()`
Alias for blank tab creation.

### `tab_new()`
Alias for blank tab creation.

### `TabCatch()`
Binds to currently selected UI tab.

### `TabCurrent()`
Alias for selected tab binding.

### `tab_catch()`
Alias for selected tab binding.

Common failures:
- no selected tab in UI

---

## 2.3 Tab Methods

### `tab.navigate(url: string)`
Queues navigation.

### `tab.wait_for_ms(ms: int)`
Queues wait (rounded to seconds in current action mapping).

### `tab.screenshot()` / `tab.screenshot(name: string)`
Queues screenshot step.

### `tab.find_el(selector: string) -> ElementQuery`
Returns query handle for chained filtering and actions.

### `tab.run_automation_json(dsl_json: string)`
Parses DSL JSON and runs through shared automation runtime.

Common failures:
- malformed JSON
- unknown tab binding
- runtime automation errors

---

## 2.4 ElementQuery Methods

### `query.filter_id(id: string) -> ElementQuery`
Appends id filter.

### `query.filter_class(name: string) -> ElementQuery`
Appends class filter.

### `query.filter_attr(key: string, value: string) -> ElementQuery`
Appends attribute filter.

### `query.first_or_none() -> ElementRef`
Returns first element handle (current implementation maps selector as one handle).

### `query.all() -> Array<ElementRef>`
Returns element handles (current implementation returns one selector-mapped handle).

### `query.click()`
Queues click action.

### `query.type(value: string)`
Queues type action.

Common failures:
- selector not found at runtime
- element not interactable

---

## 2.5 Service Objects

## Capture
- `tab.capture.html()`
- `tab.capture.mirror()`
- `tab.capture.complete()`

Outputs:
- capture files/folders in output directory.

## Console
- `tab.console.inject(js_code)`
- `tab.console.logs() -> Array`
  - returns selected UI tab console snapshot when tab is `TabCatch`/`TabCurrent`
  - returns empty for newly created tab refs

## Network
- `tab.network.start()`
- `tab.network.stop()`

## Cookies
- `tab.cookies.set(name, value, overwrite)`
- `tab.cookies.delete(name, domain)`
- `tab.cookies.get_all() -> Map`
  - returns selected UI tab cookie snapshot when tab is `TabCatch`/`TabCurrent`
  - returns empty for newly created tab refs

---

## 2.6 Check Button Semantics

`Check` currently validates:
- Rhai compile success/failure
- non-empty `entry`
- entry function name presence in code text
- known warning patterns (e.g. Rust raw string style usage)

It does **not**:
- execute browser actions
- guarantee selector correctness at runtime

---

## 2.7 Execute and Stop

### Execute
- Compiles script
- Builds internal action list
- Executes actions through shared runtime/browser APIs

### Stop
- Cooperative cancel request.
- Can stop between actions; long single operations may finish current action first.

---

## 2.8 Logging and Artifacts

Script-related logs appear in:
- `System Telemetry`
- `session_<timestamp>.log` (system-level)

Chrome console logs additionally appear in:
- `chrome_session_<timestamp>.log`

---

## 2.9 Current Known Gaps

- Dot-style construction (`Tab.new`, `Tab.catch`) is still not available; function aliases are used.
- Query execution still uses selector mapping; it does not yet fetch and cache concrete DOM node snapshots.
- Runtime selector validation remains dynamic (compile-time check cannot validate DOM presence).

---

## 2.10 Contributor Notes

When adding a new scripting API function:
1. Add `ScriptAction` variant.
2. Register Rhai binding in action collection stage.
3. Implement action execution branch.
4. Add check diagnostics if needed.
5. Emit telemetry lines for observability.
6. Update this file and run `cargo check`.
