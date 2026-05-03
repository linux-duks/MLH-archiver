use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid config: {0}")]
    Invalid(String),

    #[error("Config error: {0}")]
    Other(String),
}

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
