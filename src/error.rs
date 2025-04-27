#[derive(Debug)]
pub enum WalError {
    SegmentFull,
    DeserializationError,
    InvalidLength,
    InvalidSignature,
    SeekFailure,
    WriteFailure,
    ReadFailure,
    OpenFailure,
}
