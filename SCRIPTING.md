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
4. **IDE Features:**
   - **Autocomplete:** Type `.` after a tab variable or press `Ctrl+Space` for suggestions.
   - **Live Diagnostics:** Errors are underlined in red as you type.
   - **Hover Docs:** Mouse-over a function to see its purpose.
5. Write or import script package (`.json`).
6. Press `Check` and/or `Dry-Run` first, then `Execute`.

Notes:
- `Check` validates script compile/entry/lint but does **not** execute actions.
- `Dry-Run` builds the action plan and prints it to System Telemetry without executing browser operations.
- `Debugger` shows a step-by-step preview of the action plan.
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

## 1.2.1 Template Library
- Open `Scripting` tab.
- Choose a template from `Template`.
- Press `Apply Template`.
- Adjust URL/selectors and run `Check`/`Dry-Run`.

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
        { "type": "SmartScroll", "until_selector": "#results", "max_rounds": 12, "settle_ms": 450 }
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

### `Tab.new()`
Dot-style alias (normalized to `TabNew()` before compile).

### `TabCatch()`
Binds to currently selected UI tab.

### `TabCurrent()`
Alias for selected tab binding.

### `tab_catch()`
Alias for selected tab binding.

### `Tab.catch()`
Dot-style alias (normalized to `TabCatch()` before compile).

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

## 2.6 Check Button and Live IDE Diagnostics

The editor performs **Real-time Diagnostics** in the background. The `Check` button manually triggers a full validation:
- Rhai compile success/failure
- non-empty `entry`
- entry function name presence in code text
- known warning patterns (e.g. `var` usage instead of `let`)
- API guard by running compile+binding stage to catch arity/type misuse
- optional selector preflight against selected tab

**Visual Feedback:**
- **Red Underline:** Indicates error location.
- **Error Gutter:** ❌ icon next to the line number.
- **Tooltips:** Hover over marked code to see error details and hints.

### Dry-Run
- Compiles script and builds internal action list.
- Emits planned actions to System Telemetry with `[SCRIPT -> ENGINE]` prefix.
- Does not call browser/CDP operations.

### Debugger
- Compiles script and builds internal action list.
- Stores plan in Scripting UI panel (`Script Debugger`).
- Use `Prev` / `Next` to inspect each planned action.
- `Break Condition` lets you jump to matching step in debugger preview.
- Extracted line/column info allows the engine to highlight the exact failing line in the IDE during a run.
- If Browser Control has `Stealth Mode` enabled, scripting applies stealth patch on bound/new tabs.

---

## 2.7 Execute and Stop

### Execute
- Compiles script and executes actions through shared runtime/browser APIs.
- Extracted line/column info ensures runtime errors are visually mapped in the IDE.

### Stop
- Cooperative cancel request.

---

## 2.8 Logging and Artifacts

Script-related logs appear in:
- `System Telemetry`
- `session_<timestamp>.log` (system-level)

Error Knowledge Base:
- On failures, known-pattern hints are emitted as `KB` lines in System Telemetry (e.g. `Use 'let' instead of 'var'`).

---

## 2.9 Contributor Notes

When adding a new scripting API function:
1. Add `ScriptAction` variant.
2. Register Rhai binding in action collection stage.
3. Implement action execution branch.
4. Update this file and run `cargo check`.
