#[derive(Debug)]
pub enum WalError {
    SegmentFull,
    DeserializationError,
    InvalidLength,
    InvalidSignature,
    SeekFailure,
    ReadFailure,
    OpenFailure,
}
