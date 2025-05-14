//! A Write Ahead Log (WAL) solution for concurrent environments
//!
//! # How?
//! This library provides high performance by using an in-memory buffer and append-only logs.
//! The logs are stored in multiple files, and older files are deleted to save space.
//!
//!  # Usage
//!
//! ```no_run
//! use serde::{Deserialize, Serialize};
//! use walcraft::Wal;
//!
//! // Log to write
//! #[derive(Serialize, Deserialize, Debug)]
//! struct Log {
//!     id: usize,
//!     value: f64
//! }
//!
//! // create an instance of WAL
//! let wal = Wal::new("/tmp/logz", Some(2000)).unwrap();
//!
//! // recovery: Option A
//! let all_logs = wal.iter().unwrap().collect::<Vec<_>>();
//! // recovery: Option B
//! for log in wal.iter().unwrap() {
//!   // do something with logs
//!   dbg!(log);
//! }
//!
//! // start writing
//! wal.append_struct(Log{id: 1, value: 3.14}).unwrap();
//! wal.append_struct(Log{id: 2, value: 4.20}).unwrap();
//!
//! // Flush to disk early/manually, before the buffer is filled
//! wal.flush().unwrap();
//!```
use crate::error::WalError;
use crate::iterator::WalIterator;
use crate::storage::Storage;
use crate::{WalConfig, PAGE_MULTIPLIER};
use serde::Serialize;
use std::fs::remove_dir_all;
use std::path::PathBuf;
use std::sync::atomic::Ordering::Acquire;
use std::sync::atomic::{AtomicU8, Ordering::Relaxed};
use std::sync::{Arc, Mutex};

pub(crate) const MODE_IDLE: u8 = 0;
pub(crate) const MODE_READ: u8 = 1;
const MODE_WRITE: u8 = 2;

pub(crate) struct WalInner {
    pub config: WalConfig,
    pub mode: AtomicU8,
    pub storage: Mutex<Storage>,
}

impl WalInner {
    pub fn new(config: WalConfig) -> Result<Self, WalError> {
        let storage = Storage::new(config.clone())?;
        Ok(Self {
            storage: Mutex::new(storage),
            mode: AtomicU8::new(MODE_IDLE),
            config,
        })
    }
}

#[derive(Clone)]
pub struct Wal {
    pub(crate) inner: Arc<WalInner>,
}

impl Wal {
    /// Create a new instance of [Wal]
    /// # Arguments
    /// - location: Location where the files shall be stored
    /// - size: Optional, maximum storage size taken by logs in MBs
    pub fn new(location: &str, size: Option<u16>) -> Result<Self, WalError> {
        let size = size.map(|v| v as usize * 1024 * 1024).unwrap_or(usize::MAX);
        let config = WalConfig {
            location: PathBuf::from(location),
            size,
            fsync: false,
            page_size: PAGE_MULTIPLIER,
            sync_interval: 250,
        };
        let inner = WalInner::new(config)?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    pub(crate) fn with_config(config: WalConfig) -> Result<Self, WalError> {
        let inner = WalInner::new(config)?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Read the logs
    pub fn iter(&self) -> Result<WalIterator, WalError> {
        if let Err(_) = self
            .inner
            .mode
            .compare_exchange(MODE_IDLE, MODE_READ, Relaxed, Relaxed)
        {
            return Err(WalError::LockError(
                "Unable to acquire read lock on WAL".to_string(),
            ));
        }
        WalIterator::new(self.clone())
    }

    /// Write a new log
    pub fn append(&self, item: &[u8]) -> Result<(), WalError> {
        // ensure write mode is either ON
        // or enable it if it's not ON
        let mode = self.inner.mode.load(Relaxed);
        if mode != MODE_WRITE {
            if let Err(d) = self
                .inner
                .mode
                .compare_exchange(MODE_IDLE, MODE_WRITE, Acquire, Relaxed)
            {
                // check if another thread hasn't already set the value
                if d != MODE_WRITE {
                    panic!("Walcraft Error: Writing logs while reading data is forbidden");
                }
            }
        }
        self.inner.storage.lock().unwrap().append(item)
    }

    /// Write a serializable object to the log
    pub fn append_struct<T: Serialize>(&self, item: T) -> Result<(), WalError> {
        let item = bincode::serialize(&item).map_err(|e| {
            WalError::SerializationError(format!("Unable to serialize struct, Error: {:?}", e))
        })?;
        self.append(&item)
    }

    /// Sync the in-memory buffer with Disk IO
    pub fn flush(&self) -> Result<(), WalError> {
        self.inner
            .storage
            .lock()
            .unwrap()
            .flush(self.inner.config.fsync)
    }

    /// Delete all the stored logs... Use Carefully!
    pub fn purge(&self) {
        let _ = remove_dir_all(self.inner.config.location.as_path());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{clean_test_dir, TESTING_DIR};
    use serde::Deserialize;

    #[derive(Serialize, Deserialize, Clone)]
    struct Log {
        id: usize,
        name: String,
    }

    fn bytes_to_log(bytes: Vec<u8>) -> Log {
        bincode::deserialize(&bytes).unwrap()
    }

    #[test]
    fn read_after_write() {
        clean_test_dir();
        // create a wal instance
        let wal = Wal::new(TESTING_DIR, Some(100)).unwrap();
        // add 2 logs
        wal.append_struct(Log {
            id: 420,
            name: "Jane Doe".to_string(),
        })
        .unwrap();
        wal.append_struct(Log {
            id: 840,
            name: "John Doe".to_string(),
        })
        .unwrap();
        // ensure data is written to disk
        wal.flush().unwrap();
        drop(wal);
        // read it
        let wal = Wal::new(TESTING_DIR, Some(100)).unwrap();
        let logs = wal.iter();
        assert!(logs.is_ok());
        let mut logs = logs.unwrap();
        // check item 1
        let item = logs.next();
        assert!(item.is_some());
        let item = item.map(bytes_to_log).unwrap();
        assert_eq!(item.id, 420);
        assert_eq!(&item.name, "Jane Doe");
        // check item 2
        let item = logs.next();
        assert!(item.is_some());
        let item = item.map(bytes_to_log).unwrap();
        assert_eq!(item.id, 840);
        assert_eq!(&item.name, "John Doe");
        // no item 3
        assert!(logs.next().is_none());
    }

    #[test]
    fn write_after_read() {
        clean_test_dir();
        // add some data
        let wal = Wal::new(TESTING_DIR, Some(500)).unwrap();
        for i in 0..20 {
            wal.append_struct(Log {
                id: i + 1,
                name: "".to_string(),
            })
            .unwrap();
        }
        wal.flush().unwrap();
        drop(wal);
        // read data
        let wal = Wal::new(TESTING_DIR, Some(500)).unwrap();
        let data = wal
            .iter()
            .unwrap()
            .into_iter()
            .map(bytes_to_log)
            .collect::<Vec<Log>>();
        assert_eq!(data.len(), 20);
        // write more data
        for i in 20..25 {
            wal.append_struct(Log {
                id: i + 1,
                name: "".to_string(),
            })
            .unwrap();
        }
        wal.flush().unwrap();
        drop(wal);
        // read to ensure everything new is also there
        let wal = Wal::new(TESTING_DIR, Some(500)).unwrap();
        let data = wal
            .iter()
            .unwrap()
            .into_iter()
            .map(bytes_to_log)
            .collect::<Vec<Log>>();
        assert_eq!(data.len(), 25);
        assert_eq!(data.first().unwrap().id, 1);
        assert_eq!(data.last().unwrap().id, 25);
    }
}
