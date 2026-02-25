use crate::storage::segment::FileSegment;
use std::collections::VecDeque;

pub(crate) struct PageIterator {
    segment: FileSegment,
    current_page: u32,
    buffer: VecDeque<Vec<u8>>,
}

impl PageIterator {
    pub fn new(segment: FileSegment) -> Self {
        PageIterator {
            segment,
            current_page: 0,
            buffer: VecDeque::new(),
        }
    }
}

impl Iterator for PageIterator {
    type Item = Vec<u8>;

    fn next(&mut self) -> Option<Self::Item> {
        // buffer contains records
        if !self.buffer.is_empty() {
            return self.buffer.pop_front();
        }
        // read from next page
        loop {
            self.current_page += 1;
            // reached end of file
            if self.current_page > self.segment.header.num_pages {
                return None;
            }
            // read next page
            let page = match self.segment.read_page(self.current_page) {
                Ok(page) => page,
                Err(e) => {
                    println!(
                        "Walcraft - Error reading Segment: {}, Page: {}, Error: {:?}",
                        self.segment.header.segment_id, self.current_page, e
                    );
                    continue;
                }
            };
            // add entries to buffer
            let entries = page.read(self.segment.header.length_prefix);
            self.buffer.extend(entries);
            break;
        }
        self.buffer.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::clean_test_dir;
    use crate::{PAGE_MULTIPLIER, TESTING_DIR};
    use std::io::{Seek, SeekFrom, Write};

    #[test]
    fn it_works() {
        clean_test_dir();
        // Add some data to the segment
        let mut segment =
            FileSegment::create_new(TESTING_DIR, 1, PAGE_MULTIPLIER, PAGE_MULTIPLIER * 100)
                .unwrap();
        assert!(segment.append(b"Hello"));
        assert!(segment.append(b"World"));
        segment.flush(false).unwrap();
        drop(segment);
        // open segment
        let path = FileSegment::get_path(TESTING_DIR, 1);
        let segment =
            FileSegment::open_existing(path.to_str().unwrap(), PAGE_MULTIPLIER * 100).unwrap();
        // Iterate over the pages
        let mut iterator = PageIterator::new(segment).collect::<Vec<_>>();
        assert_eq!(iterator.len(), 2);
        assert_eq!(iterator.pop().unwrap(), b"World");
        assert_eq!(iterator.pop().unwrap(), b"Hello");
    }

    #[test]
    fn corrupted_page_is_skipped() {
        clean_test_dir();
        // Write data across two pages: fill page 1, then write to page 2
        let mut segment =
            FileSegment::create_new(TESTING_DIR, 1, PAGE_MULTIPLIER, PAGE_MULTIPLIER * 100)
                .unwrap();
        // Fill page 1 with a large entry
        let large = vec![0xAB; PAGE_MULTIPLIER - 16];
        assert!(segment.append(&large));
        // Page 2 gets a small entry
        assert!(segment.append(b"survivor"));
        segment.flush(false).unwrap();
        drop(segment);

        // Corrupt page 1 on disk (byte inside the first page's payload area)
        let path = FileSegment::get_path(TESTING_DIR, 1);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(path.as_path())
            .unwrap();
        // Page 1 starts at offset PAGE_MULTIPLIER (after header); corrupt a payload byte
        let corrupt_offset = PAGE_MULTIPLIER + 20;
        file.seek(SeekFrom::Start(corrupt_offset as u64)).unwrap();
        file.write_all(&[0xFF]).unwrap();
        file.sync_all().unwrap();
        drop(file);

        // Re-open and iterate — page 1 should be skipped, page 2 should still read
        let segment =
            FileSegment::open_existing(path.to_str().unwrap(), PAGE_MULTIPLIER * 100).unwrap();
        let entries: Vec<_> = PageIterator::new(segment).collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], b"survivor");
    }
}
