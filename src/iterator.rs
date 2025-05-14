use crate::error::WalError;
use crate::storage::iterator::StorageIterator;
use crate::storage::meta::Meta;
use crate::wal::{MODE_IDLE, MODE_READ};
use crate::Wal;
use std::sync::atomic::Ordering::Relaxed;

pub struct WalIterator {
    wal: Wal,
    inner: StorageIterator,
}
impl WalIterator {
    pub fn new(wal: Wal) -> Result<Self, WalError> {
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
    type Item = Vec<u8>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some(v) => Some(v),
            None => {
                // release read lock when done
                if let Err(_) = self
                    .wal
                    .inner
                    .mode
                    .compare_exchange(MODE_READ, MODE_IDLE, Relaxed, Relaxed)
                {
                    panic!("Walcraft error: unable to release read lock on WAL");
                }
                None
            }
        }
    }
}
