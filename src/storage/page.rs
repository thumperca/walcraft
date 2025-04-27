use crate::error::WalError;

/// Calculate how many bytes are needed to store a given value
/// For example: 1 byte is needed to store 0-255, 2 bytes for 256-65535, etc.
fn bytes_for_value(value: usize) -> usize {
    let mut counter = 1;
    loop {
        if usize::pow(2, counter * 8) > value {
            return counter as usize;
        }
        counter += 1;
    }
}

/// A page is simply a fixed-size block of bytes that the WAL uses as its basic unit of I/O.
///
/// The size of the page is in multiple of 4 KiB. All writes and reads go in page-sized chunks.
///
/// ## Structure of Page
/// - **Page Header** (64-bit) - A small metadata for signature to ensure alignment to page size
///         and a page sequence number.
/// - **Payload Area** (variable length) - A byte region into which you serialize one or more WAL records.
///         The payload area is padded to ensure that the page is aligned to the page size.
/// - **Checksum** (32-bit) - A checksum to ensure the integrity of the page.
///
pub(crate) struct Page {
    pub id: u32,
    size: usize,
    pub is_dirty: bool,
    data: Vec<u8>,
    checksum: u32,
    size_bytes: usize,
}

impl Page {
    /// Create a new empty page
    pub fn new(id: u32, size: usize) -> Self {
        assert_ne!(id, 0);
        Page {
            id,
            size,
            is_dirty: false,
            data: Vec::with_capacity(size),
            checksum: 0,
            size_bytes: bytes_for_value(size),
        }
    }

    /// Calculate how many bytes in the page can be used to store data
    ///
    /// Not all the bytes in the page can be used to store data,
    /// since the page stores a few additional fields, such as:
    /// - Signature (4 bytes)
    /// - Checksum (4 bytes)
    /// - Page ID (4 bytes)
    ///
    /// Returns: the number of bytes available for data storage
    ///
    pub fn size_data(&self) -> usize {
        let signature_size = 4;
        let checksum_size = 4;
        let page_id_size = 4;
        self.size - (signature_size + checksum_size + page_id_size)
    }

    /// Calculate how many free bytes are available in the current page
    pub fn size_available(&self) -> usize {
        self.size_data() - self.data.len()
    }

    /// Add a new item to the page
    pub fn add(&mut self, data: &[u8]) -> bool {
        if self.data.len() + data.len() + self.size_bytes > self.size {
            return false;
        }
        self.data.extend_from_slice(data);
        self.is_dirty = true;
        true
    }

    /// Convert the page to bytes array
    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.size);
        let mut data = vec![0u8; self.size_data()];
        data[..self.data.len()].copy_from_slice(&self.data);
        bytes.extend_from_slice(b"PAGE");
        bytes.extend_from_slice(&self.id.to_le_bytes());
        bytes.extend_from_slice(&data);
        bytes.extend_from_slice(&self.checksum.to_le_bytes());
        assert_eq!(bytes.len(), self.size); // ensure the page is aligned to the page size
        assert_eq!(bytes.len() % 4096, 0); // ensure the page is aligned to 4 KiB
        bytes
    }
}

impl TryFrom<&[u8]> for Page {
    type Error = WalError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() < 16 {
            return Err(WalError::InvalidLength);
        }

        // ensure the first 4 bytes are "PAGE"
        let sign = &data[0..4];
        if sign != b"PAGE" {
            return Err(WalError::InvalidSignature);
        }

        // read the data
        let id = u32::from_le_bytes(data[4..8].try_into().unwrap());
        let checksum = u32::from_le_bytes(data[data.len() - 4..].try_into().unwrap());
        let size = data.len();
        let page_data = &data[8..size - 4];

        Ok(Page {
            id,
            size,
            is_dirty: false,
            data: page_data.to_vec(),
            checksum,
            size_bytes: bytes_for_value(size),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn util_fn() {
        assert_eq!(bytes_for_value(100), 1);
        assert_eq!(bytes_for_value(200), 1);
        assert_eq!(bytes_for_value(300), 2);
        assert_eq!(bytes_for_value(50_000), 2);
        assert_eq!(bytes_for_value(100_000), 3);
    }

    #[test]
    fn available_size() {
        let mut page = Page::new(1, 4096);

        // test empty
        assert_eq!(page.size_data(), 4084);
        assert_eq!(page.size_available(), 4084);

        // test after adding some data
        page.add(&[0; 1084]);
        assert_eq!(page.size_data(), 4084);
        assert_eq!(page.size_available(), 3000);
    }

    #[test]
    fn bytes_conversion() {
        // convert page to bytes
        let mut page = Page::new(101, 4096);
        let msg = b"Hello World!";
        page.add(&msg[..]);
        let bytes = page.as_bytes();
        assert_eq!(bytes.len(), 4096);

        // convert bytes back to page
        let page = Page::try_from(&bytes[..]);
        assert!(page.is_ok());
        let page = page.unwrap();
        assert_eq!(page.id, 101);
        let data = &page.data[..msg.len()];
        assert_eq!(data, b"Hello World!");
    }
}
