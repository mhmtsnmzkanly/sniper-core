use thiserror::Error;

/// Centralized error type for Sniper Studio.
/// Leverages `thiserror` for clean, descriptive error messages and automatic conversions.
#[derive(Error, Debug)]
pub enum AppError {
    /// Errors related to file system operations.
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),

    /// Errors in application configuration or .env file.
    #[error("Configuration Error: {0}")]
    Config(String),

    /// Failures in spawning or controlling the browser process.
    #[error("Browser Process Error: {0}")]
    Browser(String),

    /// Specific errors returned by the Chrome DevTools Protocol (CDP).
    #[error("CDP Protocol Error: {0}")]
    Cdp(String),

    /// Network-level connectivity or protocol errors.
    #[error("Network Error: {0}")]
    Network(String),

    /// Errors from the underlying HTTP client (rquest).
    #[error("HTTP Client Error: {0}")]
    HttpClient(#[from] rquest::Error),

    /// Failures during the execution of automation steps.
    #[error("Automation Error: {0}")]
    Automation(String),

    /// AI-related processing or API errors.
    #[error("AI/Translation Error: {0}")]
    Ai(String),

    /// JSON serialization or deserialization failures.
    #[error("Serialization Error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Malformed URL errors.
    #[error("URL Parse Error: {0}")]
    UrlParse(#[from] url::ParseError),

    /// Unexpected internal application state failures.
    #[error("Internal Engine Error: {0}")]
    Internal(String),

    /// Triggered when a requested target (Tab, File, Resource) is missing.
    #[error("Resource Not Found: {0}")]
    NotFound(String),
}

pub type AppResult<T> = Result<T, AppError>;
