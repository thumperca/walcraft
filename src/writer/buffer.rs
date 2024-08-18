use crate::DEFAULT_BUFFER_SIZE;

pub(crate) struct Buffer {
    size: usize,
    inner: Vec<u8>,
    // checksum: u32 <- for future use
}

impl Buffer {
    /// Create a buffer of given size
    ///
    /// ## Arguments
    /// - `size`: The size of new buffer in bytes
    ///
    /// ## Returns
    /// A new empty buffer of provided size
    ///
    pub fn new(size: Option<usize>) -> Self {
        let size = size.unwrap_or(DEFAULT_BUFFER_SIZE);
        Self {
            inner: Vec::with_capacity(size),
            size,
        }
    }

    /// Add data to buffer
    ///
    /// ## Returns
    /// Tuple of 2 boolean where
    /// - 0: Whether the new data was accepted to the buffer or not
    /// - 1: Whether the buffer is ready to be flushed or not
    ///
    pub fn try_add(&mut self, data: &[u8]) -> (bool, bool) {
        // check for empty addition
        if data.is_empty() {
            return (true, false);
        }
        // check whether the buffer isn't already filled
        if self.inner.len() >= self.size {
            return (false, true);
        }

        // Note: uncomment the code below to ensure alignment of buffer to PAGE_SIZE
        // check if the data shall be accepted or not
        // It can be rejected if there isn't enough space for small payloads
        // let new_pointer = self.inner.len() + data.len() + 2;
        // if data.len() < (PAGE_SIZE / 4) && new_pointer > PAGE_SIZE {
        //     return (false, true);
        // }

        // add to buffer & return accepted status
        self.add(data);
        (true, self.inner.len() >= self.size)
    }

    /// Add new data to buffer
    ///
    /// If enough space is not available, then this method will
    /// extend the size of the buffer beyond [PAGE_SIZE]
    fn add(&mut self, data: &[u8]) {
        // store length
        let size: [u8; 2] = (data.len() as u16).to_ne_bytes();
        self.inner.extend(&size);
        // store data
        self.inner.extend(data);
    }

    /// Consume the buffer to return the inner data for dumping to file
    ///
    /// ## Argument
    /// - `padding` - Whether the inner data shall be padded to [PAGE_SIZE] or not
    ///
    /// ## Returns
    /// The internal contents of the buffer
    pub fn consume(mut self, padding: bool) -> Vec<u8> {
        if padding && self.inner.len() < self.size {
            let diff = self.size - self.inner.len();
            let v = (0..diff).map(|_| 0).collect::<Vec<_>>();
            self.inner.extend(v);
        }
        self.inner
    }

    pub fn inner(&self) -> &[u8] {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_size() {
        let buffer = Buffer::new(None);
        assert_eq!(buffer.size, DEFAULT_BUFFER_SIZE);
    }

    #[test]
    fn consume() {
        let mut buffer = Buffer::new(None);
        let data = [20; 100];
        buffer.add(&data);
        let data = buffer.consume(false);
        assert_eq!(data.len(), 102); // 2 extra bytes are for representation of length of 1 added item to buffer
    }

    #[test]
    fn consume_padding() {
        let mut buffer = Buffer::new(None);
        let data = [10; 100];
        buffer.add(&data);
        let data = buffer.consume(true);
        assert_eq!(data.len(), DEFAULT_BUFFER_SIZE);
    }

    #[test]
    fn try_add() {
        let mut buffer = Buffer::new(Some(120));
        let data = [10; 100];
        let d = buffer.try_add(&data);
        assert_eq!(d, (true, false));
        let data = [10; 100];
        let d = buffer.try_add(&data);
        assert_eq!(d, (true, true));
    }

    #[test]
    fn reject_on_add() {
        let mut buffer = Buffer::new(Some(120));
        // first larger than buffer size payload
        let data = [10; 140];
        let d = buffer.try_add(&data);
        assert_eq!(d, (true, true));
        // extending the existing buffer will fail now
        let data = [10; 20];
        let d = buffer.try_add(&data);
        assert_eq!(d, (false, true));
    }
}
