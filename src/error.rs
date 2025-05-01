#[derive(Debug)]
pub enum WalError {
    IoError(String), // todo: replace lot of specialized errors below with IoError
    MetaFileError,
    SegmentFull,
    DeserializationError,
    InvalidLength,
    InvalidSignature,
    SeekFailure,
    WriteFailure,
    ReadFailure,
    OpenFailure,
    GcFailure,
}
