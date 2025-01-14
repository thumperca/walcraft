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
mod iter;
mod wal;
pub(crate) mod writer;

pub use self::builder::WalBuilder;
pub use self::wal::Wal;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_BUFFER_SIZE: usize = 4096; // 4 KB

/// Represents size of data on KBs, MBs or GBs, such as:
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
