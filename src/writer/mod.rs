mod buffer;

use self::buffer::Buffer;

pub const PAGE_SIZE: usize = 4096; // 4 KB

struct Writer {
    buffer: Buffer
}

impl Writer {
    // pub fn log(&self, msg: )
}