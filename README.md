# 🎯 SNIPER STUDIO // V1.2.0
### Advanced Browser Forensics & Intelligent Automation Engine

Sniper Studio is a high-precision, Rust-powered tool designed for web forensics, asset extraction, and complex browser automation. It combines low-level CDP (Chrome DevTools Protocol) control with a modern, block-based automation interface.

---

## 🚀 Key Features

### 1. Persistent Automation Engine
- **Visual Block Builder:** Create complex automation logic using a Scratch-like drag-and-drop interface.
- **Dynamic DSL:** Save and load your automation pipelines as JSON (DSL v1).
- **Control Flow:** Full support for `If/Else` conditions, `ForEach` loops, and `WaitSelector` steps.
- **Smart Execution:** Automatic element focus, smooth scrolling, and existence checks for bulletproof stability.

### 2. Digital Forensics & Monitoring
- **Deep CSS Scan:** Automatically extract hidden assets (icons, fonts, backgrounds) from CSS files.
- **Real-time Network Traffic:** Intercept and inspect REQ/RES payloads with a dedicated inspector.
- **Active URL Blocking:** Block unwanted trackers or resources on the fly.
- **Media Asset Manager:** Preview images with high-resolution scroll support and batch download capabilities.

### 3. Professional Intelligence (V1.2.0+)
- **Selector Discovery:** Instant scan of all IDs, Classes, and Attributes (data-*, href, name) on any page.
- **Searchable Selector Listbox:** Quick-select the perfect selector for your automation steps.
- **Robust JS Execution:** Sandbox-wrapped JavaScript execution with detailed error reporting back to Rust.

---

## 🛠 Tech Stack
- **Core:** Rust (Tokio Async Runtime)
- **Browser Control:** Chromoxide (CDP Implementation)
- **UI Framework:** eframe / egui (GPU Accelerated)
- **Data Engine:** Serde / regex / scraper

---

## 🚦 Getting Started

### Prerequisites
- **Google Chrome** or **Chromium** installed.
- **Rust Toolchain** (cargo) installed.

### Installation
1. Clone the repository:
   ```bash
   git clone https://github.com/yourusername/auto-crawler.git
   cd auto-crawler
   ```
2. Run the application:
   ```bash
   cargo run --release
   ```

### Usage
1. **Target Selection:** Choose an output directory for logs and extracted data.
2. **Launch Instance:** Start a fresh browser instance with a single click.
3. **Capture:** Select a tab and use the **Command Center** to extract HTML, capture full-page mirrors, or monitor network traffic.
4. **Automate:** Open the **AUTO** window, scan for selectors, and build your robot using visual blocks.

---

## 📂 Project Architecture
```text
src/
 ├─ core/
 │   ├─ automation/  # DSL, Engine, and Step logic
 │   ├─ browser/     # CDP and Browser lifecycle management
 │   └─ events/      # Central Event Bus
 ├─ ui/              # Modular UI Panels (Scrape, Media, Network, etc.)
 └─ state.rs         # Unified Application State
```

---

## 📝 License
Copyright © 2026. Built for high-precision data extraction.
