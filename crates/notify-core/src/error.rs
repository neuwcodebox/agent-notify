use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, NotifyError>;

#[derive(Debug, Error)]
pub enum NotifyError {
    #[error("configuration file not found")]
    ConfigNotFound,
    #[error("failed to read configuration file {path}: {source}")]
    ConfigRead {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse configuration file {path}: {source}")]
    ConfigParse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("channel \"{0}\" was not found")]
    ChannelNotFound(String),
    #[error("default_channel is not configured")]
    DefaultChannelMissing,
    #[error("{0}")]
    InvalidInput(String),
    #[error("{0}")]
    Validation(String),
    #[error("channel \"{channel}\" is missing environment variable {env}")]
    MissingEnv { channel: String, env: String },
    #[error("channel type \"{0}\" is not available for sending yet")]
    UnsupportedProvider(String),
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to serialize JSON output: {0}")]
    Json(#[from] serde_json::Error),
}

impl NotifyError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ConfigNotFound => "CONFIG_NOT_FOUND",
            Self::ConfigRead { .. } => "CONFIG_READ",
            Self::ConfigParse { .. } => "CONFIG_PARSE",
            Self::ChannelNotFound(_) => "CHANNEL_NOT_FOUND",
            Self::DefaultChannelMissing => "DEFAULT_CHANNEL_MISSING",
            Self::InvalidInput(_) => "INVALID_INPUT",
            Self::Validation(_) => "VALIDATION",
            Self::MissingEnv { .. } => "MISSING_ENV",
            Self::UnsupportedProvider(_) => "UNSUPPORTED_PROVIDER",
            Self::Io { .. } => "IO",
            Self::Json(_) => "JSON",
        }
    }
}
