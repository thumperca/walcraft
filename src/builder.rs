use crate::{Size, Wal, WalConfig};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
/// let wal: Wal<String> = WalBuilder::new().buffer_size(Size::Kb(4)).storage_size(Size::Gb(10)).build().unwrap();
/// // create a wal with no buffer, enable fsync and use 250 MB of storage
/// let wal: Wal<String> = WalBuilder::new().storage_size(Size::Mb(250)).disable_buffer().enable_fsync().build().unwrap();
/// ```
pub struct WalBuilder {
    location: Option<String>,
    buffer_enabled: bool,
    buffer_size: Option<Size>,
    storage_size: Option<Size>,
    fsync: bool,
}

impl WalBuilder {
    /// Initiate a default instance of [WalBuilder]
    pub fn new() -> Self {
        Self {
            location: None,
            buffer_enabled: true,
            buffer_size: Some(Size::Kb(4)),
            storage_size: None,
            fsync: false,
        }
    }

    /// Set log storage location
    /// Note: Ensure that no other files are present in this directory
    pub fn location(mut self, loc: &str) -> Self {
        self.location = Some(loc.to_string());
        self
    }

    /// Enable fsync to commit all data from the kernel filesystem buffers to storage
    pub fn enable_fsync(mut self) -> Self {
        self.fsync = true;
        self
    }

    /// Disable the use of in-memory buffer to write directly to the disk
    pub fn disable_buffer(mut self) -> Self {
        self.buffer_enabled = false;
        self
    }

    /// Set a custom buffer size
    pub fn buffer_size(mut self, size: Size) -> Self {
        self.buffer_size = Some(size);
        self
    }

    /// Set a storage size limit
    pub fn storage_size(mut self, size: Size) -> Self {
        self.storage_size = Some(size);
        self
    }

    pub fn build<T>(self) -> Result<Wal<T>, String>
    where
        T: Serialize + for<'a> Deserialize<'a>,
    {
        // validate location
        let location = match self.location {
            None => {
                return Err("Location field is required".to_string());
            }
            Some(loc) => loc,
        };
        let location = PathBuf::from(location);
        if let Err(e) = std::fs::create_dir_all(location.as_path()) {
            let s = format!("Failed to access location: {}", e.to_string());
            return Err(s);
        }
        // buffer size in KBs
        let buffer_size = match self.buffer_enabled {
            true => self.buffer_size.map(|size| size.to_bytes()).unwrap_or(0),
            false => 0,
        };
        // create Wal
        let config = WalConfig {
            location,
            size: self
                .storage_size
                .map(|size| size.to_bytes())
                .unwrap_or(usize::MAX),
            fsync: self.fsync,
            buffer_size,
        };
        let wal = Wal::with_config(config);
        Ok(wal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, Deserialize)]
    struct Log {
        id: usize,
        value: f32,
    }

    #[test]
    fn it_works() {
        let wal = WalBuilder::new().location("./tmp/dupe").build::<Log>();
        assert!(wal.is_ok());
    }

    #[test]
    fn read_after_write() {
        let location = "./tmp/testing";
        std::fs::remove_dir_all(location).ok();

        // write some data
        let wal = WalBuilder::new()
            .location(location)
            .disable_buffer()
            .build::<Log>()
            .unwrap();
        wal.write(Log { id: 1, value: 3.14 });
        wal.write(Log { id: 2, value: 6.14 });
        wal.write(Log { id: 3, value: 9.14 });
        drop(wal);

        // try reading data
        let wal = WalBuilder::new()
            .location(location)
            .disable_buffer()
            .build::<Log>()
            .unwrap();
        wal.flush();
        let data = wal.read().unwrap().collect::<Vec<_>>();
        assert_eq!(data.len(), 3);
    }
}
