use super::meta::Meta;
use super::segment::iterator::PageIterator;
use crate::storage::segment::FileSegment;

/// Internal iterator over files in storage
pub(crate) struct StorageIterator {
    meta: Meta,
    pointer: usize,
    iterator: Option<PageIterator>,
}

impl StorageIterator {
    pub fn new(meta: Meta) -> Self {
        Self {
            meta,
            pointer: 0,
            iterator: None,
        }
    }
}

impl Iterator for StorageIterator {
    type Item = Vec<u8>;

    fn next(&mut self) -> Option<Self::Item> {
        if let None = self.iterator {
            if self.pointer >= self.meta.segments.len() {
                return None;
            }
            let meta = &self.meta.segments[self.pointer];
            let path = FileSegment::get_path(self.meta.location.parent().unwrap(), meta.file_id);
            let segment = match FileSegment::open_existing(path, meta.page_size) {
                Ok(segment) => segment,
                Err(e) => {
                    println!(
                        "Walcraft - Error opening Segment: {}, Error: {:?}",
                        meta.file_id, e
                    );
                    return None;
                }
            };
            let iterator = PageIterator::new(segment);
            self.iterator = Some(iterator);
            self.pointer += 1;
        }
        let iterator = self.iterator.as_mut().unwrap();
        match iterator.next() {
            Some(v) => Some(v),
            None => {
                self.iterator = None;
                self.next()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Storage;
    use crate::tests::clean_test_dir;
    use crate::{WalConfig, PAGE_MULTIPLIER, TESTING_DIR};

    #[test]
    fn it_works() {
        clean_test_dir();
        // write data
        let config = WalConfig {
            location: TESTING_DIR.into(),
            size: PAGE_MULTIPLIER * 10,
            fsync: false,
            page_size: PAGE_MULTIPLIER,
            sync_interval: 100,
        };
        let mut storage = Storage::new(config).unwrap();
        for i in 1..=100 {
            let msg = format!("Item {}", i);
            storage.append(msg.as_bytes()).unwrap();
        }
        storage.flush().unwrap();
        drop(storage);

        // read data
        let meta = Meta::read_from_file(TESTING_DIR).unwrap();
        let wal_iterator = StorageIterator::new(meta);
        let data = wal_iterator.collect::<Vec<_>>();
        assert_eq!(data.len(), 100);
        assert_eq!(data.first().unwrap(), b"Item 1");
        assert_eq!(data.last().unwrap(), b"Item 100");
    }

    #[test]
    fn many_files_with_gc() {
        clean_test_dir();
        // write data
        let config = WalConfig {
            location: TESTING_DIR.into(),
            size: PAGE_MULTIPLIER * 10,
            fsync: false,
            page_size: PAGE_MULTIPLIER,
            sync_interval: 100,
        };
        let mut storage = Storage::new(config).unwrap();
        for i in 1..=5000 {
            let msg = format!("Item {}", i);
            storage.append(msg.as_bytes()).unwrap();
        }
        storage.flush().unwrap();
        drop(storage);

        // read data
        let meta = Meta::read_from_file(TESTING_DIR).unwrap();
        assert_eq!(meta.segments.len(), 2);
        let wal_iterator = StorageIterator::new(meta);
        let data = wal_iterator.collect::<Vec<_>>();
        assert_eq!(data.len(), 1561);
        assert_eq!(data.first().unwrap(), b"Item 3440");
        assert_eq!(data.last().unwrap(), b"Item 5000");
    }
}
