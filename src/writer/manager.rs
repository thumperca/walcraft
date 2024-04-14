use super::Buffer;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

const MAX_FILE_SIZE: usize = 480 * 1024 * 1024; // 480 MB

struct GcSettings {
    enabled: bool,
    current_pointer: usize,
    gc_pointer: usize,
}

impl Default for GcSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            current_pointer: 1,
            gc_pointer: 0,
        }
    }
}

struct FileSettings {
    size_per_file: usize,
    total_files: usize,
}

impl Default for FileSettings {
    fn default() -> Self {
        Self {
            size_per_file: MAX_FILE_SIZE,
            total_files: usize::MAX,
        }
    }
}

pub(crate) struct FileManager {
    location: PathBuf,
    file: File,
    filled: usize,
    file_config: FileSettings,
    gc: GcSettings,
}

impl FileManager {
    pub fn new(path: &str) -> Self {
        let location = PathBuf::from(path);
        let mut file_path = location.clone();
        file_path.push("log_1.bin");
        let (file, filled) = Self::open_file(file_path).expect("Failed to open WAL file");
        Self {
            location,
            file,
            filled,
            file_config: Default::default(),
            gc: Default::default(),
        }
    }

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
