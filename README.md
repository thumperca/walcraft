# Walcraft

Walcraft is a Write Ahead Log (WAL) solution for concurrent environments. The library provides high performance by using
an in-memory buffer and append-only logs.
The logs are stored in multiple files, and older files are deleted to save space.

# Features

- Awesome crate name
- Simple to use and customize
- Configurable storage limit
- Configurable buffer size
- fsync support
- High write throughput
- Built for concurrent and parallel environments
- Prevents write amplification for high frequency writes

# How

The crate uses an in-memory buffer, whose size can be adjusted, with default size of 4 KB.
The data is first added to the buffer and written to the disk when the buffer becomes full.

For high frequency environments, this setup ensures higher write throughput by batching several smaller updates
to a few large updates, preventing unnecessary wear and tear on the SSD due
to [Write Amplification](https://en.wikipedia.org/wiki/Write_amplification).

For low frequency environments, the library provides a way to flush the changes early
before the buffer is completely filled (by using `.flush()` method), ensuring higher guarantee of log recoverability.

# Initialization

### Builder Pattern (Recommended)

The builder pattern allows for complete customization of the WAL instance.

```
use walcraft::{Size, WalBuilder, Wal};

// create a wal with 4 KB buffer and 10 GB storage
let wal: Wal<String> = WalBuilder::new()
  .location("/tmp/logs/wal")
  .buffer_size(Size::Kb(4))
  .storage_size(Size::Gb(10))
  .build()
  .unwrap();

// create a wal with no buffer, enable fsync and use 250 MB of storage
let wal: Wal<String> = WalBuilder::new()
  .location("/tmp/logs/wal")
  .storage_size(Size::Mb(250))
  .disable_buffer()
  .enable_fsync()
  .build()
  .unwrap();
```

### Direct Initialization

This method only allows you to set location and storage size (in MBs) only.
The buffer size is set to 4 KB by default and fsync is disabled.

```
use walcraft::Wal;

// Create a wal instance with 200 MB of storage
let wal = Wal::new("/tmp/logs/wal", Some(200));
```

# Usage

### Writing logs

```
use serde::{Deserialize, Serialize};
use walcraft::Wal;

// Log to write
#[derive(Serialize, Deserialize, Clone)]
struct Log {
    id: usize,
    value: f64
}

let log = Log {id: 1, value: 5.6234};

// initiate wal and add a log
let wal = Wal::new("./tmp/", None);
wal.write(log); // write a log

// write a log in another thread
let wal2 = wal.clone();
std::thread::spawn(move | | {
let log = Log{id: 2, value: 0.45};
  wal2.write(log);
});

// keep writing logs in current thread
let log = Log{id: 3, value: 123.59};
wal.write(log);

// Flush the logs to the disk manually
// This happens automatically as well after some time. However, it's advised to
// run this method before terminating the program to ensure that no logs are lost.
wal.flush();
```

### Reading logs

```
use serde::{Deserialize, Serialize};
use walcraft::Wal;

// Log to write
#[derive(Serialize, Deserialize, Debug)]
struct Log {
    id: usize,
    value: f64
}
let wal: Wal<Log> = Wal::new("./tmp/", None);
let iterator = wal.read().unwrap();

for log in iterator {
    dbg!(log);
}
```

### Limiting the size of logs

`Wal::new` method accepts 2 arguments. The first argument is the directory where logs will be stored.
The second (optional) argument is for the preferred storage that logs shall occupy in MBs.

Once the storage occupied by log files exceed the provided limit, the older logs are deleted in chunks
to free up some space.

```
// Unlimited log storage
let wal = Wal::new("/tmp/logz", None);

// 500 MB of logs storage
let wal = Wal::new("/tmp/logz", Some(500));

// 20 GB of logs storage
let wal = Wal::new("/tmp/logz", Some(20_000));
```

# Upcoming features

- Concurrent reads and writes
- Support for JSON & CSV log formats

# Useful tips

- **Buffer size**: The buffer size can be adjusted to suit your needs. A larger buffer size will reduce the number of
  writes to the disk, but it will also increase the memory usage.
- **Storage size**: The storage size can be adjusted to limit the amount of space the logs can occupy. Once the limit is
  reached, the older logs are deleted to free up space.
- **Fsync**: By default, fsync is disabled. You can enable it by using the builder pattern. Enabling fsync will ensure
  that the data is written to the disk before returning from the write operation. This will ensure that the data is
  not lost in case of a power failure. However, this method reduces the amount of writes per second significantly.
- **Recovery**: The library provides a way to recover the logs at startup. You can read the logs using the `.read()`
  method. This method returns an iterator that you can use to read the logs. Calling this method after writing starts,
  results in error return.
- **Flush**: The library automatically flushes the logs to the disk once the buffer is filled. However, it's advised
  to run the `.flush()` method before terminating the program to ensure that no logs are lost.

# Quirks

The WAL can only be in read mode or write mode, not both at the same time.

- **Ideal**: When created, the WAL is in an idle mode.
- **Read**: Calling `.read()` method switches the WAL to read mode. In this mode, you cannot write data;
  any write attempts will be ignored. Once the reading finishes, the WAL automatically reverts back to ideal mode.
- **Write**: When you start writing to the WAL, it switches to write mode and cannot switch back to ideal or read mode.

This design prevents conflicts between reading and writing. Ideally, you should read the data at startup, as part of the
recovery process, before beginning to write.

**Note:** This behaviour will be fixed in a future update.

```
use serde::{Deserialize, Serialize};
use walcraft::Wal;

// Log to write
#[derive(Serialize, Deserialize, Clone)]
struct Log {
    id: usize,
    value: f64
}

// create an instance of WAL
let wal = Wal::new("/tmp/logz", Some(2000));

// recovery: Option A (read all data at once)
// This method reads all the data at once and shall only be used 
// if all the logs, depending on storage size, can fit in the memory
let all_logs = wal.read().unwrap().into_iter().collect::<Vec<Log>>();

// recovery: Option B
// This method reads data in chunks of 8 KB. 
// It is useful when you have a large number of logs
for log in wal.read().unwrap() {
  // do something with logs 
  dbg!(log);
}

// start writing
wal.write(Log{id: 1, value: 3.14});

```

# Known issues

- **Enum support**: Using enum in the log struct is not supported.
  The library uses `serde` and `bincode` to serialize and deserialize the logs.
  Enums are not guaranteed to be serialized and deserialized correctly.
  A workaround this limitation is to convert the enum field to string with serde_json
  and store it as string in logs struct.