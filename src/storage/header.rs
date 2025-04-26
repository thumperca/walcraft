/// Header for heap file that stores metadata on the file
///
/// This metadata is essential when reading the file back
/// as the version or page_size may changes over time
pub(crate) struct Header {
    version: usize,
    segment_id: usize,
    page_size: usize,
    num_pages: usize,
}

impl Header {
    /// Creates a new header
    fn new(segment_id: usize, page_size: usize) -> Self {
        Header {
            version: crate::WAL_VERSION,
            segment_id,
            page_size,
            num_pages: 0,
        }
    }

    /// Serializes the header to a byte array
    fn as_bytes(&self) -> [u8; 4096] {
        let mut data = [0; 4096];

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
    type Error = String;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        // header shall be at least 32 bytes long
        if data.len() < 32 {
            return Err("Header length is too short for serialization".to_string());
        }

        // ensure the first 4 bytes are "HEAD"
        let sign = &data[0..4];
        if sign != "HEAD".as_bytes() {
            return Err(format!("Invalid header signature: {:?}", sign));
        }

        // read the data
        let version = usize::from_le_bytes(data[4..12].try_into().unwrap());
        let segment_id = usize::from_le_bytes(data[12..20].try_into().unwrap());
        let page_size = usize::from_le_bytes(data[20..28].try_into().unwrap());
        let num_pages = usize::from_le_bytes(data[28..32].try_into().unwrap());

        Ok(Header {
            version,
            segment_id,
            page_size,
            num_pages,
        })
    }
}
