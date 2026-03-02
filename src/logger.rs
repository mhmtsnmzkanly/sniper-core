use crate::state::LogEntry;
use tokio::sync::mpsc;
use tracing_subscriber::{Layer, registry::LookupSpan, prelude::*};
use tracing_appender::non_blocking::WorkerGuard;

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
            message: visitor.message,
            level: *event.metadata().level(),
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        };
        
        let _ = self.sender.send(entry);
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

pub fn init_logging(sender: mpsc::UnboundedSender<LogEntry>) -> (WorkerGuard, String) {
    let now = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let log_filename = format!("{}.log", now);
    
    // Logları logs/ klasörüne kaydet
    let _ = std::fs::create_dir_all("logs");
    let file_appender = tracing_appender::rolling::never("logs", &log_filename);
    let (non_blocking_file, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout)) // Console
        .with(tracing_subscriber::fmt::layer().with_writer(non_blocking_file)) // File
        .with(GuiLoggerLayer::new(sender)) // GUI
        .init();

    (guard, now)
}
