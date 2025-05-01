use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub(crate) enum MetaError {
    IoError,
    SerializationError,
}

// todo: update meta information when new files are created
// sync this information to disk
// Disk sync shall happen when a new file is create and upon close on filled
// current_pointer can be used for this
#[derive(Serialize, Deserialize)]
pub(crate) struct Meta {
    gc_pointer: u32,
    current_pointer: u32,
    sizes: Vec<SizeEntry>,
}

#[derive(Serialize, Deserialize)]
struct SizeEntry {
    file_id: u32,
    page_size: usize,
    file_size: usize,
}

impl Meta {
    pub fn read_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, MetaError> {
        let contents = std::fs::read_to_string(path).map_err(|_| MetaError::IoError)?;
        toml::from_str(&contents).map_err(|_| MetaError::SerializationError)
    }

    pub fn write_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), MetaError> {
        let contents = toml::to_string(self).map_err(|_| MetaError::SerializationError)?;
        std::fs::write(path, contents).map_err(|_| MetaError::IoError)?;
        Ok(())
    }
}

#[test]
fn it_works() {
    // file path
    let mut path = std::path::PathBuf::from(crate::TESTING_DIR);
    path.push("meta_test.toml");
    // create a new file
    let meta = Meta {
        gc_pointer: 0,
        current_pointer: 101,
        sizes: vec![
            SizeEntry {
                file_id: 1,
                page_size: 4096,
                file_size: 8192,
            },
            SizeEntry {
                file_id: 2,
                page_size: 1024,
                file_size: 2048,
            },
        ],
    };
    meta.write_to_file(&path).unwrap();
    // read from file
    let meta = Meta::read_from_file(path).unwrap();
    assert_eq!(meta.gc_pointer, 0);
    assert_eq!(meta.current_pointer, 101);
    assert_eq!(meta.sizes.len(), 2);
}
