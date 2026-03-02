# Auto-Crawler: Advanced Web Novel Scraper & Localizer

Auto-Crawler is a multi-novel, high-performance CLI tool for scraping and localizing web novels from BookToki using the Google Gemini API.

## 🚀 Key Features

- **Novel-Aware Organization**: Saves content in nested directories: `raw/{novel_name}/{chapter}.txt`.
- **Chronological Scraping**: Automatically detects pagination and scrapes from Chapter 1 onwards.
- **Smart Checkpointing**: Resumes progress by checking for existing files in `translated/{novel_name}/`.
- **AI Localization**: Uses Gemini 1.5 Flash with a professional localization prompt for Turkish.
- **CLI Power**: Powered by `clap` for easy URL input and management.
- **Safety First**: Implements rate-limiting (1s delay) and custom User-Agents for anti-bot protection.

## 📋 Prerequisites

- **Rust**: [Install Rust](https://rustup.rs/) (1.75+).
- **Gemini API Key**: [Get your key here](https://aistudio.google.com/app/apikey).

## 🛠️ Setup

1. **Environment**:
   Create a `.env` file or set the variable:
   ```bash
   export GEMINI_API_KEY="your_api_key"
   ```

2. **Installation**:
   ```bash
   cd /home/duldul/Masaüstü/auto-crawler
   cargo build --release
   ```

## 🎮 Usage

Run the crawler by providing the novel's main page URL:

```bash
cargo run --release -- --url "https://booktoki469.com/novel/263943"
```

### Options:
- `-u` / `--url`: The main page URL of the novel you want to scrape.

## 📂 Data Structure

- `raw/{novel_name}/`: Original Korean chapters.
- `translated/{novel_name}/`: Localized Turkish chapters.

## 🛡️ Disclaimer
Use responsibly. This tool is for personal educational use only. Respect the copyright and terms of service of the content providers.
