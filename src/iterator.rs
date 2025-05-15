use crate::error::WalError;
use crate::storage::iterator::StorageIterator;
use crate::storage::meta::Meta;
use crate::wal::{MODE_IDLE, MODE_READ};
use crate::Wal;
use std::sync::atomic::Ordering::Relaxed;

/// A log entry in the Write Ahead Log (WAL).
/// This struct wraps a byte vector and provides methods to access the data
pub struct LogEntry {
    inner: Vec<u8>,
}

impl LogEntry {
    pub(crate) fn new(inner: Vec<u8>) -> Self {
        Self { inner }
    }

    /// Returns the raw byte data of the log entry.
    pub fn data(&self) -> &[u8] {
        &self.inner
    }

    /// Returns the length of the byte vector
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// A deserialization method for data written with `append_struct` method
    /// ## Returns
    /// - `Ok(T)` if the deserialization is successful
    /// - `Err(WalError)` if the deserialization fails
    pub fn to_struct<T: for<'a> serde::Deserialize<'a>>(&self) -> Result<T, WalError> {
        bincode::deserialize(&self.inner)
            .map_err(|e| WalError::DeserializationError(format!("Failed to deserialize: {}", e)))
    }
}

/// An iterator over the log entries in the Write Ahead Log (WAL).
///
/// This struct wraps a `StorageIterator` and provides methods to iterate over the log entries.
/// ## Usage
/// ```no_run
/// use walcraft::Wal;
///
/// fn main() {
///     let wal = Wal::new("/tmp/walcraft", Some(2000)).unwrap();
///     // get the iterator
///     let iterator = wal.iter().unwrap();
///     // Iterate over the log entries
///     for item in iterator {
///         println!("{:?}", item.data());
///     }
/// }
///
/// ```
pub struct WalIterator {
    wal: Wal,
    inner: StorageIterator,
}
impl WalIterator {
    pub(crate) fn new(wal: Wal) -> Result<Self, WalError> {
        let location = &wal.inner.config.location;
        let meta = Meta::read_from_file(location)?;
        let iterator = StorageIterator::new(meta);
        Ok(Self {
            wal,
            inner: iterator,
        })
    }
}

impl Iterator for WalIterator {
    type Item = LogEntry;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some(v) => Some(LogEntry::new(v)),
            None => {
                // release read lock when done
                if self
                    .wal
                    .inner
                    .mode
                    .compare_exchange(MODE_READ, MODE_IDLE, Relaxed, Relaxed)
                    .is_err()
                {
                    panic!("Walcraft error: unable to release read lock on WAL");
                }
                None
            }
        }
    }
}

impl Drop for WalIterator {
    fn drop(&mut self) {
        if self.inner.next().is_none() {
            return;
        }
        if self
            .wal
            .inner
            .mode
            .compare_exchange(MODE_READ, MODE_IDLE, Relaxed, Relaxed)
            .is_err()
        {
            panic!("Walcraft error: unable to release read lock on WAL");
        }
    }
}
