# Walcraft

Walcraft is a Write Ahead Log (WAL) solution for concurrent environments. The library provides high performance by using
an in-memory buffer and append-only logs.
The logs are stored in multiple files, and older files are deleted to save space.

# Features

- Awesome crate name
- Simple to use and customize
- Configurable storage limit
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
- Configurable buffer size
- Support to skip buffer and directly write to disk
- `fsync` support

# Quirks

- Using enum in the log struct is not supported. The library uses `serde` and `bincode` to serialize and deserialize the
  logs. Enums are not gauranteed to be serialized and deserialized correctly.

The WAL can only be in read mode or write mode, not both at the same time.

- **Ideal**: When created, the WAL is in an idle mode.
- **Read**: Calling `.read()` method switches the WAL to read mode. In this mode, you cannot write data;
  any write attempts will be ignored. Once the reading finishes, the WAL automatically reverts back to ideal mode.
- **Write**: When you start writing to the WAL, it switches to write mode and cannot switch back to ideal or read mode.

This design prevents conflicts between reading and writing. Ideally, you should read the data at startup, as part of the
recovery process, before beginning to write.

**Note:** This behaviour will be fixed in 0.2 update.

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

// recovery: Option A
let all_logs = wal.read().unwrap().into_iter().collect::<Vec<Log> > ();
// recovery: Option B
for log in wal.read().unwrap() {
  // do something with logs 
  dbg!(log);
}

// start writing
wal.write(Log{id: 1, value: 3.14});

```