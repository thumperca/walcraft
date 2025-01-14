mod buffer;
pub(crate) mod manager;

use self::buffer::Buffer;
use self::manager::FileManager;
use crate::WalConfig;
use std::sync::Mutex;

/// Log Writer responsible for writing the information to the buffer as well as on disk
pub(crate) struct Writer {
    buffer: Mutex<Buffer>,
    io: Mutex<FileManager>,
    config: WalConfig,
}

impl Writer {
    /// Create a new Log Writer
    ///
    /// ## Arguments
    /// - `location`: Location where the log files shall be stored
    /// - `size`: Maximum amount of data that can be stored, in bytes
    pub fn new(config: WalConfig) -> Self {
        Self {
            buffer: Mutex::new(Buffer::new(Some(config.buffer_size))),
            io: Mutex::new(FileManager::new(config.location.clone(), config.size)),
            config,
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
        // if buffer is disabled, write directly to file
        if self.config.buffer_size == 0 {
            return self.write(msg);
        }

        // Buffer is enabled
        // acquire lock on buffer
        let mut lock = self.buffer.lock().unwrap();
        // add data to buffer
        let (added, flush) = lock.try_add(msg);
        if added && !flush {
            return;
        }
        // buffer not able to accept more data, due to being filled
        // create a new buffer
        let mut new_buffer = Buffer::new(None);
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
            self.write(&data);
        }
    }

    /// Write the data to the file
    fn write(&self, msg: &[u8]) {
        let mut lock = self.io.lock().unwrap();
        lock.commit(msg);
        return;
    }

    /// Flush the in-memory buffer to Disk, if any data exists in the buffer
    pub fn flush(&self) {
        // get buffer
        let mut lock = self.buffer.lock().unwrap();
        let buffer = std::mem::replace(&mut *lock, Buffer::new(None));
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
        let mut config = WalConfig::default();
        config.location = "./tmp/".into();
        let writer = Writer::new(config);
        let data = String::from("This is sparta");
        let data = data.as_bytes();
        writer.log(data);
        for _ in 0..10 {
            let data = [101; 420];
            writer.log(&data);
        }
    }
}
