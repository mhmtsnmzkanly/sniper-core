# 🎯 Sniper Studio v1.1.1 Stable
### *Unified Browser Intelligence & Forensic Automation Studio*

Sniper Studio is a professional-grade, multi-target DevTools environment built in **Rust**. It enables high-performance network interception, media sniffing, and automated scraping across multiple browser tabs simultaneously using the Multiple Document Interface (MDI) architecture.

---

## 🚀 Key Features

*   **Multi-Tasking MDI:** Open independent Network, Media, and Storage windows for different browser tabs at once.
*   **Media Forensic Engine:** Real-time binary sniffing of images, videos, and audio streams with 120x120 previews and batch downloading.
*   **Target-Centric Capture:** Save clean HTML snapshots or full Mirror replicas (CSS/JS included) into a structured hierarchy.
*   **Universal Identity:** Intelligent OS detection for using real system profiles or isolated fresh environments.
*   **Data Governance:** Categorized output at `{OUTPUT_DIR}/{CATEGORY}/{DOMAIN}/{PAGE}/` for enterprise forensic organization.
*   **Low-Code Automation:** Sequential operation pipeline and live JavaScript injection with real-time console monitoring.

---

## 📂 Storage Standard
All studio operations follow this strict data protocol:
*   `MEDIA/` - Intercepted binary assets.
*   `NETWORK/` - Traffic logs (`traffic.json`).
*   `HTML/` - Single-page snapshots.
*   `MIRROR/` - Offline site replicas.
*   `*.log` - Integrated session and protocol logs.

---

## 🛠 Usage
1.  **Launch:** `cargo run --release`
2.  **Setup:** Confirm your `OUTPUT_DIR` and select your Browser Identity.
3.  **Command:** Select a tab in the Scrape panel gallery and click **MEDIA** or **NETWORK** to spawn an autonomous inspector.

---

## ⚖ License
MIT License. Created for researchers and developers.
