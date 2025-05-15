use std::fmt::{Display, Formatter};

/// Error handling module for the Write Ahead Log (WAL) implementation
#[derive(Debug, PartialEq)]
pub enum WalError {
    /// Something is wrong with the provided configuration to WAL
    ///
    /// Such as the location of the WAL is inacecssible, page size is not aligned to 4 KB, etc.
    ConfigError(String),
    /// The log size exceeds the page limit
    LogTooLarge,
    /// IO error occurred
    ///
    /// This error is best resolved by using a larger page size to ensure that no single log entry
    /// exceeds the page size.
    IoError(String),
    /// Error while acquiring or releasing a lock (read/write)
    ///
    /// This error occurs when concurrent read and writes are attempted on the WAL.
    /// or when WAL is being read from after writing has started.
    ///
    /// To prevent this error, always read the WAL before writing to it and ensure that the WAL
    /// is fully read or drop the iterator before writing to it.
    LockError(String),
    /// Error while serializing data in serde or bincode libraries
    SerializationError(String),
    /// Error while deserializing data in serde or bincode libraries
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
