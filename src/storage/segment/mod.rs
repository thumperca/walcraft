mod header;
mod iterator;
mod page;

use self::header::{Header, HEADER_SIZE};
use self::iterator::PageIterator;
use self::page::Page;
use crate::error::WalError;
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

/// Deals with a single file that comprises the header plus pages.
///
/// This is where you’ll have functionality to:
/// - Read and write the file header.
/// - Append data to the file in page-sized increments.
/// - Handle synchronization/flush if needed.
pub(crate) struct FileSegment {
    header: Header,
    pages: VecDeque<Page>,
    file: File,
    is_dirty: bool,
}

impl FileSegment {
    /// Creates a new file segment
    ///
    /// This function create a new empty file on disk as well
    pub fn create_new(base_dir: &str, segment_id: usize, page_size: usize) -> Result<Self, String> {
        assert_eq!(page_size % 4096, 0);

        let path = Self::get_path(base_dir, segment_id);

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(path)
            .map_err(|e| e.to_string())?;

        let header = Header::new(segment_id, page_size);
        Ok(Self {
            header,
            pages: VecDeque::new(),
            file,
            is_dirty: true,
        })
    }

    /// Get the path of the segment file
    fn get_path(base_dir: &str, segment_id: usize) -> PathBuf {
        let mut path = PathBuf::from(base_dir);
        let width = u32::MAX.to_string().len();
        let file = format!("logs/wal_{:0width$}.log", segment_id, width = width);
        path.push(file);
        path
    }

    /// Opens an existing file and load it's latest page in memory
    pub fn open_existing(path: &str) -> Result<Self, WalError> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|_| WalError::OpenFailure)?;

        let mut header_data = [0; 4096];
        file.read_exact(&mut header_data)
            .map_err(|e| e.to_string())
            .map_err(|_| WalError::ReadFailure)?;
        let header = Header::try_from(&header_data[..])?;

        let mut segment = Self {
            header,
            pages: VecDeque::new(),
            file,
            is_dirty: false,
        };

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
        let offset = HEADER_SIZE + (page_id as usize - 1) * self.header.page_size;
        self.file
            .seek(SeekFrom::Start(offset as u64))
            .map_err(|e| e.to_string())
            .map_err(|_| WalError::SeekFailure)?;
        self.file
            .read_exact(&mut page_data)
            .map_err(|e| e.to_string())
            .map_err(|_| WalError::ReadFailure)?;
        Page::try_from(&page_data[..])
    }

    /// Read all entries from a single page
    fn read_page_entries(&mut self, page_id: u32) -> Result<Vec<Vec<u8>>, WalError> {
        let page = self.read_page(page_id)?;
        Ok(page.read(self.header.length_prefix))
    }

    /// Add a new log to file segment
    pub fn append(&mut self, data: &[u8]) -> Result<(), WalError> {
        // First page of segment
        if self.header.num_pages == 0 {
            let page = self.new_page();
            self.pages.push_back(page);
        }

        // prefix data with length
        let mut entry = vec![0u8; data.len() + self.header.length_prefix];
        entry[0..self.header.length_prefix]
            .copy_from_slice(&data.len().to_le_bytes()[..self.header.length_prefix]);
        entry[self.header.length_prefix..].copy_from_slice(data);

        // add entry to latest page and return if success
        let page = self.pages.back_mut().unwrap();
        if page.add(&entry) {
            self.is_dirty = true;
            return Ok(());
        }

        // Failed to add to existing page
        // check if segment is filled
        if self.header.num_pages == u32::MAX {
            return Err(WalError::SegmentFull);
        }

        // write to a new page
        let mut page = self.new_page();
        page.add(&entry);
        self.pages.push_back(page);

        Ok(())
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

    /// Flush all in-memory changes to IO
    pub fn flush(&mut self) -> Result<(), WalError> {
        if self.is_dirty {
            self.sync_header()?;
            self.sync_pages()?;
        }
        Ok(())
    }

    /// Flush header to IO
    fn sync_header(&mut self) -> Result<(), WalError> {
        if !self.header.is_dirty {
            return Ok(());
        }
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|_| WalError::SeekFailure)?;
        self.file
            .write_all(&self.header.as_bytes())
            .map_err(|_| WalError::WriteFailure)?;
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
            let offset = HEADER_SIZE + (page.id as usize - 1) * self.header.page_size;
            self.file
                .seek(SeekFrom::Start(offset as u64))
                .map_err(|_| WalError::SeekFailure)?;
            self.file
                .write_all(&page.as_bytes())
                .map_err(|_| WalError::WriteFailure)?;
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
    use crate::TESTING_DIR;

    // utility function to re-create test dir for each test
    pub fn create_test_dir() {
        std::fs::remove_dir_all(TESTING_DIR).unwrap();
        std::fs::create_dir_all(TESTING_DIR).unwrap();
        let mut path = PathBuf::from(TESTING_DIR);
        path.push("logs");
        std::fs::create_dir_all(path).unwrap();
    }

    #[test]
    fn new() {
        create_test_dir();
        let segment = FileSegment::create_new(TESTING_DIR, 1, 4096).unwrap();
    }

    // test file_path logic
    #[test]
    fn file_path() {
        let path = FileSegment::get_path(TESTING_DIR, 1);
        assert_eq!(
            path.to_str().unwrap(),
            "./tmp/testing/logs/wal_0000000001.log"
        );
    }

    // test to create a file segment with no data and read it back
    #[test]
    fn open_empty() {
        create_test_dir();
        let mut segment = FileSegment::create_new(TESTING_DIR, 1, 4096).unwrap();
        segment.flush().unwrap();
        drop(segment);
        let path = FileSegment::get_path(TESTING_DIR, 1);
        let segment = FileSegment::open_existing(path.to_str().unwrap()).unwrap();
    }

    // test to create a file segment with some data and read it back
    #[test]
    fn open_filled() {
        // fixtures
        create_test_dir();
        let mut segment = FileSegment::create_new(TESTING_DIR, 1, 4096).unwrap();
        segment.append(b"John Doe").expect("Failed to append data");
        segment.append(b"Jane Doe").expect("Failed to append data");
        segment.flush().unwrap();
        drop(segment);
        let path = FileSegment::get_path(TESTING_DIR, 1);
        // read data
        let mut segment = FileSegment::open_existing(path.to_str().unwrap()).unwrap();
        let data = segment.read_page_entries(1).unwrap();
        assert_eq!(data.len(), 2);
        assert_eq!(data[0], b"John Doe");
        assert_eq!(data[1], b"Jane Doe");
    }

    #[test]
    fn iter() {
        // fixtures
        create_test_dir();
        let mut segment = FileSegment::create_new(TESTING_DIR, 1, 4096).unwrap();
        segment.append(b"John Doe").expect("Failed to append data");
        segment.append(b"Jane Doe").expect("Failed to append data");
        segment.flush().unwrap();
        drop(segment);
        let path = FileSegment::get_path(TESTING_DIR, 1);
        // read data
        let mut segment = FileSegment::open_existing(path.to_str().unwrap()).unwrap();
        let items = segment.iter().collect::<Vec<_>>();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0], b"John Doe");
        assert_eq!(items[1], b"Jane Doe");
    }

    #[test]
    fn multi_page_iter() {
        // fixtures
        create_test_dir();
        let mut segment = FileSegment::create_new(TESTING_DIR, 1, 4 * 1024).unwrap();
        for i in 0..=1_000 {
            segment
                .append(format!("Record number {}", i).as_bytes())
                .expect("Failed to append data");
        }
        segment.flush().unwrap();
        drop(segment);
        let path = FileSegment::get_path(TESTING_DIR, 1);
        // read data
        let mut segment = FileSegment::open_existing(path.to_str().unwrap()).unwrap();
        let items = segment.iter().collect::<Vec<_>>();
        assert_eq!(items.len(), 1_001);
        assert_eq!(&items[0], &b"Record number 0");
        assert_eq!(&items[100], &b"Record number 100");
        assert_eq!(&items[555], &b"Record number 555");
        assert_eq!(&items[1000], &b"Record number 1000");
    }
}
