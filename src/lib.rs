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

mod builder;
pub(crate) mod error;
mod iter;
mod storage;
pub(crate) mod tests;
mod wal;
pub(crate) mod writer;

pub use self::builder::WalBuilder;
pub use self::wal::Wal;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_BUFFER_SIZE: usize = 4096; // 4 KB
pub const WAL_VERSION: usize = 1;
pub const TESTING_DIR: &str = "./tmp/testing";
pub const MAX_STORAGE: Size = Size::Gb(32768); // 32 TB
pub const MIN_SIZE_PER_FILE: Size = Size::Kb(16); // at least 16 KB files with 12 KB of data storage
pub const IDEAL_NUM_FILES: usize = 100; // ideal number of files to be created
pub const PAGE_MULTIPLIER: usize = 4096; // 4 KB page size

/// Represents the size of data in KBs, MBs or GBs, such as
/// - `Size::Kb(8)` means 8 KB
/// - `Size::Mb(16)` means 16 MB
/// - `Size::Gb(2)` means 2 GB
pub enum Size {
    Kb(usize),
    Mb(usize),
    Gb(usize),
}

impl Size {
    pub fn to_bytes(&self) -> usize {
        match self {
            Size::Kb(kb) => *kb * 1024,
            Size::Mb(mb) => *mb * 1024 * 1024,
            Size::Gb(gb) => *gb * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone)]
struct WalConfig2 {
    /// location on directory where files shall be store
    location: PathBuf,
    /// maximum storage size to be taken in KBs
    size: usize,
    /// sync is on or off
    fsync: bool,
    /// the size of each page block; shall be in multiples of 4 KB
    page_size: usize,
    /// automatic log sync interval in milliseconds
    sync_interval: usize,
}

impl Default for WalConfig2 {
    fn default() -> Self {
        Self {
            location: PathBuf::from("/tmp/walcraft"),
            size: usize::MAX,
            fsync: false,
            page_size: 4096,
            sync_interval: 250,
        }
    }
}

impl WalConfig2 {
    pub fn max_file_size(&self) -> usize {
        let size_per_file = (self.size / IDEAL_NUM_FILES) / PAGE_MULTIPLIER * PAGE_MULTIPLIER;
        if size_per_file < MIN_SIZE_PER_FILE.to_bytes() {
            MIN_SIZE_PER_FILE.to_bytes()
        } else {
            size_per_file
        }
    }
}

/// A Data object that holds configuration for [Wal]
#[derive(Serialize, Deserialize, Clone)]
struct WalConfig {
    // location on directory where files shall be store
    location: PathBuf,
    // maximum storage size to be taken in KBs
    size: usize,
    // sync is on or off
    fsync: bool,
    // a value of zero means buffer is disabled
    buffer_size: usize,
    // Auto sync interval in milliseconds
    // sync_interval: u64,
    // The maximum size of a single item in the log
    // max_item_size: usize,
}

impl Default for WalConfig {
    fn default() -> Self {
        Self {
            location: Default::default(),
            size: usize::MAX,
            fsync: false,
            buffer_size: DEFAULT_BUFFER_SIZE,
        }
    }
}

#[cfg(test)]
mod lib_tests {
    use super::*;

    #[test]
    fn small_size() {
        let mut config = WalConfig2::default();
        config.size = 1024 * 1024; // 1 MB
        assert_eq!(config.max_file_size(), 1024 * 16); // 16 KB
    }

    #[test]
    fn large_size() {
        let mut config = WalConfig2::default();
        config.size = 1024 * 1024 * 100; // 100 MB
        assert_eq!(config.max_file_size(), 1024 * 1024); // 1 MB
    }
}
