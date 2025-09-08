mod factory;
pub(crate) mod iterator;
pub(crate) mod meta;
mod segment;

use self::factory::StorageFactory;
use self::meta::Meta;
use self::segment::FileSegment;
use crate::error::WalError;
use crate::storage::meta::SizeEntry;
use crate::{WalConfig, PAGE_MULTIPLIER};
use std::collections::VecDeque;
use std::io::ErrorKind;

/// Storage manager (or storage engine) for the WAL
///
/// This module is responsible for the actual IO operations, including
/// - Reading data from IO
/// - Writing data to IO
/// - Garbage Collection
///
pub(crate) struct Storage {
    config: WalConfig,
    pub(crate) meta: Meta,
    segments: VecDeque<FileSegment>,
}

impl Storage {
    /// Initialize a new storage manager
    pub fn new(config: WalConfig) -> Result<Self, WalError> {
        let storage = StorageFactory::new(config)?;
        Ok(storage)
    }

    /// Append data to the WAL
    pub fn append(&mut self, data: &[u8]) -> Result<(), WalError> {
        // ensure a segment is loaded into memory
        if self.segments.is_empty() {
            self.load_segment()?;
        }
        // write to the segment
        let segment = self.segments.back_mut().unwrap();
        if segment.append(data) {
            return Ok(());
        }
        // failed to write data as the segment is full.
        // load a new segment into memory
        self.next_segment()?;
        // write to the new segment
        let segment = self.segments.back_mut().unwrap();
        if !segment.append(data) {
            unreachable!("Failed to write data to a new segment");
        }
        Ok(())
    }

    /// Flush all changes to disk
    pub fn flush(&mut self, fsync: bool) -> Result<(), WalError> {
        // sync all active segments
        for segment in &mut self.segments {
            if segment.is_dirty() {
                segment.flush(fsync)?;
            }
        }
        // free up memory and update metadata
        while self.segments.len() > 1 {
            let segment = self.segments.pop_front().unwrap();
            self.meta.update(&segment);
        }
        let segment = self.segments.front().unwrap();
        self.meta.update(segment);
        // run garbage collection on disk
        self.meta.sync()?;
        self.gc()
    }

    /// Load a segment file into memory for writing
    ///
    /// This is done by reading the last segment file if space is left in the last file.
    /// It creates a new file segment if the last file is full, no file exists, or
    /// the page size is different from the last file.
    ///
    fn load_segment(&mut self) -> Result<(), WalError> {
        // first init
        if !self.meta.init {
            return self.next_segment();
        }
        // read the current segment
        let current_file = self.meta.current_pointer;
        let path = FileSegment::get_path(&self.config.location, current_file);
        let segment = FileSegment::open_existing(path, self.config.max_file_size())?;
        // check segment's page_size and total file size
        let page_full = segment.len() >= self.config.max_file_size();
        let page_size_mismatch = segment.header.page_size != self.config.page_size;
        // open a new segment if the page_size is different or the segment is full
        if page_full || page_size_mismatch {
            return self.next_segment();
        }
        self.segments.push_back(segment);
        Ok(())
    }

    /// Load a new segment in memory for writing
    fn next_segment(&mut self) -> Result<(), WalError> {
        let mut new_id = self.meta.current_pointer.wrapping_add(1);
        if new_id == 0 {
            // wrap around
            new_id = 1;
        }
        if !self.meta.init {
            new_id = 1;
        }
        // create a new segment
        let segment = FileSegment::create_new(
            self.config.location.clone(),
            new_id,
            self.config.page_size,
            self.config.max_file_size(),
        )?;
        // update metadata
        self.meta.current_pointer = new_id;
        self.meta.segments.push_back(SizeEntry {
            file_id: segment.header.segment_id,
            page_size: segment.header.page_size,
            file_size: PAGE_MULTIPLIER,
        });
        self.meta.init = true;
        self.meta.dirty = true;
        self.segments.push_back(segment);
        Ok(())
    }

    /// Garbage collection of older file segments to maintain the max size
    fn gc(&mut self) -> Result<(), WalError> {
        let mut size_used = 0;
        for segment in &self.meta.segments {
            size_used += segment.file_size;
        }
        // WAL size is withing limits
        if size_used <= self.config.size {
            return Ok(());
        }
        // WAL size is over the limit; remove the oldest segments
        loop {
            let segment = match self.meta.segments.pop_front() {
                Some(segment) => segment,
                None => break,
            };
            self.meta.dirty = true;
            let path = FileSegment::get_path(&self.config.location, segment.file_id);
            if let Err(e) = std::fs::remove_file(path) {
                match e.kind() {
                    ErrorKind::NotFound => {}
                    _ => {
                        return Err(WalError::IoError(format!(
                            "Failed to delete old log file {}: {} {}",
                            segment.file_id,
                            e,
                            e.kind()
                        )));
                    }
                }
            };
            size_used -= segment.file_size;
            if size_used <= self.config.size {
                break;
            }
        }
        // update meta file
        self.meta.sync()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::iterator::StorageIterator;
    use crate::tests::clean_test_dir;
    use crate::TESTING_DIR;
    use std::path::PathBuf;

    #[test]
    fn new_instance() {
        clean_test_dir();
        let config = WalConfig {
            location: PathBuf::from(TESTING_DIR),
            size: usize::MAX,
            fsync: false,
            page_size: 4096,
            sync_interval: 0,
        };
        Storage::new(config).unwrap();
    }

    #[test]
    fn write() {
        clean_test_dir();
        let config = WalConfig {
            location: PathBuf::from(TESTING_DIR),
            size: usize::MAX,
            fsync: false,
            page_size: 4096,
            sync_interval: 0,
        };
        // write data
        let mut storage = Storage::new(config.clone()).unwrap();
        storage.append(b"Hello, world!").unwrap();
        storage.append(b"Hello, Rust!").unwrap();
        storage.append(b"Hello, WAL!").unwrap();
        storage.flush(false).unwrap();
        drop(storage);
        // ensure data is there
        let iterator = StorageIterator::new(Meta::read_from_file(TESTING_DIR).unwrap());
        let data = iterator.collect::<Vec<_>>();
        assert_eq!(data.len(), 3);
        assert_eq!(data[1], b"Hello, Rust!");
    }

    #[test]
    fn multiple_files() {
        clean_test_dir();
        let config = WalConfig {
            location: PathBuf::from(TESTING_DIR),
            size: 4096 * 10, // 40 KB
            fsync: false,
            page_size: 4096,
            sync_interval: 100,
        };
        let mut storage = Storage::new(config).unwrap();
        // write a lot of data
        for i in 0..1000 {
            let data = format!("Hello, world! {}", i); // 18 bytes
            storage.append(data.as_bytes()).unwrap();
        }
        storage.flush(false).unwrap();
    }

    #[test]
    fn garbage_collection() {
        clean_test_dir();
        let config = WalConfig {
            location: PathBuf::from(TESTING_DIR),
            size: 4096 * 10, // 40 KB
            fsync: false,
            page_size: 4096,
            sync_interval: 100,
        };
        let mut storage = Storage::new(config).unwrap();
        // write a lot of data
        for i in 0..1000 {
            let data = format!("Hello, world! {}", i); // 18 bytes
            storage.append(data.as_bytes()).unwrap();
        }
        storage.flush(false).unwrap();
    }

    fn setup_data(config: &WalConfig, pointer: u32) {
        // create a metadata file
        let mut meta = Meta {
            dirty: true,
            init: true,
            location: Meta::path(TESTING_DIR),
            current_pointer: pointer,
            segments: VecDeque::from([SizeEntry {
                file_id: pointer,
                page_size: 4096,
                file_size: 4096,
            }]),
        };
        meta.sync().expect("Failed to sync meta to disk");
        // create a segment file to prevent file access error
        let mut segment = FileSegment::create_new(
            PathBuf::from(TESTING_DIR),
            u32::MAX - 2,
            config.page_size,
            config.max_file_size(),
        )
        .expect("Failed to create new segment");
        segment.is_dirty = true;
        segment.flush(false).expect("Failed to flush segment");
    }

    #[test]
    fn wrapping_garbage_collection() {
        clean_test_dir();
        // fixtures
        let current_pointer = u32::MAX - 2;
        let config = WalConfig {
            location: PathBuf::from(TESTING_DIR),
            size: 4096 * 10, // 40 KB
            fsync: false,
            page_size: 4096,
            sync_interval: 100,
        };
        setup_data(&config, current_pointer);
        // create a new storage instance and write data
        let mut storage = Storage::new(config).expect("Failed to create storage");
        for i in 0..2000 {
            let data = format!("Hello, world! {}", i); // 18 bytes
            storage.append(data.as_bytes()).unwrap();
        }
        storage.flush(false).unwrap();
        // test for wrapping add
        let meta = storage.meta;
        assert_eq!(meta.current_pointer, 1);
        assert_eq!(meta.segments.len(), 3);
        assert_eq!(meta.segments[0].file_id, u32::MAX - 1);
        assert_eq!(meta.segments[1].file_id, u32::MAX);
        assert_eq!(meta.segments[2].file_id, 1);
    }
}
