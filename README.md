# SNIPER STUDIO
## Browser Forensics + Unified Automation/Scripting Console

Sniper Studio is a Rust desktop app for browser inspection, capture, and automation.
It combines:
- CDP-based browser control
- Block-based Automation DSL
- Rhai Scripting (running on the same automation runtime)

## What The Program Does
- Launches and controls Chrome/Chromium via remote debugging.
- Lists active tabs and lets you target one.
- Captures page content (`html`, `complete`, `mirror`).
- Collects network/media/cookie/console data per tab.
- Downloads non-DRM HLS streams (`.m3u8`) from Media panel with one click.
- Includes a Blob URL `De-Masker` to resolve `blob:http...` media URLs to probable source URLs.
- Runs automation pipelines from UI blocks.
- Runs Rhai scripts that can call automation actions.

## Architecture (Short Self-Review)
- `ui/*`: egui panels and interaction flow.
- `core/events`: event bus between UI and async tasks.
- `core/browser`: CDP/browser operations.
- `core/automation`: DSL + execution runtime.
- `core/scripting`: Rhai parser/checker + action mapper to automation runtime.
- `logger`: system log + chrome log file writing.

Design choice:
- Automation and Scripting are intentionally unified through the same runtime path.
- This keeps step behavior (timeouts/retries/errors) consistent.

## Requirements
- Chrome/Chromium installed.
- Rust toolchain (`cargo`) for build/run.
- `ffmpeg` installed (required for HLS Video Downloader).
- Linux/macOS/Windows compatible paths are handled in code; verify Chrome path in UI.

## Build & Run
```bash
cargo check
cargo run -- --port 9222
```

## First Run
1. Choose output directory.
2. Choose profile mode (isolated or system profile).
3. Launch browser from `Ops` tab.

## Main UI Flow
### Ops
- **Browser Control** (Top Row): launch/terminate + browser config (Path, Port, Proxy, Stealth Mode, User-Agent, Random UA).
- **Chrome Tabs** (Middle Row): active tab targets, columns selector, and sync button.
- **Command Center** (Bottom Row): capture (HTML, COMPLETE, MIRROR), network, media, cookie, and console actions.
- Command Center now includes a **SCAN** button in Automation panel for visual selector capture.
- While browser is active, **RELAUNCH APPLY PROFILE** applies updated proxy/identity settings via controlled restart.
- While browser is active, you can reload the current tab from Command Center.

### Scripting
- Import/Export JSON script package.
- Built-in Template Library (`Apply Template`) for quick script bootstrap.
- `Check`: compile + basic lint without executing browser actions.
- `Check` now emits structured diagnostics (`code/stage/severity/line/column/hint`).
- `Dry-Run`: build action plan without browser execution.
- `Debugger`: build step preview and inspect actions one-by-one in Scripting tab.
- `Break Condition`: stop execution when an action text matches your condition.
- `Timing Telemetry`: emit per-step `TIMING` lines into System Telemetry.
- `Execute`: runs script through shared automation runtime.
- `Stop`: cooperative cancel request.
- Script output goes to `System Telemetry` (not local script output list).
- Known scripting errors emit KB hints (`KB` lines) in System Telemetry.
- Detailed scripting tutorial + API reference: see [`SCRIPTING.md`](./SCRIPTING.md).

## Extended Documentation
- Full system documentation (A-Z): [`DOCS.md`](./DOCS.md)
- Scripting tutorial and API reference: [`SCRIPTING.md`](./SCRIPTING.md)

### Logs (System Telemetry)
Central log stream for:
- system/runtime events
- scripting output
- chrome console mirrored events
- selector inspector capture hints/notifications
- timing/stealth related runtime lines during scripting/automation sessions

## Script Package Format (`.json`)
```json
{
  "version": 1,
  "name": "example",
  "description": "my script",
  "created_at": 1741140000,
  "updated_at": 1741143600,
  "entry": "main",
  "code": "fn main() { log(\"hello\"); }",
  "tags": ["sample"]
}
```

## Rhai Helpers (Current)
Tab and action helpers include:
- `Tab()`, `Tab("url")`, `TabNew()`, `tab_new()`, `Tab.new()`
- `TabCatch()`, `TabCurrent()`, `tab_catch()`, `Tab.catch()`
- `tab.navigate(url)`
- `tab.find_el(selector).click()` / `.type(value)`
- `tab.capture.html()/mirror()/complete()`
- `tab.console.inject(js)`
- `tab.network.start()/stop()`
- `tab.cookies.set/delete(...)`
- `tab.run_automation_json(dsl_json)`

File helpers (write scope restricted to output directory tree):
- `fs_write_text(rel_path, content)`
- `fs_append_text(rel_path, content)`
- `fs_mkdir_all(rel_dir)`
- `fs_exists(rel_path)`

## Log Files
Inside selected output directory:
- `session_<timestamp>.log` -> program/system logs
- `chrome_session_<timestamp>.log` -> browser console logs

## Notifications
- Multiple toasts are queued and shown in stack.
- Level prefixed and color-coded:
- `[OK]`
- `[ERROR]`
- `[WARN]`
- each toast can be closed independently.

## Known Limitations
- Script stop is cooperative; long single external operations may finish current step before full stop.
- Some legacy UI modules still contain unused state fields; cleanup can be done incrementally.

## Development Notes
- Run `cargo check` before commit.
- Prefer extending shared runtime over creating parallel execution paths.
