use thiserror::Error;

/// Errors that occur during configuration loading and deserialization.
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid config: {0}")]
    Invalid(String),

    #[error("Config error: {0}")]
    Other(String),
}

/// Errors that occur during email parsing.
///
/// These are recoverable when [`fail_on_parsing_error`](crate::config::AppConfig::fail_on_parsing_error)
/// is `false` — the email is skipped and processing continues.
#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Failed to decode email: {0}")]
    DecodeError(String),

    #[error("Failed to extract headers: {0}")]
    HeaderError(String),

    #[error("Failed to extract body: {0}")]
    BodyError(String),

    #[error("Failed to parse date: {0}")]
    DateParseError(String),

    #[error("Failed to extract patches: {0}")]
    PatchError(String),

    #[error("Email has no Message-ID")]
    NoMessageId,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
