use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration Error: {0}")]
    Config(String),

    #[error("Browser Error: {0}")]
    Browser(String),

    #[error("Network Error: {0}")]
    Network(String),

    #[error("HTTP Client Error: {0}")]
    HttpClient(#[from] rquest::Error),

    #[error("Automation Error in step {0}: {1}")]
    Automation(usize, String),

    #[error("Translation Error: {0}")]
    Translation(String),

    #[error("Serialization Error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("URL Parse Error: {0}")]
    UrlParse(#[from] url::ParseError),

    #[error("Internal Error: {0}")]
    Internal(String),

    #[error("Resource Not Found: {0}")]
    NotFound(String),
}

pub type AppResult<T> = Result<T, AppError>;
