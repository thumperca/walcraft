use serde::{Deserialize, Serialize};
use walcraft::{Size, Wal, WalBuilder};

type Id = usize;

#[derive(Serialize, Deserialize, Debug)]
struct Message {
    id: Id,
    group: Id,
    data: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
enum Log {
    Accept(Message),
    Done(Id),
}

const TESTING_DIR: &str = "./tmp/testing";

fn build_wal() -> Wal {
    WalBuilder::new()
        .location(TESTING_DIR)
        .page_size(Size::Kb(4))
        .storage_size(Size::Mb(24))
        .build()
        .unwrap()
}

#[test]
fn test_enum() {
    // reset test location
    std::fs::remove_dir_all(TESTING_DIR).ok();
    let wal = build_wal();

    let mut handles = vec![];

    // add 1000 Log::Accept
    let wal_clone = wal.clone();
    let accept_handle = std::thread::spawn(move || {
        let mut counter = 1;
        for i in 0..100 {
            for j in 0..10 {
                wal_clone
                    .append_struct(Log::Accept(Message {
                        id: counter,
                        group: i,
                        data: Vec::from([j; 10]),
                    }))
                    .expect("Failed to add message to WAL");
                counter += 1;
            }
        }
    });
    handles.push(accept_handle);

    // add 100 Log::Done
    let done_handle = std::thread::spawn(move || {
        for i in 0..100 {
            wal.append_struct(Log::Done(i))
                .expect("Failed to add message to WAL");
        }
    });
    handles.push(done_handle);

    // wait until threads are done
    for handle in handles {
        handle.join().unwrap();
    }

    // read logs back
    let wal = build_wal();
    let reader = wal.iter().expect("Failed to get wal");
    let logs = reader.collect::<Vec<_>>();
    assert_eq!(logs.len(), 1100);

    // test division of logs
    let mut counter_a = 0;
    let mut counter_b = 0;
    for log in logs {
        let msg = log.to_struct::<Log>().expect("Failed to deserialize msg");
        match msg {
            Log::Accept(_) => {
                counter_a += 1;
            }
            Log::Done(_) => {
                counter_b += 1;
            }
        }
    }
    assert_eq!(counter_a, 1000);
    assert_eq!(counter_b, 100);
}
