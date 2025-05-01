mod meta;
mod segment;

use self::meta::Meta;
use self::segment::FileSegment;
use crate::WalConfig;
use std::collections::VecDeque;

/// Storage manager for the WAL
///
/// This module is responsible for the actual IO operations, including
/// - Reading data from IO
/// - Writing data to IO
/// - Garbage Collection
///
struct Storage {
    config: WalConfig,
    segments: VecDeque<FileSegment>,
    meta: Meta,
}

impl Storage {
    pub fn new(config: WalConfig) -> Self {
        todo!()
    }
}
