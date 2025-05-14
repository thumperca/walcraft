use crate::error::WalError;
use crate::{Size, Wal, WalConfig, PAGE_MULTIPLIER};
use std::path::{Path, PathBuf};

/// Build [Wal] with custom configuration
///
/// It uses a builder pattern and methods can be chained.
///
/// By default, [Wal] uses a buffer of 4 KB, unlimited storage size and fsync is disabled.
///
/// ### Example
/// ```no_run
/// use walcraft::{Size, WalBuilder, Wal};
/// // create a wal with 4 KB buffer and 10 GB storage
/// let wal: Wal = WalBuilder::new().page_size(Size::Kb(4)).storage_size(Size::Gb(10)).build().unwrap();
/// // create a wal with no buffer, enable fsync and use 250 MB of storage
/// let wal: Wal = WalBuilder::new().storage_size(Size::Mb(250)).enable_fsync().build().unwrap();
/// ```
pub struct WalBuilder {
    location: Option<PathBuf>,
    page_size: Option<Size>,
    storage_size: Option<Size>,
    sync_interval: Option<usize>,
    fsync: bool,
}

impl WalBuilder {
    /// Initiate a default instance of [WalBuilder]
    pub fn new() -> Self {
        Self {
            location: None,
            page_size: None,
            storage_size: None,
            sync_interval: None,
            fsync: false,
        }
    }

    /// Set log storage location
    /// Note: Ensure that no other files are present in this directory
    pub fn location<P: AsRef<Path>>(mut self, loc: P) -> Self {
        self.location = Some(loc.as_ref().to_path_buf());
        self
    }

    /// Enable fsync to commit all data from the kernel filesystem buffers to storage
    pub fn enable_fsync(mut self) -> Self {
        self.fsync = true;
        self
    }

    /// Set a custom page size
    pub fn page_size(mut self, size: Size) -> Self {
        self.page_size = Some(size);
        self
    }

    /// Set a storage size limit
    pub fn storage_size(mut self, size: Size) -> Self {
        self.storage_size = Some(size);
        self
    }

    /// Configure WAL sync interval with IO in milliseconds
    pub fn sync_interval(mut self, interval: usize) -> Self {
        self.sync_interval = Some(interval);
        self
    }

    pub fn build(self) -> Result<Wal, WalError> {
        // validate location
        let location = match self.location {
            None => {
                let error = WalError::ConfigError("Location field is required".to_string());
                return Err(error);
            }
            Some(loc) => loc,
        };
        if let Err(e) = std::fs::create_dir_all(location.as_path()) {
            let msg = format!("Failed to access WAL location: {}", e.to_string());
            return Err(WalError::ConfigError(msg));
        }
        // validate page size
        let page_size = self
            .page_size
            .map(|size| size.to_bytes())
            .unwrap_or(PAGE_MULTIPLIER);
        if page_size % PAGE_MULTIPLIER != 0 {
            let msg =
                "Page size must be a multiple of 4 KB for optimal reads and writes".to_string();
            return Err(WalError::ConfigError(msg));
        }
        // validate storage size
        let storage_size = self
            .storage_size
            .map(|size| size.to_bytes())
            .unwrap_or(usize::MAX);
        if storage_size < 1024 * 1024 * 4 {
            let msg = "Storage size must be at least 4 MB".to_string();
            return Err(WalError::ConfigError(msg));
        }
        if storage_size < page_size {
            let msg = "Storage size must be larger than page size".to_string();
            return Err(WalError::ConfigError(msg));
        }
        // validate sync interval
        // allowed range is 50 ms to 10 seconds
        let sync_interval = self.sync_interval.unwrap_or(250);
        if sync_interval < 50 || sync_interval > 10_000 {
            let msg = "Sync interval shall be between 10 milliseconds to 10 seconds".to_string();
            return Err(WalError::ConfigError(msg));
        }

        // create Wal
        let config = WalConfig {
            location,
            size: storage_size,
            fsync: self.fsync,
            page_size,
            sync_interval,
        };
        Wal::with_config(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TESTING_DIR;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct Log {
        id: usize,
        value: f32,
    }

    #[test]
    fn it_works() {
        let wal = WalBuilder::new().location(TESTING_DIR).build();
        assert!(wal.is_ok());
    }

    #[test]
    fn read_after_write() {
        let location = "./tmp/testing";
        std::fs::remove_dir_all(location).ok();

        // write some data
        let wal = WalBuilder::new().location(location).build().unwrap();
        wal.append_struct(Log { id: 1, value: 3.14 }).unwrap();
        wal.append_struct(Log { id: 2, value: 6.14 }).unwrap();
        wal.append_struct(Log { id: 3, value: 9.14 }).unwrap();
        wal.flush().unwrap();
        drop(wal);

        // try reading data
        let wal = WalBuilder::new().location(location).build().unwrap();
        let data = wal.iter().unwrap().collect::<Vec<_>>();
        assert_eq!(data.len(), 3);
    }
}
