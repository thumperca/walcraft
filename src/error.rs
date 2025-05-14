#[derive(Debug)]
pub enum WalError {
    ConfigError(String),
    IoError(String),
    LockError(String),
    SerializationError(String),
    DeserializationError(String),
}
