mod header;

use self::header::Header;
use crate::WalConfig;

struct File {
    header: Header,
    inner: std::fs::File,
}

struct Page {
    id: usize,
    num_items: usize,
    data: Vec<u8>,
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
