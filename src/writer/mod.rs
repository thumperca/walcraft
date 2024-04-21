mod buffer;
mod manager;

use self::buffer::Buffer;
use self::manager::FileManager;
use std::ops::{Deref, DerefMut};
use std::sync::Mutex;

pub const PAGE_SIZE: usize = 4096; // 4 KB

pub(crate) struct Writer {
    buffer: Mutex<Buffer>,
    io: Mutex<FileManager>,
}

impl Writer {
    pub fn new(location: &str, size: usize) -> Self {
        Self {
            buffer: Mutex::new(Buffer::new()),
            io: Mutex::new(FileManager::new(location, size)),
        }
    }

    pub fn log(&mut self, msg: &[u8]) {
        // acquire lock on buffer
        let mut lock = self.buffer.lock().unwrap();
        // add data to buffer
        if lock.add(msg) {
            return;
        }
        // buffer not able to accept more data, due to being filled
        // create a new buffer
        let mut new_buffer = Buffer::new();
        new_buffer.add(msg);
        // swap the buffers
        let buffer = std::mem::replace(&mut *lock, new_buffer);
        // drop lock
        drop(lock);

        // acquire lock on io to add the buffer to file
        let data = buffer.inner();
        let mut lock = self.io.lock().unwrap();
        lock.commit(data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[ignore]
    #[test]
    fn it_works() {
        let mut writer = Writer::new("./tmp/", usize::MAX);
        let data = String::from("This is sparta");
        let data = data.as_bytes();
        writer.log(data);
        for _ in 0..10 {
            let data = [101; 420];
            writer.log(&data);
        }
    }
}
