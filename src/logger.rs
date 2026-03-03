use crate::state::LogEntry;
use tokio::sync::mpsc;
use tracing_subscriber::{Layer, registry::LookupSpan, prelude::*};
use std::sync::{Arc, Mutex};
use std::path::PathBuf;

lazy_static::lazy_static! {
    static ref LOG_BUFFER: Arc<Mutex<Vec<LogEntry>>> = Arc::new(Mutex::new(Vec::new()));
    static ref LOG_PATH: Arc<Mutex<Option<PathBuf>>> = Arc::new(Mutex::new(None));
}

pub struct GuiLoggerLayer {
    sender: mpsc::UnboundedSender<LogEntry>,
}

impl GuiLoggerLayer {
    pub fn new(sender: mpsc::UnboundedSender<LogEntry>) -> Self {
        Self { sender }
    }
}

impl<S> Layer<S> for GuiLoggerLayer
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let mut visitor = LogVisitor::new();
        event.record(&mut visitor);
        
        let entry = LogEntry {
            message: visitor.message.clone(),
            level: *event.metadata().level(),
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        };
        
        // Always send to GUI
        let _ = self.sender.send(entry.clone());

        // Buffer logic
        let path_lock = LOG_PATH.lock().unwrap();
        if path_lock.is_none() {
            // No path set yet, buffer in memory
            let mut buffer = LOG_BUFFER.lock().unwrap();
            buffer.push(entry);
        } else if let Some(path) = path_lock.as_ref() {
            // Path set, write directly to file (buffered via std::io if needed, but here simple append)
            let log_file_path = path.clone();
            let mut buffer = LOG_BUFFER.lock().unwrap();
            
            // If there's anything in the buffer, we should have flushed it in set_log_path, 
            // but just in case of race conditions:
            if !buffer.is_empty() {
                if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&log_file_path) {
                    use std::io::Write;
                    for e in buffer.drain(..) {
                        let _ = writeln!(file, "[{}] [{}] {}", e.timestamp, e.level, e.message);
                    }
                }
            }

            if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&log_file_path) {
                use std::io::Write;
                let _ = writeln!(file, "[{}] [{}] {}", entry.timestamp, entry.level, entry.message);
            }
        }
    }
}

struct LogVisitor {
    message: String,
}

impl LogVisitor {
    fn new() -> Self {
        Self { message: String::new() }
    }
}

impl tracing::field::Visit for LogVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        }
    }
}

pub fn init_logging(sender: mpsc::UnboundedSender<LogEntry>) -> String {
    let now = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout)) // Console
        .with(GuiLoggerLayer::new(sender)) // GUI & Buffered File
        .init();

    now
}

pub fn set_log_path(dir: PathBuf, session_ts: &str) {
    let log_file_path = dir.join(format!("sniper_{}.log", session_ts));
    
    // Flush buffer to file first
    let mut buffer = LOG_BUFFER.lock().unwrap();
    if let Ok(_) = std::fs::create_dir_all(&dir) {
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&log_file_path) {
            use std::io::Write;
            for entry in buffer.drain(..) {
                let _ = writeln!(file, "[{}] [{}] {}", entry.timestamp, entry.level, entry.message);
            }
            let _ = writeln!(file, "--- LOG PATH ACTIVATED ---");
        }
    }

    let mut path_lock = LOG_PATH.lock().unwrap();
    *path_lock = Some(log_file_path);
    
    tracing::info!("[LOGGER] Log file established at: {:?}", dir);
}
