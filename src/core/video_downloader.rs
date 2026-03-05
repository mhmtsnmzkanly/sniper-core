use crate::core::error::{AppError, AppResult};
use std::path::{Path, PathBuf};

fn sanitize_name(raw: &str) -> String {
    let mut out = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out.trim_matches('_').to_string()
}

fn derive_video_name(url: &str, preferred_name: Option<&str>) -> String {
    let preferred = preferred_name.unwrap_or("").trim();
    if !preferred.is_empty() {
        let stem = preferred
            .split('.')
            .next()
            .map(str::trim)
            .unwrap_or(preferred);
        let cleaned = sanitize_name(stem);
        if !cleaned.is_empty() {
            return cleaned;
        }
    }
    let fallback = url
        .split('/')
        .last()
        .unwrap_or("video")
        .split('?')
        .next()
        .unwrap_or("video")
        .split('.')
        .next()
        .unwrap_or("video");
    let cleaned = sanitize_name(fallback);
    if cleaned.is_empty() {
        "video".to_string()
    } else {
        cleaned
    }
}

pub fn is_hls_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains(".m3u8")
}

fn ensure_unique_output(mut candidate: PathBuf) -> PathBuf {
    if !candidate.exists() {
        return candidate;
    }
    let stem = candidate
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video")
        .to_string();
    let ext = candidate
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("mp4")
        .to_string();
    let parent = candidate.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    candidate = parent.join(format!("{}_{}.{}", stem, ts, ext));
    candidate
}

/// KOD NOTU: DRM olmayan HLS akışlarını ffmpeg ile tek dosya (`.mp4`) halinde indirir.
pub async fn download_hls_to_output(output_dir: &Path, hls_url: &str, preferred_name: Option<&str>) -> AppResult<PathBuf> {
    if !is_hls_url(hls_url) {
        return Err(AppError::Internal("Video downloader currently supports HLS (.m3u8) URLs only".to_string()));
    }

    let downloads_dir = output_dir.join("video_downloads");
    std::fs::create_dir_all(&downloads_dir).map_err(AppError::Io)?;

    let base_name = derive_video_name(hls_url, preferred_name);
    let target = ensure_unique_output(downloads_dir.join(format!("{}.mp4", base_name)));

    let output = tokio::process::Command::new("ffmpeg")
        .arg("-y")
        .arg("-nostdin")
        .arg("-loglevel")
        .arg("error")
        .arg("-protocol_whitelist")
        .arg("file,http,https,tcp,tls,crypto")
        .arg("-i")
        .arg(hls_url)
        .arg("-c")
        .arg("copy")
        .arg("-bsf:a")
        .arg("aac_adtstoasc")
        .arg(&target)
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to start ffmpeg. Is it installed? {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(AppError::Internal(format!("ffmpeg failed: {}", stderr)));
    }

    Ok(target)
}
