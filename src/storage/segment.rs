use crate::storage::header::{Header, HEADER_SIZE};
use crate::storage::page::Page;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

/// Deals with a single file that comprises the header plus pages.
///
/// This is where you’ll have functionality to:
/// - Read and write the file header.
/// - Append data to the file in page-sized increments.
/// - Handle synchronization/flush if needed.
struct FileSegment {
    header: Header,
    pages: Vec<Page>,
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
            pages: vec![],
            file,
            is_dirty: true,
        })
    }

    /// Get the path of the segment file
    fn get_path(base_dir: &str, segment_id: usize) -> PathBuf {
        let mut path = PathBuf::from(base_dir);
        let width = u32::MAX.to_string().len();
        let file = format!("log/wal_{:0width$}.log", segment_id, width = width);
        path.push(file);
        path
    }

    /// Opens an existing file and load it's latest page in memory
    pub fn open_existing(path: &str) -> Result<Self, String> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| e.to_string())?;

        let mut header_data = [0; 4096];
        file.read_exact(&mut header_data)
            .map_err(|e| e.to_string())?;
        let header = Header::try_from(&header_data[..]).map_err(|e| e.to_string())?;

        let mut segment = Self {
            header,
            pages: Vec::new(),
            file,
            is_dirty: false,
        };

        // Read the latest page into memory
        if segment.header.num_pages > 0 {
            let page = segment.read_page(segment.header.num_pages)?;
            segment.pages.push(page);
        }

        Ok(segment)
    }

    /// Read a specific page from the file
    fn read_page(&mut self, page_id: u32) -> Result<Page, String> {
        assert!(page_id <= self.header.num_pages);
        let mut page_data = vec![0; self.header.page_size];
        let offset = HEADER_SIZE + (page_id as usize - 1) * self.header.page_size;
        self.file
            .seek(SeekFrom::Start(offset as u64))
            .map_err(|e| e.to_string())?;
        self.file
            .read_exact(&mut page_data)
            .map_err(|e| e.to_string())?;
        Page::try_from(&page_data[..])
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        println!("Flush called {} {}", self.is_dirty, self.pages.len());
        if self.is_dirty {
            self.sync_header();
            self.sync_pages();
        }
        Ok(())
    }

    fn sync_header(&mut self) {
        if !self.header.is_dirty {
            return;
        }
        self.file.seek(SeekFrom::Start(0)).unwrap();
        self.file.write_all(&self.header.as_bytes()).unwrap();
        self.header.is_dirty = false;
    }

    fn sync_pages(&mut self) {
        for page in &mut self.pages {
            if !page.is_dirty {
                continue;
            }
            let offset = HEADER_SIZE + (page.id as usize - 1) * self.header.page_size;
            self.file.seek(SeekFrom::Start(offset as u64)).unwrap();
            self.file.write_all(&page.as_bytes()).unwrap();
            page.is_dirty = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TESTING_DIR;

    fn create_test_dir() {
        std::fs::create_dir_all(TESTING_DIR).unwrap();
        let mut path = PathBuf::from(TESTING_DIR);
        path.push("log");
        std::fs::create_dir_all(path).unwrap();
    }

    #[test]
    fn new() {
        create_test_dir();
        let segment = FileSegment::create_new(TESTING_DIR, 1, 4096).unwrap();
    }

    #[test]
    fn open() {
        let mut segment = FileSegment::create_new(TESTING_DIR, 1, 4096).unwrap();
        segment.flush().unwrap();
        drop(segment);
        let path = FileSegment::get_path(TESTING_DIR, 1);
        let segment = FileSegment::open_existing(path.to_str().unwrap()).unwrap();
    }

    #[test]
    fn file_path() {
        let path = FileSegment::get_path(TESTING_DIR, 1);
        assert_eq!(
            path.to_str().unwrap(),
            "./tmp/testing/log/wal_0000000001.log"
        );
    }
}
