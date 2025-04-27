mod segment;

use self::segment::FileSegment;
use crate::WalConfig;
use std::collections::VecDeque;

struct Meta {
    gc_start: usize,
    gc_end: usize,
    sizes: Vec<SizeEntry>,
}

struct SizeEntry {
    file_id: usize,
    size: usize,
}

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
