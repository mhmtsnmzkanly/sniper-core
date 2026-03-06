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
- Includes a **🎭 Blob De-Masker** UI to resolve `blob:http...` media URLs to probable source URLs.
- Runs automation pipelines from UI blocks.
- **Advanced Scripting IDE:** Rhai-based editor with syntax highlighting, autocomplete, live diagnostics, and hover documentation.

## Architecture (Short Self-Review)
- `ui/*`: egui panels and interaction flow.
- `core/events`: event bus between UI and async tasks.
- `core/browser`: CDP/browser operations.
- `core/network`: Dedicated network & media capture handler.
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
- **Browser Control** (Top Row): launch/terminate + advanced config (Proxy, Stealth Mode, Headless, Incognito, Resolution, Language, GPU/Audio toggle).
- **Chrome Tabs** (Middle Row): active tab targets, responsive grid with adjustable columns, and safe background auto-sync.
- **Command Center** (Bottom Row): capture (HTML, COMPLETE, MIRROR, DE-MASK), network, media, cookie, and console actions.
- Command Center now includes a **SCAN** button in Automation panel for visual selector capture.
- While browser is active, **RELAUNCH APPLY PROFILE** applies updated proxy/identity settings via controlled restart.

### Scripting (Mini-IDE)
- **Professional Editor:** Features line numbers, syntax highlighting, and selection highlighting.
- **Intelligent Autocomplete:** Context-aware suggestions for Browser APIs and Rhai keywords (triggered via `.` or `Ctrl+Space`).
- **Real-time Diagnostics:** Background error checking with visual red underlines and an **Error Gutter** (❌ icons) for immediate feedback.
- **Hover Documentation:** Mouse-over Browser APIs or errors to see documentation and detailed fixes.
- **Smart Indentation:** Automatic tab/space alignment on `Enter` (including auto-indent after `{`).
- **Find & Replace:** Dedicated search bar integrated into the editor (`Ctrl+F`).
- **Debugger & Dry-Run:** Preview and inspect action plans before execution.
- Known scripting errors emit KB hints (`KB` lines) in System Telemetry (e.g., suggesting `let` instead of reserved `var`).

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
- `Tab()`, `Tab("url")`, `TabNew()`, `TabCatch()`, `TabCurrent()`
- `tab.navigate(url)`, `tab.wait_for_ms(ms)`
- `tab.find_el(selector).click()` / `.type(value)`
- `tab.capture.html()`, `tab.capture.mirror()`, `tab.capture.complete()`
- `tab.console.inject(js)`, `tab.console.log(msg)`
- `tab.network.start()`, `tab.network.stop()`
- `tab.cookies.set/delete/get_all(...)`
- `tab.run_automation_json(dsl_json)`

File helpers (write scope restricted to output directory tree):
- `fs_write_text(rel_path, content)`
- `fs_append_text(rel_path, content)`
- `fs_mkdir_all(rel_dir)`
- `fs_exists(rel_path)`

## Media Vault
- Responsive card-based gallery for all captured assets.
- **Sorting:** Sort by Name, Type, or Size (asc/desc).
- **Filtering:** Filter by Type or **Minimum File Size (KB)**.
- Integrated HLS Downloader and **🎭 Blob De-Masker**.

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
