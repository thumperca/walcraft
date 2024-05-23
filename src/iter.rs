use crate::writer::manager::Meta;
use crate::Wal;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::File;
use std::io::Read;

const BUFFER_SIZE: usize = 1024 * 8; // 8 KB

pub struct WalIterator<T>
where
    T: Serialize + for<'a> Deserialize<'a>,
{
    /// Handle to WAL instance
    wal: Wal<T>,
    /// Whether the init process has been done
    /// this would be false until the consumption of [WalIterator] starts
    started: bool,
    /// Identifier for when all the files has been read and the iterator has reached the end
    ended: bool,
    /// Handle to the current file
    file: Option<File>,
    /// Queue of all the files to read in the right sequence
    files: VecDeque<usize>,
    /// Buffer where the data is loaded from the file
    /// The [WalIterator] reads large files in chunks and stores them in the buffer
    /// This helps in reducing RAM usage for the iterator when reading from large files
    buffer: VecDeque<u8>,
}

impl<T> WalIterator<T>
where
    T: Serialize + for<'a> Deserialize<'a>,
{
    pub fn new(wal: Wal<T>) -> Self {
        Self {
            wal,
            started: false,
            ended: false,
            file: None,
            files: VecDeque::new(),
            buffer: VecDeque::with_capacity(BUFFER_SIZE), // 8 KB buffer
        }
    }

    fn init(&mut self) {
        match Meta::new(self.wal.inner.location.clone()).read() {
            None => {
                self.ended = true;
            }
            Some((garbage_pointer, current_pointer)) => {
                // calculate order of files to read in
                if current_pointer > garbage_pointer {
                    self.files = VecDeque::from_iter(garbage_pointer..=current_pointer);
                } else if garbage_pointer > current_pointer {
                    let mut files = VecDeque::from_iter(garbage_pointer..=(usize::MAX));
                    files.extend(0..=current_pointer);
                    self.files = files;
                } else {
                    self.files.push_back(current_pointer);
                }
                // check if the file is actually present
                if self.next_file().is_none() {
                    self.ended = true;
                }
            }
        };
        self.started = true;
    }

    fn get_next(&mut self) -> Option<T> {
        // lazy initialization
        if !self.started {
            self.init();
        }
        // the file list has been exhausted
        if self.ended {
            return None;
        }
        // get data from buffer
        self.read_buffer()
    }

    fn read_buffer(&mut self) -> Option<T> {
        loop {
            if !self.ensure_buffer() {
                return None;
            }
            let size = self.buffer.drain(0..2).collect::<Vec<_>>();
            let size = u16::from_ne_bytes([size[0], size[1]]) as usize;
            // insufficient or corrupted data
            if size == 0 || size > self.buffer.len() {
                return None;
            }
            // convert bytes to log
            let bytes = self.buffer.drain(0..size).collect::<Vec<_>>();
            if let Ok(item) = bincode::deserialize(&bytes) {
                return Some(item);
            }
        }
    }

    fn ensure_buffer(&mut self) -> bool {
        loop {
            // Clear an empty buffer
            if let Some(val) = self.buffer.get(0) {
                if *val == 0 {
                    self.buffer.clear();
                }
            }
            // has enough data in buffer to return one item
            if self.buffer.len() > 2 {
                let size = u16::from_ne_bytes([self.buffer[0], self.buffer[1]]) as usize;
                if size != 0 && self.buffer.len() >= (size + 2) {
                    return true;
                }
            }
            // in case of insufficient data, read next chunk
            // this will read from the same file, if there's more data in the file
            // otherwise it will try to open next file and read from it
            let file = self.file.as_mut().unwrap();
            let mut data = vec![0; 50];
            let bytes_read = file.read(&mut data).unwrap_or(0);
            if bytes_read == 0 {
                if self.next_file().is_none() {
                    return false;
                }
            } else {
                self.buffer.extend(data);
                return true;
            }
        }
    }

    fn next_file(&mut self) -> Option<&File> {
        loop {
            match self.files.pop_front() {
                None => {
                    self.ended = true;
                    break None;
                }
                Some(f) => {
                    let file_name = format!("log_{}.bin", f);
                    let mut path = self.wal.inner.location.clone();
                    path.push(&file_name);
                    let file = match File::open(path) {
                        Ok(f) => f,
                        Err(_) => continue,
                    };
                    self.file = Some(file);
                    break self.file.as_ref();
                }
            }
        }
    }
}

impl<T> Iterator for WalIterator<T>
where
    T: Serialize + for<'a> Deserialize<'a>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.get_next()
    }
}

#[cfg(test)]
mod tests {}
