use chromiumoxide::Page;
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use base64::prelude::*;
use crate::core::error::{AppResult, AppError};
use crate::core::events::AppEvent;
use crate::state::{NetworkRequest, MediaAsset};
use crate::ui::scrape::emit;
use chromiumoxide::cdp::browser_protocol::network::{
    EventRequestWillBeSent, EventResponseReceived, EventLoadingFinished, GetResponseBodyParams
};
use chromiumoxide::cdp::js_protocol::runtime::EventConsoleApiCalled;

pub struct NetworkHandler;

impl NetworkHandler {
    pub async fn start_tab_listeners(
        page: Arc<Page>, 
        tab_id: String, 
        port: u16,
        active: Arc<AtomicBool>
    ) -> AppResult<()> {
        let mut network_events = page.event_listener::<EventRequestWillBeSent>().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let mut response_events = page.event_listener::<EventResponseReceived>().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let mut finished_events = page.event_listener::<EventLoadingFinished>().await.map_err(|e| AppError::Browser(e.to_string()))?;
        let mut console_events = page.event_listener::<EventConsoleApiCalled>().await.map_err(|e| AppError::Browser(e.to_string()))?;

        let tid_inner = tab_id.clone();

        tokio::spawn(async move {
            let mut pending_responses: HashMap<String, (String, String)> = HashMap::new();
            loop {
                if !active.load(Ordering::Relaxed) {
                    tracing::info!("[NETWORK] Listener stop signal received for tab {}", tid_inner);
                    break;
                }

                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(500)) => { continue; }
                    
                    Some(e) = network_events.next() => {
                        let res_type = e.r#type.as_ref().map(|t| format!("{:?}", t)).unwrap_or_else(|| "Other".into());
                        let req = NetworkRequest {
                            request_id: e.request_id.as_ref().to_string(),
                            url: e.request.url.clone(),
                            method: e.request.method.clone(),
                            resource_type: res_type,
                            status: None, request_body: None, response_body: None,
                        };
                        emit(AppEvent::NetworkRequestSent(tid_inner.clone(), req));
                    }

                    Some(e) = response_events.next() => {
                        let rid = e.request_id.as_ref().to_string();
                        pending_responses.insert(rid.clone(), (e.response.url.clone(), e.response.mime_type.clone()));
                        let page_clone = page.clone(); 
                        let rid_clone = e.request_id.clone(); 
                        let tid_res = tid_inner.clone(); 
                        let status = e.response.status as u16;
                        
                        tokio::spawn(async move {
                            tokio::time::sleep(Duration::from_millis(400)).await;
                            if let Ok(res) = page_clone.execute(GetResponseBodyParams::new(rid_clone.clone())).await {
                                emit(AppEvent::NetworkResponseReceived(tid_res, rid_clone.as_ref().to_string(), status, Some(res.result.body)));
                            }
                        });
                    }

                    Some(e) = finished_events.next() => {
                        let rid = e.request_id.as_ref().to_string();
                        if let Some((url, mime)) = pending_responses.remove(&rid) {
                            Self::handle_media_capture(page.clone(), tid_inner.clone(), port, rid, url, mime).await;
                        }
                    }

                    Some(e) = console_events.next() => {
                        let msg = e.args.iter().map(|v| v.value.as_ref().map(|v| v.to_string()).unwrap_or("undefined".into())).collect::<Vec<_>>().join(" ");
                        emit(AppEvent::ConsoleLogAdded(tid_inner.clone(), msg));
                    }
                    
                    else => break,
                }
            }
        });
        Ok(())
    }

    async fn handle_media_capture(
        page: Arc<Page>, 
        tab_id: String, 
        port: u16,
        request_id: String, 
        url: String, 
        mime: String
    ) {
        let lm = mime.to_lowercase();
        let lu = url.to_lowercase();
        
        let is_video = lm.contains("video") || lm.contains("mpegurl") || lm.contains("dash+xml") || 
                       lu.ends_with(".m3u8") || lu.ends_with(".ts") || lu.ends_with(".mpd") || lu.ends_with(".m4s");
        
        let is_sniffable = lm.contains("image") || is_video || lm.contains("audio") || 
                          lm.contains("font") || lm.contains("style") || lm.contains("script") || 
                          url.ends_with(".svg") || url.ends_with(".css") || url.ends_with(".js");

        if is_sniffable {
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(600)).await;
                if let Ok(res) = page.execute(GetResponseBodyParams::new(request_id)).await {
                    let binary_data = if res.result.base64_encoded { 
                        BASE64_STANDARD.decode(&res.result.body).ok() 
                    } else { 
                        Some(res.result.body.into_bytes()) 
                    };

                    if let Some(data) = binary_data {
                        let name = url.split('/').last().unwrap_or("unknown").to_string();
                        
                        emit(AppEvent::MediaCaptured(tab_id.clone(), MediaAsset { 
                            name: name.clone(), 
                            url: url.clone(), 
                            mime_type: mime.clone(), 
                            size_bytes: data.len(), 
                            data: Some(data),
                            thumbnail: None 
                        }));

                        if is_video && !url.contains(".ts") && !url.contains(".m4s") {
                            if let Ok(Some(thumb)) = crate::core::browser::BrowserManager::capture_video_thumbnail(port, tab_id.clone(), url.clone()).await {
                                emit(AppEvent::MediaCaptured(tab_id, MediaAsset { 
                                    name, url, mime_type: mime, size_bytes: 0, data: None, thumbnail: Some(thumb) 
                                }));
                            }
                        }
                    }
                }
            });
        }
    }
}
