pub(crate) const HEADER_SIZE: usize = 4096;
use crate::error::WalError;

/// Header for heap file that stores metadata on the file
///
/// This metadata is essential when reading the file back
/// as the version or page_size may changes over time
pub(crate) struct Header {
    /// Version of WAL file
    version: usize,
    /// Identifier for the file segment
    segment_id: usize,
    /// Size of a single page in bytes
    pub page_size: usize,
    /// Number of pages in the segment
    pub num_pages: u32,
    /// Flag to indicate if the header is dirty
    pub is_dirty: bool,
    /// Size of length prefix for the data
    /// This is based on the page size
    pub length_prefix: usize,
}

impl Header {
    /// Creates a new header
    pub(crate) fn new(segment_id: usize, page_size: usize) -> Self {
        Header {
            version: crate::WAL_VERSION,
            segment_id,
            page_size,
            num_pages: 0,
            is_dirty: true,
            length_prefix: Self::bytes_for_value(page_size),
        }
    }

    /// Calculate how many bytes are needed to store a given value
    /// For example: 1 byte is needed to store 0-255, 2 bytes for 256-65535, etc.
    pub(crate) fn bytes_for_value(value: usize) -> usize {
        let mut counter = 1;
        loop {
            if usize::pow(2, counter * 8) > value {
                return counter as usize;
            }
            counter += 1;
        }
    }

    /// Serializes the header to a byte array
    pub(crate) fn as_bytes(&self) -> [u8; HEADER_SIZE] {
        let mut data = [0; HEADER_SIZE];

        // write the header signature
        data[0..4].copy_from_slice("HEAD".as_bytes());

        // write data
        data[4..12].copy_from_slice(&self.version.to_le_bytes());
        data[12..20].copy_from_slice(&self.segment_id.to_le_bytes());
        data[20..28].copy_from_slice(&self.page_size.to_le_bytes());
        data[28..32].copy_from_slice(&self.num_pages.to_le_bytes());

        data
    }
}

/// Create a new header from a byte array
impl TryFrom<&[u8]> for Header {
    type Error = WalError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        // header shall be at least 32 bytes long
        if data.len() < 32 {
            return Err(WalError::InvalidLength);
        }

        // ensure the first 4 bytes are "HEAD"
        let sign = &data[0..4];
        if sign != "HEAD".as_bytes() {
            return Err(WalError::InvalidSignature);
        }

        // read the data
        let version = usize::from_le_bytes(data[4..12].try_into().unwrap());
        let segment_id = usize::from_le_bytes(data[12..20].try_into().unwrap());
        let page_size = usize::from_le_bytes(data[20..28].try_into().unwrap());
        let num_pages = u32::from_le_bytes(data[28..32].try_into().unwrap());

        Ok(Header {
            version,
            segment_id,
            page_size,
            num_pages,
            is_dirty: false,
            length_prefix: Self::bytes_for_value(page_size),
        })
    }
}

#[test]
fn conversion() {
    let mut header = Header::new(1, 4096);
    header.num_pages = 10;
    let bytes = header.as_bytes();
    let header = Header::try_from(&bytes[..]);
    assert!(header.is_ok());
    let header = header.unwrap();
    assert_eq!(header.segment_id, 1);
    assert_eq!(header.page_size, 4096);
    assert_eq!(header.num_pages, 10);
}

#[test]
fn util_fn() {
    assert_eq!(Header::bytes_for_value(100), 1);
    assert_eq!(Header::bytes_for_value(200), 1);
    assert_eq!(Header::bytes_for_value(300), 2);
    assert_eq!(Header::bytes_for_value(50_000), 2);
    assert_eq!(Header::bytes_for_value(100_000), 3);
}
