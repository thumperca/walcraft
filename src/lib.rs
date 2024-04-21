// Wal > read
// Wal > write > buffer > file

// Wal -> Reader
// Wal -> Writer
// Wal -> Writer -> Buffer
// Wal -> Writer -> FileManager
// Wal -> Writer -> FileManager -> Queue[Buffer]
// Wal -> Writer -> FileManager -> FileHandle

mod writer;
use self::writer::Writer;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use std::sync::atomic::AtomicU8;
use std::sync::Arc;

const MODE_IDLE: u8 = 0;
const MODE_READ: u8 = 1;
const MODE_WRITE: u8 = 2;

struct WalInner<T>
where
    T: Serialize + for<'a> Deserialize<'a>,
{
    mode: AtomicU8,
    writer: Writer,
    _phantom: PhantomData<T>,
}

impl<T> WalInner<T>
where
    T: Serialize + for<'a> Deserialize<'a>,
{
    pub fn new(location: &str, size: usize) -> Self {
        Self {
            mode: AtomicU8::new(MODE_IDLE),
            writer: Writer::new(location, size),
            _phantom: PhantomData,
        }
    }
}

#[derive(Clone)]
pub struct Wal<T>
where
    T: Serialize + for<'a> Deserialize<'a>,
{
    inner: Arc<WalInner<T>>,
}

impl<T> Wal<T>
where
    T: Serialize + for<'a> Deserialize<'a>,
{
    /// Create a new instance of [Wal]
    /// # Arguments
    /// - location: Location where the files shall be stored
    /// - size: Optional, maximum storage size taken by logs in MBs
    pub fn new(location: &str, size: Option<u16>) -> Self {
        let size = size.map(|v| v as usize * 1024).unwrap_or(usize::MAX);
        let inner = WalInner::new(location, size);
        Self {
            inner: Arc::new(inner),
        }
    }

    /// Read last `num` amount of logs
    pub fn read(&self, num: usize) -> Vec<T> {
        todo!()
    }

    /// Read all stored logs
    pub fn read_all(&self) -> Vec<T> {
        todo!()
    }

    /// Write a new log
    pub fn write(&self, item: T) {
        todo!()
    }

    /// Delete all the stored logs
    pub fn purge(&self) {
        todo!()
    }
}
