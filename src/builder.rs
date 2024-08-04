use crate::{Size, Wal};
use serde::{Deserialize, Serialize};

/// Build [Wal] with custom configuration
/// It uses a builder pattern and methods can be chained
///
/// ### Example
/// ```no_run
/// use walcraft::{Size, WalBuilder};
/// // create a wal with 4 KB buffer and 10 GB storage
/// let wal = WalBuilder::new().buffer_size(Size::Kb(4)).storage_size(Size::Gb(10)).build();
/// // crate a wal with no buffer and 250 MB storage
/// let wal = WalBuilder::new().storage_size(Size::Mb(250)).disable_buffer().build();
/// ```
pub struct WalBuilder {
    buffer_enabled: bool,
    buffer_size: Option<Size>,
    storage_size: Option<Size>,
}

impl WalBuilder {
    pub fn new() -> Self {
        Self {
            buffer_enabled: true,
            buffer_size: None,
            storage_size: None,
        }
    }

    pub fn disable_buffer(mut self) -> Self {
        self.buffer_enabled = false;
        self
    }

    pub fn buffer_size(mut self, size: Size) -> Self {
        self.buffer_size = Some(size);
        self
    }

    pub fn storage_size(mut self, size: Size) -> Self {
        self.storage_size = Some(size);
        self
    }

    pub fn build<T>(mut self) -> Wal<T>
    where
        T: Serialize + for<'a> Deserialize<'a>,
    {
        todo!()
    }
}
