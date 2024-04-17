use super::Buffer;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

const MAX_FILE_SIZE: usize = 2 * 1024 * 1024 * 1024; // 2 GB

struct Meta {
    location: PathBuf,
}

impl Meta {
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
        let content = format!("{}{}", v.0, v.1);
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
            max_files: usize::MAX,
            size_per_file: MAX_FILE_SIZE,
            current_pointer: 1,
            gc_pointer: 0,
        }
    }
}

impl FileConfig {
    pub fn new(size: usize) -> Self {
        let mut conf = Self::default();
        let capacity = std::cmp::min(size / 4, MAX_FILE_SIZE);
        conf.size_per_file = capacity;
        conf.max_files = if size % capacity == 0 {
            size / capacity
        } else {
            size / capacity + 1
        };
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
    pub fn new(path: &str, size: Option<usize>) -> Self {
        let location = PathBuf::from(path);

        let mut config = size.map(|v| FileConfig::new(v)).unwrap_or_default();
        let meta = Meta {
            location: location.clone(),
        };
        if let Some(data) = meta.read() {
            config.gc_pointer = data.0;
            config.current_pointer = data.1;
        }

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
        match self.file.write(data) {
            Ok(size) => {
                self.filled += size;
            }
            Err(e) => {
                println!("Failed to write to file: {}", e);
            }
        }
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
