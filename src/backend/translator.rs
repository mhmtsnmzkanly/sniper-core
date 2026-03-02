use anyhow::{anyhow, Result};
use tracing::{error, debug, warn, info};
use rquest::header::CONTENT_TYPE;
use serde_json::json;
use std::env;
use std::time::Duration;
use tokio::time::sleep;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Semaphore;
use futures::stream::{self, StreamExt};

pub struct GeminiClient {
    api_key: String,
    client: rquest::Client,
}

impl GeminiClient {
    pub fn new(api_key: Option<String>) -> Result<Self> {
        let api_key = api_key
            .or_else(|| env::var("GEMINI_API_KEY").ok())
            .ok_or_else(|| anyhow!("GEMINI_API_KEY must be set"))?;
            
        let client = rquest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()?;
        Ok(Self { api_key, client })
    }

    pub async fn localize_content(&self, content: &str, context_type: &str) -> Result<String> {
        let mut attempts = 0;
        let max_attempts = 5;
        let mut base_delay = 5;

        loop {
            debug!("Attempt {}/{} for: {}", attempts + 1, max_attempts, context_type);
            
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={}",
                self.api_key
            );

            let prompt = format!(
                "ROLE: Professional Literary Localizer\nCONTEXT: {}\n\nTEXT:\n{}", 
                context_type, 
                content
            );

            let payload = json!({
                "contents": [{"parts": [{"text": prompt}]}],
                "generationConfig": {
                    "temperature": 0.7,
                    "maxOutputTokens": 8192
                }
            });

            let response = self.client.post(&url)
                .header(CONTENT_TYPE, "application/json")
                .json(&payload)
                .send()
                .await?;

            let status = response.status();
            
            if status.is_success() {
                let res_json: serde_json::Value = response.json().await?;
                return if let Some(text) = res_json["candidates"][0]["content"]["parts"][0]["text"].as_str() {
                    sleep(Duration::from_secs(3)).await;
                    Ok(text.trim().to_string())
                } else {
                    error!("Unexpected JSON structure: {:?}", res_json);
                    Err(anyhow!("Gemini response parsing failed"))
                };
            }

            if status.as_u16() == 429 {
                attempts += 1;
                if attempts >= max_attempts {
                    return Err(anyhow!("Gemini quota exceeded after {} retries.", max_attempts));
                }
                warn!("⚠️  Gemini Rate Limit (429) hit. Waiting {}s before retry...", base_delay);
                sleep(Duration::from_secs(base_delay)).await;
                base_delay *= 2;
                continue;
            }

            let err_text = response.text().await?;
            error!("Gemini API Error ({}): {}", status, err_text);
            return Err(anyhow!("Gemini API non-retryable error"));
        }
    }

    pub async fn run_translate_workflow(
        &self, 
        raw_dir: PathBuf, 
        trans_dir: PathBuf, 
        concurrency: usize
    ) -> Result<()> {
        let _ = std::fs::create_dir_all(&trans_dir);
        
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&raw_dir)? {
            let entry = entry?;
            if entry.path().extension().map_or(false, |e| e == "txt") {
                entries.push(entry.path());
            }
        }
        
        entries.sort_by_key(|p| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.replace("chapter_", "").parse::<f32>().ok())
                .unwrap_or(0.0) as i32
        });

        info!("Toplam {} dosya bulundu. Çeviri başlıyor (Paralellik: {})...", entries.len(), concurrency);

        let semaphore = Arc::new(Semaphore::new(concurrency));
        let client = Arc::new(self.clone());

        let mut stream = stream::iter(entries)
            .map(|raw_path| {
                let sem = Arc::clone(&semaphore);
                let client = Arc::clone(&client);
                let trans_dir = trans_dir.clone();
                
                async move {
                    let _permit = sem.acquire().await.unwrap();
                    let file_name = raw_path.file_name().unwrap().to_os_string();
                    let trans_path = trans_dir.join(&file_name);

                    if trans_path.exists() {
                        debug!("Bölüm {:?} zaten çevrilmiş, atlanıyor.", file_name);
                        return;
                    }

                    if let Ok(raw_content) = super::utils::load_file(&raw_path) {
                        let context = file_name.to_string_lossy().to_string();
                        info!("✍️ Çevriliyor: {}", context);
                        
                        let client_ref = client.clone();
                        match client_ref.localize_content(&raw_content, &context).await {
                            Ok(trans_text) => {
                                if let Err(e) = super::utils::save_text_file(&trans_path, &trans_text) {
                                    error!("❌ Kayıt Hatası ({}): {}", context, e);
                                } else {
                                    info!("✅ Tamamlandı: {}", context);
                                }
                            }
                            Err(e) => error!("❌ Çeviri Hatası ({}): {}", context, e),
                        }
                    }
                }
            })
            .buffer_unordered(concurrency);

        while let Some(_) = stream.next().await {}

        info!("✨ Çeviri işlemi başarıyla tamamlandı.");
        Ok(())
    }
}

impl Clone for GeminiClient {
    fn clone(&self) -> Self {
        Self {
            api_key: self.api_key.clone(),
            client: self.client.clone(),
        }
    }
}
