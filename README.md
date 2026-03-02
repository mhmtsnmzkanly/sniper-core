# Sniper Scraper Studio 4.0

A professional-grade DevTools, Automation, and Scraper Studio built with Rust. Leveraging the **Chrome DevTools Protocol (CDP)**, it provides a powerful GUI for web data extraction, network monitoring, and browser automation while remaining 100% immune to anti-bot systems (Cloudflare, Akamai, etc.) through manual intervention and remote debugging.

## 🚀 Key Features

- **Direct Tab Connection:** High-precision tab management using direct WebSocket connections via `chromiumoxide`.
- **Mirror Mode:** Automatically discover and download all page assets (Images, CSS, JS) for offline browsing.
- **Automation Builder:** A low-code step engine to build custom scraping pipelines (Navigate, Click, Wait, Extract).
- **Network Inspector:** Real-time HTTP request/response monitoring with status code highlights and resource filtering.
- **Script Injection Studio:** Live JavaScript execution on target tabs with instant result capture.
- **DevTools Studio:** Inspect and fetch session cookies, and emulate different devices via User-Agent and Geolocation spoofing.
- **Cross-Platform:** Native support for Linux (Manjaro/Arch optimized), Windows, and macOS.
- **Versioned Config:** Robust `.env` based configuration system with automatic migration support.

## 🛠 Usage Guide

### 1. Browser Preparation
To allow the studio to control your browser, launch Chrome/Chromium with the remote debugging port:
```bash
google-chrome --remote-debugging-port=9222
```

### 2. Launching the Studio
```bash
cargo run --release
```

### 3. Core Workflow
1. **SCRAPE Tab:** Set your target URL and click `LAUNCH BROWSER`. 
2. **Tab Selection:** Once the browser is open, click `REFRESH LIST` in Step 2. Select the tab you want to target.
3. **Capture:** Click `CAPTURE TARGET PAGE` to save a UTF-8 HTML copy. Enable `Mirror Mode` if you want to download images and styles.
4. **Automation:** Switch to the `AUTOMATION` tab to add steps like clicking buttons or waiting for elements to load.

## 🏗 Architecture Overview

The project follows a **Layered Event-Driven Architecture**:

- **`src/ui/`**: Modular GUI panels built with `egui`. Contains no business logic.
- **`src/core/browser/`**: A robust wrapper around `chromiumoxide` for handling CDP commands and tab management.
- **`src/core/events/`**: The central **Event Bus** using asynchronous channels (`mpsc`) to route messages between the UI and backend.
- **`src/core/automation/`**: State machine engine for executing sequential automation steps.
- **`src/config/`**: Handles versioned configuration loading, default values, and schema migrations.
- **`src/logger/`**: Centralized tracing system that pipes logs to both files (`logs/{TIME}.log`) and the GUI panel.

## 👨‍💻 Developer Guide

### Prerequisites
- **Rust Toolchain:** Latest stable version.
- **Cmake & Build-Essential:** Required for compiling `rquest` (BoringSSL).
  - *Linux (Manjaro):* `sudo pacman -S cmake base-devel`
  - *Windows:* Install via Visual Studio Build Tools.

### Adding a New Feature
1. **Define Event:** Add a new variant to `AppEvent` in `src/core/events/mod.rs`.
2. **Implement Logic:** Add the core logic in `src/core/` (e.g., `browser` or `downloader`).
3. **Update UI:** Create or update a panel in `src/ui/` to emit the new event.
4. **Handle Event:** Add a match arm in `src/app.rs` to connect the UI signal to the core logic.

### Optimization Tips
- **Binary Size:** Use `cargo build --release` to benefit from LTO and symbol stripping.
- **Async Safety:** Always use `tokio::spawn` for browser interactions to keep the UI thread responsive.

---
*Disclaimer: This tool is intended for educational and authorized testing purposes only. Please respect the Terms of Service of any website you interact with.*
