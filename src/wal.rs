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
//! let wal = Wal::new("/tmp/logz", Some(2000));
//!
//! // recovery: Option A
//! let all_logs = wal.read().unwrap().into_iter().collect::<Vec<Log> > ();
//! // recovery: Option B
//! for log in wal.read().unwrap() {
//!   // do something with logs
//!   dbg!(log);
//! }
//!
//! // start writing
//! wal.write(Log{id: 1, value: 3.14});
//! wal.write(Log{id: 2, value: 4.20});
//!
//! // Flush to disk early/manually, before the buffer is filled
//! wal.flush();
//!```
use crate::iter::WalIterator;
use crate::writer::Writer;
use crate::{WalConfig, DEFAULT_BUFFER_SIZE};
use serde::{Deserialize, Serialize};
use std::fs::remove_dir_all;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::atomic::Ordering::Acquire;
use std::sync::atomic::{AtomicU8, Ordering::Relaxed};
use std::sync::Arc;

const MODE_IDLE: u8 = 0;
const MODE_READ: u8 = 1;
const MODE_WRITE: u8 = 2;

/// Represents size of data on KBs, MBs or GBs, such as:
/// - `Size::Kb(8)` means 8 KB
/// - `Size::Mb(16)` means 16 MB
/// - `Size::Gb(2)` means 2 GB
pub enum Size {
    Kb(usize),
    Mb(usize),
    Gb(usize),
}

pub(crate) struct WalInner<T>
where
    T: Serialize + for<'a> Deserialize<'a>,
{
    pub config: WalConfig,
    pub mode: AtomicU8,
    pub writer: Writer,
    _phantom: PhantomData<T>,
}

impl<T> WalInner<T>
where
    T: Serialize + for<'a> Deserialize<'a>,
{
    pub fn new(config: WalConfig) -> Self {
        Self {
            writer: Writer::new(config.clone()),
            mode: AtomicU8::new(MODE_IDLE),
            config,
            _phantom: PhantomData,
        }
    }
}

#[derive(Clone)]
pub struct Wal<T>
where
    T: Serialize + for<'a> Deserialize<'a>,
{
    pub(crate) inner: Arc<WalInner<T>>,
}

impl<T> Wal<T>
where
    T: Serialize + for<'a> Deserialize<'a>,
{
    /// Create a new instance of [Wal]
    /// # Arguments
    /// - location: Location where the files shall be stored
    /// - size: Optional, maximum storage size taken by logs in MBs
    pub fn new(location: &str, size: Option<u16>) -> Self {
        let size = size.map(|v| v as usize * 1024 * 1024).unwrap_or(usize::MAX);
        let config = WalConfig {
            location: PathBuf::from(location),
            fsync: false,
            buffer_size: DEFAULT_BUFFER_SIZE,
            size,
        };
        let inner = WalInner::new(config);
        Self {
            inner: Arc::new(inner),
        }
    }

    pub fn with_config(config: WalConfig) -> Self {
        let inner = Arc::new(WalInner::new(config));
        Self { inner }
    }

    /// Read the logs
    pub fn read(&self) -> Result<impl Iterator<Item = T>, String> {
        if let Err(_) = self
            .inner
            .mode
            .compare_exchange(MODE_IDLE, MODE_READ, Relaxed, Relaxed)
        {
            return Err("Unable to acquire read lock on WAL".to_string());
        }
        let wal = Wal {
            inner: self.inner.clone(),
        };
        let t = WalIterator::new(wal);
        Ok(t)
    }

    /// Write a new log
    pub fn write(&self, item: T) {
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
                    panic!("Writing logs while reading data is forbidden");
                }
            }
        }
        // write the data
        if let Ok(d) = bincode::serialize(&item) {
            self.inner.writer.log(&d);
        }
    }

    /// Sync the in-memory buffer with Disk IO
    pub fn flush(&self) {
        self.inner.writer.flush();
    }

    /// Delete all the stored logs... Use Carefully!
    pub fn purge(&self) {
        let _ = remove_dir_all(self.inner.config.location.as_path());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, Deserialize, Clone)]
    struct Log {
        id: usize,
        name: String,
    }

    const LOCATION: &str = "./tmp/testing";

    // reset the folder
    fn reset() {
        let _ = std::fs::remove_dir_all(LOCATION);
        std::fs::create_dir(LOCATION).unwrap();
    }

    #[test]
    fn read_after_write() {
        reset();
        // create a wal instance
        let wal = Wal::new(LOCATION, Some(100));
        // add 2 logs
        wal.write(Log {
            id: 420,
            name: "Jane Doe".to_string(),
        });
        wal.write(Log {
            id: 840,
            name: "John Doe".to_string(),
        });
        // ensure data is written to disk
        wal.flush();
        drop(wal);
        // read it
        let wal: Wal<Log> = Wal::new(LOCATION, Some(100));
        let logs = wal.read();
        assert!(logs.is_ok());
        let mut logs = logs.unwrap();
        // check item 1
        let item = logs.next();
        assert!(item.is_some());
        let item = item.unwrap();
        assert_eq!(item.id, 420);
        assert_eq!(&item.name, "Jane Doe");
        // check item 2
        let item = logs.next();
        assert!(item.is_some());
        let item = item.unwrap();
        assert_eq!(item.id, 840);
        assert_eq!(&item.name, "John Doe");
        // no item 3
        assert!(logs.next().is_none());
    }

    #[test]
    fn write_after_read() {
        reset();
        // add some data
        let wal = Wal::new(LOCATION, Some(500));
        for i in 0..20 {
            wal.write(Log {
                id: i + 1,
                name: "".to_string(),
            })
        }
        wal.flush();
        drop(wal);
        // read data
        let wal = Wal::new(LOCATION, Some(500));
        let data = wal.read().unwrap().into_iter().collect::<Vec<Log>>();
        assert_eq!(data.len(), 20);
        // write more data
        for i in 20..25 {
            wal.write(Log {
                id: i + 1,
                name: "".to_string(),
            })
        }
        wal.flush();
        drop(wal);
        // read to ensure everything new is also there
        let wal = Wal::new(LOCATION, Some(500));
        let data = wal.read().unwrap().into_iter().collect::<Vec<Log>>();
        assert_eq!(data.len(), 25);
        assert_eq!(data.first().unwrap().id, 1);
        assert_eq!(data.last().unwrap().id, 25);
    }
}
