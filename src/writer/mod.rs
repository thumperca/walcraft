mod buffer;
pub(crate) mod manager;

use self::buffer::Buffer;
use self::manager::FileManager;
use std::path::PathBuf;
use std::sync::Mutex;

pub const PAGE_SIZE: usize = 4096; // 4 KB

/// Log Writer responsible for writing the information to the buffer as well as on disk
pub(crate) struct Writer {
    buffer: Mutex<Buffer>,
    io: Mutex<FileManager>,
}

impl Writer {
    /// Create a new Log Writer
    ///
    /// ## Arguments
    /// - `location`: Location where the log files shall be stored
    /// - `size`: Maximum amount of data that can be stored, in bytes
    pub fn new(location: PathBuf, size: usize) -> Self {
        Self {
            buffer: Mutex::new(Buffer::new()),
            io: Mutex::new(FileManager::new(location, size)),
        }
    }

    /// Add a new log
    ///
    /// This method will either write the log to the buffer or a file
    ///
    /// ## Arguments
    /// - `msg`: The log data to be written
    ///
    pub fn log(&self, msg: &[u8]) {
        // acquire lock on buffer
        let mut lock = self.buffer.lock().unwrap();
        // add data to buffer
        let (added, flush) = lock.try_add(msg);
        if added && !flush {
            return;
        }
        // buffer not able to accept more data, due to being filled
        // create a new buffer
        let mut new_buffer = Buffer::new();
        if !added {
            new_buffer.try_add(msg);
        }
        // swap the buffers
        let buffer = std::mem::replace(&mut *lock, new_buffer);
        // drop lock
        drop(lock);

        // acquire lock on io to add the buffer to file
        if flush {
            let data = buffer.consume(true);
            let mut lock = self.io.lock().unwrap();
            lock.commit(&data);
        }
    }

    /// Flush the in-memory buffer to Disk, if any data exists in the buffer
    pub fn flush(&self) {
        // get buffer
        let mut lock = self.buffer.lock().unwrap();
        let buffer = std::mem::replace(&mut *lock, Buffer::new());
        drop(lock);
        // acquire lock on io to add the buffer to file
        let data = buffer.consume(false);
        if !data.is_empty() {
            let mut lock = self.io.lock().unwrap();
            lock.commit(&data);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let writer = Writer::new("./tmp/".into(), usize::MAX);
        let data = String::from("This is sparta");
        let data = data.as_bytes();
        writer.log(data);
        for _ in 0..10 {
            let data = [101; 420];
            writer.log(&data);
        }
    }
}
