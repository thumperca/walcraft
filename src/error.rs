#[derive(Debug)]
pub enum WalError {
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
