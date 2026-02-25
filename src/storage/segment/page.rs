use crate::error::WalError;
use crate::PAGE_MULTIPLIER;

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
#[derive(Debug)]
pub(crate) struct Page {
    pub id: u32,
    max_size: usize,
    pub is_dirty: bool,
    data: Vec<u8>,
    checksum: u32,
}

impl Page {
    const MAGIC: &'static [u8; 4] = b"PAGE";

    /// Create a new empty page
    pub fn new(id: u32, size: usize) -> Self {
        assert_ne!(id, 0);
        Page {
            id,
            max_size: size,
            is_dirty: false,
            data: Vec::with_capacity(size),
            checksum: 0,
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
        self.max_size - (signature_size + checksum_size + page_id_size)
    }

    /// Calculate how many free bytes are available in the current page
    pub fn size_available(&self) -> usize {
        self.size_data() - self.data.len()
    }

    /// Add a new item to the page
    pub fn add(&mut self, data: &[u8]) -> bool {
        // check if there is enough space
        if self.data.len() + data.len() > self.size_data() {
            return false;
        }
        // add the data
        self.data.extend_from_slice(data);
        self.is_dirty = true;
        true
    }

    /// Read individual records from the page
    pub fn read(&self, length_header: usize) -> Vec<Vec<u8>> {
        let mut d = Vec::new();
        // convert a single bytes sequence to vector of individual sequences
        let mut pointer = 0;
        loop {
            let start = pointer + length_header;
            if start >= self.data.len() {
                break;
            }
            let mut length_bytes = (&self.data[pointer..start]).to_vec();
            while length_bytes.len() < 4 {
                length_bytes.push(0);
            }
            let length = u32::from_le_bytes(length_bytes.try_into().unwrap()) as usize;
            if length == 0 || start >= self.data.len() {
                break;
            }
            let end = start + length;
            let data = (&self.data[start..end]).to_vec();
            d.push(data);
            pointer = end;
        }
        d
    }

    /// Utility method to compute checksum over a byte slice
    fn compute_checksum(data: &[u8]) -> u32 {
        crc32fast::hash(data)
    }

    /// Convert the page to bytes array
    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.max_size);
        let mut data = vec![0u8; self.size_data()];
        data[..self.data.len()].copy_from_slice(&self.data);
        bytes.extend_from_slice(Self::MAGIC);
        bytes.extend_from_slice(&self.id.to_le_bytes());
        bytes.extend_from_slice(&data);
        // compute checksum over everything except the checksum field itself
        let checksum = Self::compute_checksum(&bytes);
        bytes.extend_from_slice(&checksum.to_le_bytes());
        assert_eq!(bytes.len(), self.max_size); // ensure the page is aligned to the page size
        assert_eq!(bytes.len() % 4096, 0); // ensure the page is aligned to 4 KiB
        bytes
    }
}

impl TryFrom<&[u8]> for Page {
    type Error = WalError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() < PAGE_MULTIPLIER {
            return Err(WalError::DeserializationError(format!(
                "The page is too short at {}",
                data.len()
            )));
        }

        // ensure the first 4 bytes are "PAGE"
        let sign = &data[0..4];
        if sign != Self::MAGIC {
            return Err(WalError::DeserializationError(
                "Invalid page signature".to_string(),
            ));
        }

        // validate checksum
        let stored_checksum = u32::from_le_bytes(data[data.len() - 4..].try_into().unwrap());
        let computed_checksum = Self::compute_checksum(&data[..data.len() - 4]);
        if stored_checksum != computed_checksum {
            return Err(WalError::ChecksumMismatch);
        }

        // read the data
        let id = u32::from_le_bytes(data[4..8].try_into().unwrap());
        let size = data.len();
        let page_data = &data[8..size - 4];

        Ok(Page {
            id,
            max_size: size,
            is_dirty: false,
            data: page_data.to_vec(),
            checksum: stored_checksum,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        // verify checksum is non-zero
        let stored_checksum = u32::from_le_bytes(bytes[bytes.len() - 4..].try_into().unwrap());
        assert_ne!(stored_checksum, 0);

        // convert bytes back to page
        let page = Page::try_from(&bytes[..]);
        assert!(page.is_ok());
        let page = page.unwrap();
        assert_eq!(page.id, 101);
        let data = &page.data[..msg.len()];
        assert_eq!(data, b"Hello World!");
    }

    #[test]
    fn checksum_mismatch() {
        let mut page = Page::new(1, 4096);
        page.add(b"test data");
        let mut bytes = page.as_bytes();

        // corrupt a byte in the payload
        bytes[10] ^= 0xFF;

        let result = Page::try_from(&bytes[..]);
        assert_eq!(result.unwrap_err(), WalError::ChecksumMismatch);
    }
}
