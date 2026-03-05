# SNIPER STUDIO - Full Documentation (A to Z)

This document is the complete operational and technical guide for Sniper Studio.
It explains what each subsystem does, how it works, what outputs it produces, and which errors (current or potential) you should expect.

---

## 1. What Sniper Studio Is

Sniper Studio is a Rust desktop application that combines:
- Browser control over Chrome DevTools Protocol (CDP)
- Visual operations panel (capture, network, media, cookies, console)
- Block-based Automation DSL
- Rhai-based scripting that runs through the same automation runtime

Design principle:
- **Automation and Scripting share one execution path** for consistency in timeout/retry behavior.

---

## 2. High-Level Architecture

### 2.1 UI Layer (`src/ui/*`)
Main panels:
- `Ops` (`scrape.rs`)
- `Scripting` (`scripting.rs`)
- `System Telemetry` (`log_panel.rs`)
- Modal windows for Network/Media/Cookies/Automation/Console per tab workspace

### 2.2 Event Bus (`src/core/events/mod.rs`)
UI and async workers communicate via `AppEvent`.

### 2.3 Core Services
- `core/browser`: browser lifecycle, tab listing, capture, cookies, listener setup
- `core/automation`: DSL + runtime + engine
- `core/scripting`: Rhai check and action execution

### 2.4 Shared State (`src/state.rs`)
`AppState` contains:
- global app config
- tab/workspace data
- system logs
- notification queue
- scripting runtime metadata

### 2.5 Logging (`src/logger.rs`)
Two separate log files are maintained:
- `session_<timestamp>.log` (program/system)
- `chrome_session_<timestamp>.log` (browser console)

---

## 3. Startup and Session Lifecycle

1. Program starts and initializes async channels.
2. User confirms output directory.
3. User confirms browser profile mode.
4. Browser is launched with CDP remote debugging.
5. Health checks run periodically.
6. Active tabs are polled/refreshed.

Output side-effects:
- Session log files created in output directory.
- Capture/media artifacts written under output subfolders.

Potential startup errors:
- Chrome binary not found.
- Port already in use.
- Output directory not writable.
- Browser/CDP handshake failed.

---

## 4. Ops Panel Features

## 4.1 Browser Control
What it does:
- Launches/terminates browser instance.
- Uses configured Chrome path/profile/path/port.

Outputs:
- Browser process lifecycle events.
- System telemetry entries.

Errors:
- Launch failures due to bad binary path.
- Permission errors for profile/output folders.

## 4.2 Chrome Tabs
What it does:
- Lists CDP tabs (`type=page`).
- Lets user choose active tab target.

Outputs:
- Selected tab id used by commands and scripting target fallback.

Potential issues:
- No tabs when browser not ready.
- Stale tab list if refresh fails.

## 4.3 Command Center
Actions:
- Capture HTML / Complete / Mirror
- Open Automation window
- Open Network/Media/Cookies/Console windows

Outputs:
- Capture files
- Workspace records (network/media/cookies/logs)

Potential issues:
- Command denied when browser offline.
- Target tab no longer exists.

---

## 5. Capture System

Capture modes:
- `html`: writes one HTML file snapshot
- `complete`: writes a dedicated folder with page HTML content
- `mirror`: writes MHTML snapshot

Output examples:
- `html_captures/capture_<timestamp>.html`
- `complete_captures/complete_<timestamp>/index.html`
- `mirrors/snapshot_<timestamp>.mhtml`

Potential errors:
- CDP command failures
- file write failures
- missing tab target

---

## 6. Network, Media, Cookies, Console

## 6.1 Network Listener
- Starts per-tab listener.
- Receives requests/responses and stores into workspace.
- Can be toggled on/off with cancellation token.

Potential errors:
- Listener setup failure
- response body retrieval partial failures

## 6.2 Media Capture
- Media assets are derived from network responses.
- Binary payload may be decoded from base64 when needed.

Potential errors:
- binary decode failures
- very large payload memory pressure

## 6.3 Cookies
- read/add/delete cookie APIs via browser manager.

Potential errors:
- invalid domain/path combinations
- CDP rejection for malformed cookie fields

## 6.4 Console
- Browser console logs are mirrored into:
  - workspace console list
  - system telemetry (`CHROME` lines)
  - `chrome_session_<ts>.log`

Potential errors:
- log file append failure
- missing listener state

---

## 7. Automation Engine

## 7.1 Purpose
Executes DSL steps with retry/timeout semantics and emits progress events.

## 7.2 Runtime Behavior
- Step-by-step execution
- retry loop (`retry_attempts`)
- timeout enforcement (`step_timeout`)
- optional screenshot-on-error

## 7.3 DSL Features (current)
Includes actions such as:
- navigation, click, type, wait
- wait selector / wait idle / wait network idle
- extract, export, screenshot
- conditionals (`If`) and loops (`ForEach`)
- function calls
- dataset import mode (`ImportDataset`)

## 7.4 Outputs
- extracted dataset updates
- screenshots on failure
- progress, finish, error events

## 7.5 Known limits
- some advanced recovery strategies are still basic
- selectors can be brittle on dynamic pages

---

## 8. Scripting Engine (Rhai)

Scripting is a thin layer over automation/browser operations.

Flow:
1. Script package loaded
2. `Check` performs compile + basic lint
3. `Execute` compiles and maps Rhai calls to internal actions
4. Actions are executed via shared automation runtime and browser APIs

Current helper families:
- tab creation/binding (`Tab`, `TabNew`, `TabCatch`)
- element actions (`find_el`, `click`, `type`)
- capture/network/console/cookies services
- file helpers (`fs_write_text`, `fs_append_text`, `fs_mkdir_all`)

Important behavior:
- Script output is written to **System Telemetry**.
- `Stop` is cooperative (cancel token).

Known limitations:
- `Tab.catch()` naming not fully normalized (`TabCatch` still used)
- `console.logs`, `cookies.get_all`, `fs_exists` are stub/limited
- advanced query chaining is not fully implemented yet

See full API in [`SCRIPTING.md`](./SCRIPTING.md).

---

## 9. Telemetry and Notifications

## 9.1 System Telemetry
Unified stream for:
- system events
- scripting output
- mirrored chrome console logs

## 9.2 Notification Queue
- Multiple toasts are supported.
- Levels: `[OK]`, `[ERROR]`, `[WARN]`, `[INFO]`
- Each notification can be closed independently.

Potential issues:
- overflow if too many events in short time (queue is capped)

---

## 10. Files and Artifacts Produced

Runtime directories/files (under output root):
- `session_<timestamp>.log`
- `chrome_session_<timestamp>.log`
- capture folders/files
- profile folder (when isolated profile selected)
- user-created script files via fs helpers

---

## 11. Error Catalog (Current + Potential)

## 11.1 Browser/Connectivity Errors
Examples:
- "Action Denied: Browser instance is not active"
- "Failed to connect to browser API"

Root causes:
- browser process down
- CDP port mismatch
- blocked local endpoint

Fixes:
- relaunch browser
- verify port and binary path

## 11.2 Scripting Compile/Entry Errors
Examples:
- "Rhai compile error: ..."
- "Entry function not found"

Root causes:
- invalid Rhai syntax
- wrong `entry` name

Fixes:
- run `Check`
- ensure `fn <entry>()` exists

## 11.3 Script Runtime Target Errors
Examples:
- "token not bound"
- "TabCatch failed: no selected tab in UI"

Root causes:
- missing tab selection
- invalid action order

Fixes:
- select tab in Ops
- ensure tab object is created before actions

## 11.4 Filesystem Scope Errors
Examples:
- "Absolute paths are not allowed"
- "Path escapes output_dir scope"

Root causes:
- unsafe path usage

Fixes:
- use relative paths under output directory

## 11.5 Automation Step Errors
Examples:
- selector not found
- navigation timeout
- extract failures

Root causes:
- brittle selectors
- dynamic page timing

Fixes:
- add waits, retries
- refine selectors
- use smarter sequencing

## 11.6 Potential (not always explicit) Failures
- large media payload memory pressure
- long running operations before cooperative cancellation takes effect
- stale tab references when user closes tab externally
- partial listener teardown edge cases

---

## 12. Operational Best Practices

- Always run `Check` before script execution.
- Use `Execution Target` intentionally (or select active tab in Ops).
- Prefer stable selectors (`id`, `data-*`) over fragile class chains.
- Keep script file operations relative and scoped.
- Review System Telemetry after each run.

---

## 13. Developer Extension Guidance

If you add new scripting functionality:
1. Add action enum entry in scripting engine.
2. Register Rhai binding.
3. Implement execution branch.
4. Add `Check` diagnostics if relevant.
5. Emit telemetry lines for observability.
6. Run `cargo check` and add/update docs.

---

## 14. Related Docs
- Main overview: [`README.md`](./README.md)
- Scripting tutorial + API: [`SCRIPTING.md`](./SCRIPTING.md)
