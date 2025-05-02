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
