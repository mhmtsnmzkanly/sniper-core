use crate::state::LogEntry;
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::prelude::*;

lazy_static::lazy_static! {
    static ref LOG_BUFFER: Arc<Mutex<Vec<LogEntry>>> = Arc::new(Mutex::new(Vec::new()));
    static ref LOG_PATH: Arc<Mutex<Option<PathBuf>>> = Arc::new(Mutex::new(None));
    static ref CHROME_LOG_PATH: Arc<Mutex<Option<PathBuf>>> = Arc::new(Mutex::new(None));
}

pub fn init_logging(log_tx: mpsc::UnboundedSender<LogEntry>) -> String {
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    
    let buffer_layer = LogBufferLayer { tx: log_tx };
    
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with(tracing_subscriber::fmt::layer().with_span_events(FmtSpan::CLOSE))
        .with(buffer_layer)
        .init();

    timestamp
}

pub fn set_log_path(path: PathBuf, session_ts: &str) {
    let mut lock = LOG_PATH.lock().unwrap();
    let final_path = path.join(format!("session_{}.log", session_ts));
    *lock = Some(final_path);

    let mut chrome_lock = CHROME_LOG_PATH.lock().unwrap();
    *chrome_lock = Some(path.join(format!("chrome_session_{}.log", session_ts)));
}

pub fn write_chrome_log_line(line: &str) {
    if let Ok(path_lock) = CHROME_LOG_PATH.lock() {
        if let Some(path) = &*path_lock {
            if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                use std::io::Write;
                let ts = chrono::Local::now().format("%H:%M:%S").to_string();
                let _ = writeln!(file, "[{}] {}", ts, line);
            }
        }
    }
}

struct LogBufferLayer {
    tx: mpsc::UnboundedSender<LogEntry>,
}

impl<S> tracing_subscriber::Layer<S> for LogBufferLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut message = String::new();
        let mut visitor = MessageVisitor { message: &mut message };
        event.record(&mut visitor);

        let entry = LogEntry {
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
            level: event.metadata().level().to_string(),
            message,
        };

        // Send to UI
        let _ = self.tx.send(entry.clone());

        // Add to global buffer
        if let Ok(mut buffer) = LOG_BUFFER.lock() {
            buffer.push(entry.clone());
            if buffer.len() > 1000 {
                buffer.remove(0);
            }
        }

        // Write to file if path set
        if let Ok(path_lock) = LOG_PATH.lock() {
            if let Some(path) = &*path_lock {
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path) {
                    use std::io::Write;
                    let _ = writeln!(file, "[{}] [{}] {}", entry.timestamp, entry.level, entry.message);
                }
            }
        }
    }
}

struct MessageVisitor<'a> {
    message: &'a mut String,
}

impl<'a> tracing::field::Visit for MessageVisitor<'a> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            *self.message = format!("{:?}", value);
        }
    }
}
