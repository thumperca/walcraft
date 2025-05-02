mod factory;
mod meta;
mod segment;

use self::factory::StorageFactory;
use self::meta::Meta;
use self::segment::FileSegment;
use crate::error::WalError;
use crate::WalConfig2;
use std::collections::VecDeque;
use std::path::PathBuf;

/// Storage manager for the WAL
///
/// This module is responsible for the actual IO operations, including
/// - Reading data from IO
/// - Writing data to IO
/// - Garbage Collection
///
struct Storage {
    config: WalConfig2,
    meta: Meta,
    segments: VecDeque<FileSegment>,
}

impl Storage {
    pub fn new(config: WalConfig2) -> Result<Self, WalError> {
        let storage = StorageFactory::new(config)?;
        Ok(storage)
    }

    fn meta_path(base_dir: &PathBuf) -> PathBuf {
        let mut path = base_dir.clone();
        path.push("meta.json");
        path
    }

    /// Read the metadata file from the disk
    fn read_meta(path: &PathBuf) -> Result<Meta, WalError> {
        let path = Self::meta_path(path);
        // create a default object for the first run
        if !path.exists() {
            return Ok(Meta::new(&path));
        }
        // read from the file
        Meta::read_from_file(path)
    }

    /// Initialize the storage layer
    ///
    /// This process performs 3 tasks:
    /// - Ensures the size of the last segment is accurate
    /// - Runs garbage collection
    /// - Load a segment file in memory for future writes
    ///
    fn init(&mut self) -> Result<(), WalError> {
        // self.sync_meta()?;
        self.gc()?;
        self.load_segment()?;
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
            let segment = match self.meta.segments.front() {
                Some(segment) => segment,
                None => break,
            };
            let path = FileSegment::get_path(&self.config.location, segment.file_id);
            std::fs::remove_file(path).map_err(|_| WalError::GcFailure)?;
            size_used -= segment.file_size;
            if size_used <= self.config.size {
                break;
            }
        }
        // update meta file
        self.meta.sync().map_err(|_| WalError::MetaFileError)?;
        Ok(())
    }

    /// Load a segment file into memory for writing
    ///
    /// This is done by reading the last segment file if space is left in the last file.
    /// It creates a new file segment if the last file is full, no file exists, or
    /// the page size is different from the last file.
    ///
    fn load_segment(&mut self) -> Result<(), WalError> {
        match self.meta.segments.back() {
            Some(segment) => {
                // check if the last segment is full
                if segment.file_size >= self.config.size {
                    return Ok(());
                }
                // check if the page size is different
                if segment.page_size != self.config.page_size {
                    return Ok(());
                }
            }
            None => {}
        }
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TESTING_DIR;

    #[test]
    fn new_instance() {
        let config = WalConfig2 {
            location: PathBuf::from(TESTING_DIR),
            size: usize::MAX,
            fsync: false,
            page_size: 4096,
            sync_interval: 0,
        };
        let storage = Storage::new(config);
        assert!(storage.is_ok());
    }

    #[test]
    fn write_and_read() {}

    #[test]
    fn multiple_files() {}

    #[test]
    fn garbage_collection() {}
}
