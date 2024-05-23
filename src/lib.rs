//! A Write Ahead Log (WAL) solution for concurrent environments
//!
//! # How?
//! This library gives high performance/throughput by using in-memory buffer and leveraging append-only logs.
//! The logs are split across multiple files. The older files are deleted to preserve the capacity constraints.
//!
//! # Usage
//! ```
//! use serde::{Deserialize, Serialize};
//! use walcraft::Wal;
//!
//! // Log to write
//! #[derive(Serialize, Deserialize, Clone)]
//! struct Log {
//!     id: usize,
//!     value: f64
//! }
//! let log = Log {id: 1, value: 5.6234};
//!
//! // initiate wal and add a log
//! let wal = Wal::new("./tmp/", Some(500)); // 500MB of log capacity
//! wal.write(log); // write a log
//!
//! // write a log in another thread
//! let wal2 = wal.clone();
//! std::thread::spawn(move || {
//!     let log = Log{id: 2, value: 0.45};
//!     wal2.write(log);
//! });
//!
//! // keep writing logs in current thread
//! let log = Log{id: 3, value: 123.59};
//! wal.write(log);
//!
//! // Flush the logs to the disk manually
//! // This happens automatically as well after some time. However, it's advised to
//! // run this method before terminating the program to ensure that no logs are lost.
//! wal.flush();
//! ```
//!

mod iter;
pub(crate) mod writer;

use self::writer::Writer;
use crate::iter::WalIterator;
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

struct WalInner<T>
where
    T: Serialize + for<'a> Deserialize<'a>,
{
    mode: AtomicU8,
    writer: Writer,
    location: PathBuf,
    _phantom: PhantomData<T>,
}

impl<T> WalInner<T>
where
    T: Serialize + for<'a> Deserialize<'a>,
{
    pub fn new(location: &str, size: usize) -> Self {
        Self {
            mode: AtomicU8::new(MODE_IDLE),
            writer: Writer::new(location, size),
            location: PathBuf::from(location),
            _phantom: PhantomData,
        }
    }
}

#[derive(Clone)]
pub struct Wal<T>
where
    T: Serialize + for<'a> Deserialize<'a>,
{
    inner: Arc<WalInner<T>>,
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
        let size = size.map(|v| v as usize * 1024).unwrap_or(usize::MAX);
        let inner = WalInner::new(location, size);
        Self {
            inner: Arc::new(inner),
        }
    }

    /// Read the logs
    pub fn iter(&self) -> Option<WalIterator<T>> {
        if let Err(_) = self
            .inner
            .mode
            .compare_exchange(MODE_IDLE, MODE_READ, Relaxed, Relaxed)
        {
            return None;
        }
        let wal = Wal {
            inner: self.inner.clone(),
        };
        let t = WalIterator::new(wal);
        Some(t)
    }

    /// Read all stored logs
    pub fn read_all(&self) -> Vec<T> {
        todo!()
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
                    return;
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
        let _ = remove_dir_all(self.inner.location.as_path());
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

    #[test]
    fn read_after_write() {
        // reset the folder
        let location = "./tmp/testing";
        let _ = std::fs::remove_dir_all(location);
        std::fs::create_dir(location).unwrap();
        // create a wal instance
        let wal = Wal::new(location, Some(100));
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
        let wal: Wal<Log> = Wal::new(location, Some(100));
        let logs = wal.iter();
        assert!(logs.is_some());
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
}
