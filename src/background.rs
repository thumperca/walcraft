use crate::Wal;

pub(crate) struct BackgroundSync {
    wal: Wal,
}

impl BackgroundSync {
    pub(crate) fn new(wal: Wal) -> Self {
        Self { wal }
    }

    pub(crate) fn run(&self, interval: usize) {
        // Start the background sync thread
        let wal = self.wal.clone();
        std::thread::spawn(move || {
            loop {
                // Sleep for the specified interval
                std::thread::sleep(std::time::Duration::from_millis(interval as u64));
                // Perform the sync operation
                let _ = wal.flush();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{clean_test_dir, TESTING_DIR};
    use crate::Wal;

    #[test]
    fn it_works() {
        clean_test_dir();
        // write data in background
        let wal = Wal::new(TESTING_DIR, Some(100)).unwrap();
        wal.append(b"Hello").unwrap();
        wal.append(b"Hola").unwrap();
        BackgroundSync::new(wal).run(10);
        // read data
        std::thread::sleep(std::time::Duration::from_millis(20));
        let wal = Wal::new(TESTING_DIR, Some(100)).unwrap();
        let data = wal.iter().unwrap().collect::<Vec<_>>();
        assert_eq!(data.len(), 2);
        assert_eq!(data[0].data(), b"Hello");
        assert_eq!(data[1].data(), b"Hola");
    }
}
