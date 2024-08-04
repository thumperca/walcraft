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
