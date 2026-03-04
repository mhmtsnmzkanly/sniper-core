# SNIPER STUDIO - Automation Engine State & Architecture

## 1. Core Logic Overview
The automation engine (`src/core/automation/engine.rs`) is an asynchronous executor that processes a series of steps defined in a DSL (Domain Specific Language). It uses the **Chromium DevTools Protocol (CDP)** to interact directly with the browser.

## 2. Component breakdown

### A. AutomationContext (`src/core/automation/context.rs`)
- **Variables:** A key-value store (`HashMap<String, String>`) for runtime data (e.g., extracted text).
- **Dataset:** A collection of rows (`Vec<HashMap<String, String>>`) for structured data extraction.
- **State:** Tracks the current step index and tab ID.

### B. DSL & Steps (`src/core/automation/dsl.rs`)
- Defines the `Step` enum, which is symmetric with the UI blocks.
- **Key Operations:**
    - `Navigate`: Standard navigation.
    - `Click/Type/Hover`: Element interaction using CSS selectors.
    - `WaitSelector`: JavaScript-based polling for element existence (more robust than native CDP wait).
    - `Extract`: Pulls `innerText` or `value` from an element and stores it in variables/dataset.
    - `ForEach`: Iterates over multiple elements matching a selector.
    - `If`: Conditional execution based on element existence.

### C. Execution Engine (`src/core/automation/engine.rs`)
- **Connection Management:** For each run, it establishes a fresh CDP connection to the target tab.
- **JS Injection:** Most "checks" (like WaitSelector or element highlights) are performed by injecting wrapped JavaScript into the page for maximum precision.
- **Interpolation:** Before execution, strings (selectors, URLs, values) are scanned for `{{variable_name}}` placeholders and replaced with actual values from the context.
- **Error Handling:** If a step fails, the engine captures a full-page screenshot to the output directory and aborts the pipeline to prevent cascading errors.

## 3. Current Limitations & Planned Improvements
- **Frame Support:** `SwitchFrame` exists but is basic (focuses the window of the iframe). Needs better recursive selector support.
- **Error Recovery:** Currently, any failure stops the whole engine. No `try/catch` or `resume` logic exists yet.
- **Variable Scoping:** All variables are global to the session. No local scope for loops.
- **Network Awareness:** `WaitNetworkIdle` is implemented but can be sensitive to background trackers.

## 4. How the UI Manages It
- The UI builds a `Vec<AutomationStep>`.
- When "START" is clicked, it sends a `RequestAutomationRun` event.
- `app.rs` spawns a background `tokio` task for the engine, keeping the UI responsive while automation runs in the background.
- Progress is reported back via `AutomationProgress` and `AutomationDatasetUpdated` events.
