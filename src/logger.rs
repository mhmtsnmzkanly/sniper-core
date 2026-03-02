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

        // Buffer for file writing if path not yet set
        let path_lock = LOG_PATH.lock().unwrap();
        if path_lock.is_none() {
            let mut buffer = LOG_BUFFER.lock().unwrap();
            buffer.push(entry);
        } else if path_lock.is_some() {
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

pub fn set_log_path(path: PathBuf) {
    let mut path_lock = LOG_PATH.lock().unwrap();
    *path_lock = Some(path.clone());

    // Flush buffer to file
    let now = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let log_file_path = path.join(format!("{}.log", now));
    
    let mut buffer = LOG_BUFFER.lock().unwrap();
    if let Ok(_) = std::fs::create_dir_all(&path) {
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&log_file_path) {
            use std::io::Write;
            for entry in buffer.drain(..) {
                let _ = writeln!(file, "[{}] [{}] {}", entry.timestamp, entry.level, entry.message);
            }
        }
    }

    // Start a background task to keep flushing the buffer
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let mut buffer = LOG_BUFFER.lock().unwrap();
            if !buffer.is_empty() {
                if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&log_file_path) {
                    use std::io::Write;
                    for entry in buffer.drain(..) {
                        let _ = writeln!(file, "[{}] [{}] {}", entry.timestamp, entry.level, entry.message);
                    }
                }
            }
        }
    });
}
