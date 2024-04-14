mod buffer;
mod manager;

use self::buffer::Buffer;
use self::manager::FileManager;
use std::ops::{Deref, DerefMut};
use std::sync::Mutex;

pub const PAGE_SIZE: usize = 4096; // 4 KB

pub(crate) struct Writer {
    buffer: Mutex<Buffer>,
    io: FileManager,
}

impl Writer {
    pub fn new(location: &str) -> Self {
        Self {
            buffer: Mutex::new(Buffer::new()),
            io: FileManager::new(location),
        }
    }

    pub fn log(&mut self, msg: &[u8]) {
        let mut lock = self.buffer.lock().unwrap();
        if lock.add(msg) {
            return;
        }
        let mut new_buffer = Buffer::new();
        new_buffer.add(msg);
        let buffer = std::mem::replace(&mut *lock, new_buffer);
        drop(lock);
        let data = buffer.inner();
        self.io.commit(data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[ignore]
    #[test]
    fn it_works() {
        let mut writer = Writer::new("./tmp/");
        let data = String::from("This is sparta");
        let data = data.as_bytes();
        writer.log(data);
        for _ in 0..10 {
            let data = [101; 420];
            writer.log(&data);
        }
    }
}
