use crate::writer::PAGE_SIZE;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

const MAX_FILE_SIZE: usize = 10 * 1024 * 1024 * 1024; // 10 GB
const NUM_FILES_SPLIT: usize = 4;

pub(crate) struct Meta {
    location: PathBuf,
}

impl Meta {
    pub fn new(dir_path: PathBuf) -> Self {
        let mut path = dir_path;
        path.push("meta");
        Self { location: path }
    }

    pub fn read(&self) -> Option<(usize, usize)> {
        let content = std::fs::read_to_string(&self.location).ok()?;
        let d = content
            .split_whitespace()
            .filter_map(|v| v.parse::<usize>().ok())
            .collect::<Vec<usize>>();
        if d.len() != 2 {
            return None;
        }
        Some((d[0], d[1]))
    }

    pub fn write(&self, v: (usize, usize)) {
        let content = format!("{} {}", v.0, v.1);
        let mut file = match File::create(&self.location) {
            Ok(v) => v,
            Err(err) => return eprintln!("Failed to write meta info: {:?}", err),
        };
        if let Err(e) = file.write_all(content.as_bytes()) {
            eprintln!("Failed to write meta to file: {}", e);
        }
    }
}

struct FileConfig {
    /// Number of total files to have
    /// Defaults to `usize::MAX` in case of absence of any size restrictions
    max_files: usize,
    /// How much data to store per file
    size_per_file: usize,
    /// Postfix of current file where data is being stored
    current_pointer: usize,
    /// Pointer to garbage collector
    /// This is always be 0 if there are no size restrictions on WAL
    gc_pointer: usize,
}

impl Default for FileConfig {
    fn default() -> Self {
        Self {
            max_files: usize::MAX - 10,
            size_per_file: MAX_FILE_SIZE,
            current_pointer: 0,
            gc_pointer: 0,
        }
    }
}

impl FileConfig {
    pub fn new(size: usize) -> Self {
        // calculate how much data to store per file
        let mut capacity = std::cmp::min(size / NUM_FILES_SPLIT, MAX_FILE_SIZE);
        capacity = std::cmp::max(capacity, PAGE_SIZE);
        // create a conf object
        let mut conf = Self::default();
        conf.size_per_file = capacity;
        // set how many maximum files shall be there
        conf.max_files = if size % capacity == 0 {
            size / capacity + 1
        } else {
            size / capacity + 2
        };
        // sync with disk
        conf
    }
}

pub(crate) struct FileManager {
    location: PathBuf,
    file: File,
    filled: usize,
    config: FileConfig,
}

impl FileManager {
    pub fn new(path: &str, size: usize) -> Self {
        let location = PathBuf::from(path);
        let mut config = FileConfig::new(size);
        let meta = Meta::new(location.clone());
        if let Some(data) = meta.read() {
            config.gc_pointer = data.0;
            config.current_pointer = data.1;
        }
        meta.write((config.gc_pointer, config.current_pointer));

        let current_file = format!("log_{}.bin", config.current_pointer);
        let mut file_path = location.clone();
        file_path.push(current_file);

        let (file, filled) = Self::open_file(file_path).expect("Failed to open WAL file");
        Self {
            location,
            file,
            filled,
            config,
        }
    }

    /// Write the change to file
    pub fn commit(&mut self, data: &[u8]) {
        let written = match self.file.write(data) {
            Ok(size) => size,
            Err(e) => {
                return println!("Failed to write to file: {}", e);
            }
        };
        self.filled += written;
        if self.filled >= self.config.size_per_file {
            self.next_file()
        }
    }

    // Open next file and run garbage collection
    fn next_file(&mut self) {
        // set a new pointer
        let (new_pointer, _) = self.config.current_pointer.overflowing_add(1);
        self.config.current_pointer = new_pointer;
        // run garbage collection
        self.gc();
        let meta = Meta::new(self.location.clone());
        meta.write((self.config.gc_pointer, self.config.current_pointer));
        // open new file
        let file_name = format!("log_{}.bin", new_pointer);
        let mut file_path = self.location.clone();
        file_path.push(file_name);
        let _ = std::fs::remove_file(&file_path); // remove the file in case it exists
        let d = Self::open_file(file_path).expect("Failed to open next WAL file");
        self.file = d.0;
        self.filled = d.1;
    }

    // Run garbage collection on files
    // i.e. delete files beyond max_files limit
    fn gc(&mut self) {
        let current = self.config.current_pointer;
        let mut gc_pointer = self.config.gc_pointer;
        // check files between the two pointers
        let mut diff = 0;
        if current > gc_pointer {
            diff = current - gc_pointer;
        } else if gc_pointer > current {
            diff = usize::MAX - (gc_pointer - current) + 1;
        }
        // no GC needed
        if diff <= self.config.max_files {
            return;
        }

        // GC is needed
        let del_count = diff - self.config.max_files;
        let mut counter = 0;
        // delete files upto `del_count`
        while counter <= del_count {
            let file_name = format!("log_{}.bin", gc_pointer);
            let mut file_path = self.location.clone();
            file_path.push(&file_name);
            let _ = std::fs::remove_file(file_path).unwrap();
            // increment counter
            gc_pointer = gc_pointer.overflowing_add(1).0;
            counter += 1;
        }
        // set a new garbage pointer
        self.config.gc_pointer = gc_pointer;
    }

    /// Create or open the current file to write logs to
    ///
    /// ## Returns
    /// A tuple with 2 values:
    /// - 0: the handle to opened file
    /// - 1: size of data in the current file
    ///
    fn open_file(path: PathBuf) -> Result<(File, usize), ()> {
        // open the current file in append mode
        let file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&path)
            .map_err(|_| ())?;

        // read size of current file
        let meta_data = file.metadata().map_err(|_| ())?;
        let filled = meta_data.len() as usize;
        Ok((file, filled))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garbage_collection() {
        let location = "./tmp/testing";
        let _ = std::fs::remove_dir_all(location);
        std::fs::create_dir(location).unwrap();
        // create some files
        for i in 0..10 {
            let path = format!("{}/log_{}.bin", location, i);
            let _ = File::create(&path).unwrap();
        }
        // set a pointer
        let meta = Meta::new(PathBuf::from(location));
        meta.write((0, 9));

        // write to manager to test that the GC ran
        let mut manager = FileManager::new("./tmp/testing", PAGE_SIZE * NUM_FILES_SPLIT); // 1MB
        assert_eq!(manager.config.max_files, 5);
        for _ in 0..2 {
            let data = [101; PAGE_SIZE];
            manager.commit(&data);
        }

        // run tests
        let meta = Meta::new(PathBuf::from(location));
        let (gc, cp) = meta.read().unwrap();
        assert_eq!(gc, 6);
        assert_eq!(cp, 11);
        assert_eq!(PathBuf::from("./tmp/testing/log_1.bin").exists(), false);
        assert_eq!(PathBuf::from("./tmp/testing/log_5.bin").exists(), false);
        assert_eq!(PathBuf::from("./tmp/testing/log_6.bin").exists(), true);
        assert_eq!(PathBuf::from("./tmp/testing/log_10.bin").exists(), true);
        assert_eq!(PathBuf::from("./tmp/testing/log_11.bin").exists(), true);
    }

    // Test garbage collection when logs until
    #[test]
    fn garbage_collection_cyclic() {
        let location = "./tmp/testing";
        let _ = std::fs::remove_dir_all(location);
        std::fs::create_dir(location).unwrap();
        // create some files - 10 in end and 3 in start of usize range
        for i in 0..3 {
            let path = format!("{}/log_{}.bin", location, i);
            let _ = File::create(&path).unwrap();
        }

        for i in (usize::MAX - 9)..=usize::MAX {
            let path = format!("{}/log_{}.bin", location, i);
            let _ = File::create(&path).unwrap();
        }
        // set a pointer
        let meta = Meta::new(PathBuf::from(location));
        meta.write((usize::MAX - 9, 1));

        // write to manager to test that the GC ran
        let mut manager = FileManager::new("./tmp/testing", PAGE_SIZE * NUM_FILES_SPLIT);
        assert_eq!(manager.config.max_files, 5);
        for _ in 0..2 {
            let data = [101; PAGE_SIZE];
            manager.commit(&data);
        }

        // run tests
        let meta = Meta::new(PathBuf::from(location));
        let (gc, cp) = meta.read().unwrap();
        assert_eq!(gc, usize::MAX - 1);
        assert_eq!(cp, 3);
        assert_eq!(PathBuf::from("./tmp/testing/log_1.bin").exists(), true);
        assert_eq!(PathBuf::from("./tmp/testing/log_3.bin").exists(), true);
        assert_eq!(
            PathBuf::from(format!("./tmp/testing/log_{}.bin", usize::MAX)).exists(),
            true
        );
        assert_eq!(
            PathBuf::from(format!("./tmp/testing/log_{}.bin", usize::MAX - 1)).exists(),
            true
        );
        assert_eq!(
            PathBuf::from(format!("./tmp/testing/log_{}.bin", usize::MAX - 3)).exists(),
            false
        );
    }

    #[test]
    fn overflowing_arithmetics() {
        let v = usize::MAX - 1;
        let (new_v, of) = v.overflowing_add(1);
        assert_eq!(new_v, usize::MAX);
        assert_eq!(of, false);

        let (new_v, of) = v.overflowing_add(5);
        assert_eq!(new_v, 3);
        assert_eq!(of, true);
    }
}
