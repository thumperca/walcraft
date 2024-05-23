use super::PAGE_SIZE;

pub(crate) struct Buffer {
    inner: Vec<u8>,
    // checksum: u32 <- for future use
}

impl Buffer {
    /// Create a new buffer
    ///
    /// ## Returns
    /// A new empty buffer with size of [PAGE_SIZE]
    ///
    pub fn new() -> Self {
        Self {
            inner: Vec::with_capacity(PAGE_SIZE),
        }
    }

    /// Create a buffer of custom size
    ///
    /// #### Note
    /// This method is only called when the previous buffer gets misaligned from [PAGE_SIZE] and the
    /// new buffer shall be either longer or shorter than the [PAGE_SIZE] in order to re-align again
    ///
    /// ## Arguments
    /// - `size`: The size of new buffer in bytes
    ///
    /// ## Returns
    /// A new empty buffer of provided size
    ///
    pub fn with_size(_size: usize) -> Self {
        todo!()
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
        if self.inner.len() >= PAGE_SIZE {
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
        (true, self.inner.len() >= PAGE_SIZE)
    }

    /// Add new data to buffer
    ///
    /// If enough space is not available, then this method will extend the size of the buffer beyond [PAGE_SIZE]
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
        if padding && self.inner.len() < PAGE_SIZE {
            let diff = PAGE_SIZE - self.inner.len();
            let v = (0..diff).map(|_| 0).collect::<Vec<_>>();
            self.inner.extend(v);
        }
        self.inner
    }

    pub fn inner(&self) -> &[u8] {
        &self.inner
    }
}
