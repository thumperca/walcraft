use super::Storage;
use crate::error::WalError;
use crate::storage::meta::{Meta, SizeEntry};
use crate::storage::segment::FileSegment;
use crate::{WalConfig2, PAGE_MULTIPLIER};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

pub(crate) struct StorageFactory {
    config: WalConfig2,
    meta: Meta,
}

impl StorageFactory {
    pub fn new(config: WalConfig2) -> Result<Storage, WalError> {
        std::fs::create_dir_all(&config.location)
            .map_err(|_| WalError::IoError("Failed to create WAL directory".to_string()))?;
        let meta = Self::read_meta(&config.location)?;
        let mut factory = Self { meta, config };
        let segment = factory.init()?;
        let mut storage = Storage {
            config: factory.config,
            meta: factory.meta,
            segments: VecDeque::from([segment]),
        };
        storage.gc()?;
        Ok(storage)
    }

    /// Read the metadata file from the disk
    fn read_meta(path: &PathBuf) -> Result<Meta, WalError> {
        let path = Meta::path(path);
        // create a default object for the first run
        if !path.exists() {
            return Ok(Meta::new(path));
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
    fn init(&mut self) -> Result<FileSegment, WalError> {
        self.sync_with_disk()?;
        let segment = self.load_segment()?;
        Ok(segment)
    }

    fn sync_with_disk(&mut self) -> Result<(), WalError> {
        let mut location = self.config.location.clone();
        location.push("logs");
        // create dir if not exists
        self.ensure_location(&location)?;
        // sync the metadata with the logs directory
        let sizes = self.read_contents(&location)?;
        self.update_meta(sizes);
        // write the metadata to IO
        if self.meta.dirty {
            self.meta.sync()?;
        }
        Ok(())
    }

    /// Create the log directory if it doesn't exist
    fn ensure_location(&self, path: &PathBuf) -> Result<(), WalError> {
        // create logs directory if it doesn't exist
        let exists = std::fs::exists(&path)
            .map_err(|e| WalError::IoError(format!("Failed to read WAL logs directory: {}", e)))?;
        if !exists {
            std::fs::create_dir_all(&path).map_err(|e| {
                WalError::IoError(format!("Failed to create WAL logs directory: {}", e))
            })?;
        }
        Ok(())
    }

    /// Read the contents of the WAL logs directory
    ///
    /// # Argument
    /// - location: The path to the WAL logs directory
    ///
    /// # Returns
    /// - A HashMap where the key is the file ID and the value is the file size
    ///
    fn read_contents(&self, location: &PathBuf) -> Result<HashMap<u32, u64>, WalError> {
        // read the contents of the directory
        let dir_content = std::fs::read_dir(location)
            .map_err(|e| WalError::IoError(format!("Failed to read WAL logs directory: {}", e)))?;

        let mut sizes = HashMap::new();

        for entry in dir_content {
            let entry = entry.map_err(|e| {
                WalError::IoError(format!("Failed to read WAL logs directory: {}", e))
            })?;
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            if !file_name.starts_with("wal_") {
                continue;
            }
            let meta = entry.metadata().map_err(|e| {
                WalError::IoError(format!("Failed to read WAL logs directory: {}", e))
            })?;
            if meta.is_file() {
                let f = file_name.replace("wal_", "");
                let f = f.replace(".log", "");
                let file_id = match f.parse::<u32>() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                sizes.insert(file_id, meta.len());
            }
        }
        Ok(sizes)
    }

    /// Update the metadata with the information read from IO
    ///
    /// # Argument
    /// - A HashMap where the key is the file ID and the value is the file size
    ///
    fn update_meta(&mut self, sizes: HashMap<u32, u64>) {
        // remove segments for whom the file is missing
        let d = self
            .meta
            .segments
            .iter()
            .filter_map(|entry| {
                let actual_size = *sizes.get(&entry.file_id)? as usize;
                if actual_size == 0 {
                    return None;
                }
                let mut entry = entry.clone();
                if actual_size != entry.file_size {
                    entry.file_size = actual_size;
                }
                Some(entry)
            })
            .collect::<VecDeque<_>>();
        let is_same = Self::compare_segments(&self.meta.segments, &d);
        if !is_same {
            self.meta.segments = d;
            self.meta.dirty = true;
        }
        // todo: add detected segments that are missing in the metadata file
        // write code here for the above

        // detect corruption from deletion of all log files and reset
        if self.meta.segments.is_empty() {
            if self.meta.current_pointer != 1 {
                self.meta.current_pointer = 1;
                self.meta.dirty = true;
            }
            return;
        }
        // detect corruption from selective deletion and revert to the last file
        let last = self.meta.segments.back().unwrap();
        if last.file_id != self.meta.current_pointer {
            self.meta.current_pointer = last.file_id;
            self.meta.dirty = true;
        }
    }

    /// Compares two segments to check if they are the same
    fn compare_segments(left: &VecDeque<SizeEntry>, right: &VecDeque<SizeEntry>) -> bool {
        if left.len() != right.len() {
            return false;
        }
        for (l, r) in left.iter().zip(right.iter()) {
            if l.file_id != r.file_id || l.file_size != r.file_size {
                return false;
            }
        }
        true
    }

    /// Load a segment file into memory for writing
    ///
    /// This is done by reading the last segment file if space is left in the last file.
    /// It creates a new file segment if the last file is full, no file exists, or
    /// the page size is different from the last file.
    ///
    fn load_segment(&mut self) -> Result<FileSegment, WalError> {
        let current_file = self.meta.current_pointer;
        let path = FileSegment::get_path(&self.config.location, current_file);
        let mut segment = FileSegment::open_existing(path)?;
        // open a new segment if the page_size is different
        if segment.header.page_size != self.config.page_size {
            // create a new segment
            segment = FileSegment::create_new(
                self.config.location.clone(),
                segment.header.segment_id.wrapping_add(1),
                self.config.page_size,
            )?;
            // add the new segment to the list
            self.meta.segments.push_back(SizeEntry {
                file_id: segment.header.segment_id,
                page_size: segment.header.page_size,
                file_size: PAGE_MULTIPLIER,
            });
            self.meta.dirty = true;
            self.meta.sync()?;
        }
        Ok(segment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new() {}

    #[test]
    fn changed_page_size() {}

    #[test]
    fn missing_file() {}

    #[test]
    fn missing_current_file() {}
}
