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

        // Use a tiny scope for the lock to prevent deadlocks if tracing is called recursively
        let path = {
            let path_lock = LOG_PATH.lock().unwrap();
            path_lock.clone()
        };

        if let Some(log_file_path) = path {
            // Path set, write to file
            if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&log_file_path) {
                use std::io::Write;
                let _ = writeln!(file, "[{}] [{}] {}", entry.timestamp, entry.level, entry.message);
            }
        } else {
            // No path set yet, buffer in memory
            let mut buffer = LOG_BUFFER.lock().unwrap();
            buffer.push(entry);
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
    
    // 1. Flush buffer to file first
    if let Ok(_) = std::fs::create_dir_all(&dir) {
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&log_file_path) {
            use std::io::Write;
            let mut buffer = LOG_BUFFER.lock().unwrap();
            for entry in buffer.drain(..) {
                let _ = writeln!(file, "[{}] [{}] {}", entry.timestamp, entry.level, entry.message);
            }
            let _ = writeln!(file, "--- LOG PATH ACTIVATED ---");
        }
    }

    // 2. Set the global path lock (in a separate scope)
    {
        let mut path_lock = LOG_PATH.lock().unwrap();
        *path_lock = Some(log_file_path);
    }
    
    // 3. Now we can safely log because the lock is released
    tracing::info!("[LOGGER] Log file established at: {:?}", dir);
}
