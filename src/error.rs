#[derive(Debug)]
pub enum WalError {
    IoError(String),
    DeserializeError(String),
}
