use std::fmt::{Display, Formatter};

#[derive(Debug, PartialEq)]
pub enum WalError {
    ConfigError(String),
    LogTooLarge,
    IoError(String),
    LockError(String),
    SerializationError(String),
    DeserializationError(String),
}

impl Display for WalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            WalError::ConfigError(msg) => write!(f, "Configuration Error: {}", msg),
            WalError::LogTooLarge => write!(f, "Log size exceeds the page limit"),
            WalError::IoError(msg) => write!(f, "IO Error: {}", msg),
            WalError::LockError(msg) => write!(f, "Lock Error: {}", msg),
            WalError::SerializationError(msg) => write!(f, "Serialization Error: {}", msg),
            WalError::DeserializationError(msg) => write!(f, "Deserialization Error: {}", msg),
        }
    }
}
