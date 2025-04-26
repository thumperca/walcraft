mod header;
mod page;

use self::header::Header;
use self::page::Page;
use crate::WalConfig;
use std::fs::File;

struct FileSegment {
    header: Header,
    file: File,
    pages: Vec<Page>,
}

struct Meta {
    gc_start: usize,
    gc_end: usize,
    sizes: Vec<SizeEntry>,
}

struct SizeEntry {
    file_id: usize,
    size: usize,
}

struct Heap {
    file: File,
    pages: Vec<Page>,
}

struct Storage {
    config: WalConfig,
    heap: Heap,
    meta: Meta,
}

impl Storage {
    pub fn new(config: WalConfig) -> Self {
        todo!()
    }
}
