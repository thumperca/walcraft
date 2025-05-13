mod header;
mod iterator;
mod page;

use self::header::Header;
use self::iterator::PageIterator;
use self::page::Page;
use crate::error::WalError;
use crate::PAGE_MULTIPLIER;
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Deals with a single file that comprises the header plus pages.
///
/// This is where you’ll have functionality to:
/// - Read and write the file header.
/// - Append data to the file in page-sized increments.
/// - Handle synchronization/flush if needed.
pub(crate) struct FileSegment {
    pub(crate) header: Header,
    pages: VecDeque<Page>,
    file: File,
    is_dirty: bool,
    max_size: usize,
}

impl FileSegment {
    /// Creates a new file segment
    ///
    /// This function creates a new empty file on disk as well
    pub fn create_new<P: AsRef<Path>>(
        base_dir: P,
        segment_id: u32,
        page_size: usize,
        max_size: usize,
    ) -> Result<Self, WalError> {
        assert_eq!(page_size % 4096, 0);

        let path = Self::get_path(base_dir, segment_id);

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(path)
            .map_err(|e| {
                WalError::IoError(format!("Failed to create segment {}: {}", segment_id, e))
            })?;

        let header = Header::new(segment_id, page_size);
        Ok(Self {
            header,
            pages: VecDeque::new(),
            file,
            is_dirty: true,
            max_size,
        })
    }

    /// Get the path of the segment file
    pub(crate) fn get_path<P: AsRef<Path>>(base_dir: P, segment_id: u32) -> PathBuf {
        let mut path = PathBuf::from(base_dir.as_ref());
        let width = u32::MAX.to_string().len();
        let file = format!("logs/wal_{:0width$}.bin", segment_id, width = width);
        path.push(file);
        path
    }

    /// Opens an existing file and load it's latest page in memory
    pub fn open_existing<P: AsRef<Path>>(path: P, max_size: usize) -> Result<Self, WalError> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| {
                WalError::IoError(format!(
                    "Failed to open log file ERR_OPEN_77: {} - {:?}",
                    e,
                    path.as_ref()
                ))
            })?;
        let metadata = file.metadata().map_err(|e| {
            WalError::IoError(format!("Failed to get metadata for log file: {}", e))
        })?;

        let mut header_data = [0; 4096];
        file.read_exact(&mut header_data)
            .map_err(|e| WalError::IoError(format!("Failed to open log file: {}", e)))?;
        let header = Header::try_from(&header_data[..])?;

        let mut segment = Self {
            header,
            pages: VecDeque::new(),
            file,
            is_dirty: false,
            max_size,
        };

        // ensure the file size is correct
        let file_size = metadata.len() as usize;
        let expected_size = segment.len();
        // reset the header if corruption is detected
        if expected_size != file_size {
            println!(
                "Correcting possible data corruption in segment {}",
                segment.header.segment_id
            );
            // ceil division
            let num_pages = (file_size + segment.header.page_size - 1) / segment.header.page_size;
            segment.header.num_pages = num_pages as u32;
            segment.header.is_dirty = true;
            segment.sync_header()?;
        }

        // Read the latest page into memory
        if segment.header.num_pages > 0 {
            let page = segment.read_page(segment.header.num_pages)?;
            segment.pages.push_back(page);
        }

        Ok(segment)
    }

    /// Read a specific page from the file
    fn read_page(&mut self, page_id: u32) -> Result<Page, WalError> {
        assert!(page_id <= self.header.num_pages);
        let mut page_data = vec![0; self.header.page_size];
        let offset = PAGE_MULTIPLIER + (page_id as usize - 1) * self.header.page_size;
        let error_fn = |e| {
            WalError::IoError(format!(
                "Failed to read Segment {} Page {}: {}",
                self.header.segment_id, page_id, e
            ))
        };
        self.file
            .seek(SeekFrom::Start(offset as u64))
            .map_err(error_fn)?;
        self.file.read_exact(&mut page_data).map_err(error_fn)?;
        Page::try_from(&page_data[..])
    }

    /// Read all entries from a single page
    fn read_page_entries(&mut self, page_id: u32) -> Result<Vec<Vec<u8>>, WalError> {
        let page = self.read_page(page_id)?;
        Ok(page.read(self.header.length_prefix))
    }

    /// Add a new log to file segment
    pub fn append(&mut self, data: &[u8]) -> bool {
        // First page of the segment
        if self.header.num_pages == 0 {
            let page = self.new_page();
            self.pages.push_back(page);
        }

        // prefix data with length
        let mut entry = vec![0u8; data.len() + self.header.length_prefix];
        entry[0..self.header.length_prefix]
            .copy_from_slice(&data.len().to_le_bytes()[..self.header.length_prefix]);
        entry[self.header.length_prefix..].copy_from_slice(data);

        // add entry to the latest page and return if success
        let page = self.pages.back_mut().unwrap();
        if page.add(&entry) {
            self.is_dirty = true;
            return true;
        }

        // Failed to add to existing page
        // check if the segment is filled
        if self.len() + self.header.page_size > self.max_size {
            return false;
        }

        // write to a new page
        let mut page = self.new_page();
        page.add(&entry);
        self.pages.push_back(page);
        true
    }

    fn new_page(&mut self) -> Page {
        self.header.num_pages += 1;
        self.header.is_dirty = true;
        self.is_dirty = true;
        Page::new(self.header.num_pages, self.header.page_size)
    }

    /// Returns an iterator that yields each record from the segment file
    pub fn iter(&mut self) -> PageIterator {
        PageIterator::new(self)
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    /// Flush all in-memory changes to IO
    pub fn flush(&mut self) -> Result<(), WalError> {
        if self.is_dirty {
            self.sync_header()?;
            self.sync_pages()?;
            self.is_dirty = false;
        }
        Ok(())
    }

    /// Returns the size this segment will take on disk when flushed without adding any new information
    pub fn len(&self) -> usize {
        PAGE_MULTIPLIER + self.header.num_pages as usize * self.header.page_size
    }

    /// Flush header to IO
    fn sync_header(&mut self) -> Result<(), WalError> {
        // exit early if no changes
        if !self.header.is_dirty {
            return Ok(());
        }
        // error handling
        let error_fn = |e| {
            WalError::IoError(format!(
                "Failed to write header for segment {}: {}",
                self.header.segment_id, e
            ))
        };
        // update the file
        self.file.seek(SeekFrom::Start(0)).map_err(error_fn)?;
        self.file
            .write_all(&self.header.as_bytes())
            .map_err(error_fn)?;
        // update the header
        self.header.is_dirty = false;
        Ok(())
    }

    /// Flush dirty pages to IO
    fn sync_pages(&mut self) -> Result<(), WalError> {
        // sync all dirty pages
        for page in &mut self.pages {
            if !page.is_dirty {
                continue;
            }
            let offset = PAGE_MULTIPLIER + (page.id as usize - 1) * self.header.page_size;
            self.file
                .seek(SeekFrom::Start(offset as u64))
                .map_err(|e| WalError::IoError(e.to_string()))?;
            self.file
                .write_all(&page.as_bytes())
                .map_err(|e| WalError::IoError(e.to_string()))?;
            page.is_dirty = false;
        }
        // remove all but latest page from memory
        while self.pages.len() > 1 {
            self.pages.pop_front();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::clean_test_dir;
    use crate::TESTING_DIR;

    #[test]
    fn new() {
        clean_test_dir();
        FileSegment::create_new(TESTING_DIR, 1, PAGE_MULTIPLIER, PAGE_MULTIPLIER * 100).unwrap();
    }

    // test file_path logic
    #[test]
    fn file_path() {
        let path = FileSegment::get_path(TESTING_DIR, 1);
        assert_eq!(
            path.to_str().unwrap(),
            "./tmp/testing/logs/wal_0000000001.bin"
        );
    }

    // test to create a file segment with no data and read it back
    #[test]
    fn open_empty() {
        clean_test_dir();
        let mut segment =
            FileSegment::create_new(TESTING_DIR, 1, PAGE_MULTIPLIER, PAGE_MULTIPLIER * 100)
                .unwrap();
        segment.flush().unwrap();
        drop(segment);
        let path = FileSegment::get_path(TESTING_DIR, 1);
        let segment =
            FileSegment::open_existing(path.to_str().unwrap(), PAGE_MULTIPLIER * 100).unwrap();
    }

    // test to create a file segment with some data and read it back
    #[test]
    fn open_filled() {
        // fixtures
        clean_test_dir();
        let mut segment =
            FileSegment::create_new(TESTING_DIR, 1, PAGE_MULTIPLIER, PAGE_MULTIPLIER * 100)
                .unwrap();
        assert!(segment.append(b"John Doe"));
        assert!(segment.append(b"Jane Doe"));
        segment.flush().unwrap();
        drop(segment);
        let path = FileSegment::get_path(TESTING_DIR, 1);
        // read data
        let mut segment =
            FileSegment::open_existing(path.to_str().unwrap(), PAGE_MULTIPLIER * 100).unwrap();
        let data = segment.read_page_entries(1).unwrap();
        assert_eq!(data.len(), 2);
        assert_eq!(data[0], b"John Doe");
        assert_eq!(data[1], b"Jane Doe");
    }

    #[test]
    fn iter() {
        // fixtures
        clean_test_dir();
        let mut segment =
            FileSegment::create_new(TESTING_DIR, 1, PAGE_MULTIPLIER, PAGE_MULTIPLIER * 100)
                .unwrap();
        assert!(segment.append(b"John Doe"));
        assert!(segment.append(b"Jane Doe"));
        segment.flush().unwrap();
        drop(segment);
        let path = FileSegment::get_path(TESTING_DIR, 1);
        // read data
        let mut segment =
            FileSegment::open_existing(path.to_str().unwrap(), PAGE_MULTIPLIER * 100).unwrap();
        let items = segment.iter().collect::<Vec<_>>();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0], b"John Doe");
        assert_eq!(items[1], b"Jane Doe");
    }

    #[test]
    fn multi_page_iter() {
        // fixtures
        clean_test_dir();
        let mut segment =
            FileSegment::create_new(TESTING_DIR, 1, PAGE_MULTIPLIER, PAGE_MULTIPLIER * 100)
                .unwrap();
        for i in 0..=1_000 {
            assert!(segment.append(format!("Record number {}", i).as_bytes()));
        }
        segment.flush().unwrap();
        drop(segment);
        let path = FileSegment::get_path(TESTING_DIR, 1);
        // read data
        let mut segment =
            FileSegment::open_existing(path.to_str().unwrap(), PAGE_MULTIPLIER * 100).unwrap();
        let items = segment.iter().collect::<Vec<_>>();
        assert_eq!(items.len(), 1_001);
        assert_eq!(&items[0], &b"Record number 0");
        assert_eq!(&items[100], &b"Record number 100");
        assert_eq!(&items[555], &b"Record number 555");
        assert_eq!(&items[1000], &b"Record number 1000");
    }
}
