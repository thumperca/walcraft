use crate::WalConfig;

/// Header for heap file that stores metadata on the file
///
/// This metadata is essential when reading the file back
/// as the version or page_size may changes over time
struct Header {
    version: usize,
    page_size: usize,
    num_pages: usize,
}

impl Header {
    /// Creates a new header with the given version, page size and number of pages
    fn new(version: usize, page_size: usize) -> Self {
        Header {
            version,
            page_size,
            num_pages: 0,
        }
    }

    /// Serializes the header to a byte vector
    fn as_bytes(&self) -> [u8; 4096] {
        let mut data = [0; 4096];

        // write the header signature
        data[0..4].copy_from_slice("HEAD".as_bytes());

        // write the version, page size and number of pages
        data[4..12].copy_from_slice(&self.version.to_le_bytes());
        data[12..20].copy_from_slice(&self.page_size.to_le_bytes());
        data[20..28].copy_from_slice(&self.num_pages.to_le_bytes());

        data
    }
}

impl TryFrom<&[u8]> for Header {
    type Error = String;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        // header shall be at least 28 bytes long
        if data.len() < 28 {
            return Err("Header length is too short for serialization".to_string());
        }

        // ensure the first 4 bytes are "HEAD"
        let sign = &data[0..4];
        if sign != "HEAD".as_bytes() {
            return Err(format!("Invalid header signature: {:?}", sign));
        }

        // read the version, page size and number of pages
        let version = usize::from_le_bytes(data[4..12].try_into().unwrap());
        let page_size = usize::from_le_bytes(data[12..20].try_into().unwrap());
        let num_pages = usize::from_le_bytes(data[20..28].try_into().unwrap());

        Ok(Header {
            version,
            page_size,
            num_pages,
        })
    }
}

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
