use super::PAGE_SIZE;

pub(crate) struct Buffer {
    inner: [u8; PAGE_SIZE],
    pointer: usize
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            inner: [0; PAGE_SIZE],
            pointer: 0
        }
    }

    pub fn add(&mut self, data: &[u8]) -> bool {
        let new_pointer = self.pointer + data.len();
        // check if buffer has enough space
        if new_pointer >= PAGE_SIZE {
            return false;
        }
        for item in data {
            self.inner[self.pointer] = *item;
            self.pointer += 1;
        }
        true
    }
}