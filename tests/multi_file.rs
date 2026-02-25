use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use walcraft::{Size, Wal, WalBuilder};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Log {
    seq: usize,
    payload: Vec<u8>,
}

fn build_wal(dir: &str, storage_mb: usize) -> Wal {
    WalBuilder::new()
        .location(dir)
        .page_size(Size::Kb(4))
        .storage_size(Size::Mb(storage_mb))
        .sync_interval(50)
        .build()
        .unwrap()
}

fn wal_files(dir: &str) -> Vec<String> {
    let logs_dir = PathBuf::from(dir).join("logs");
    let mut files: Vec<String> = std::fs::read_dir(&logs_dir)
        .unwrap()
        .filter_map(|entry| {
            let name = entry.unwrap().file_name().into_string().unwrap();
            if name.starts_with("wal_") && name.ends_with(".bin") {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    files.sort();
    files
}

fn parse_file_id(filename: &str) -> u32 {
    // wal_XXXXXXXXXX.bin -> XXXXXXXXXX
    filename[4..filename.len() - 4].parse().unwrap()
}

fn read_meta_segments(dir: &str) -> Vec<toml::Value> {
    let meta_path = PathBuf::from(dir).join("meta.toml");
    let content = std::fs::read_to_string(&meta_path).unwrap();
    let meta: toml::Value = content.parse().unwrap();
    meta["segments"].as_array().unwrap().clone()
}

// Each Log entry with a 500-byte payload serializes to ~518 bytes (including length prefix).
// With 4KB pages (4084 usable bytes), that's ~7 entries per page.
//
// File sizing at 4MB storage:
//   max_file_size ≈ 208KB → 50 pages → ~350 entries per file → ~7000 total capacity (20 files)
// File sizing at 8MB storage:
//   max_file_size ≈ 408KB → 101 pages → ~707 entries per file → ~14K total capacity

/// Write enough data to span multiple files without triggering GC.
/// Verify all entries survive and can be read back in order.
#[test]
fn data_spans_multiple_files() {
    let dir = "./tmp/testing_mf_span";
    std::fs::remove_dir_all(dir).ok();

    // 8MB storage — holds ~14K entries across ~20 files before GC would trigger
    let wal = build_wal(dir, 8);
    let total_entries = 5_000;
    for i in 0..total_entries {
        wal.append_struct(Log {
            seq: i,
            payload: vec![(i % 256) as u8; 500],
        })
        .unwrap();
    }
    wal.flush().unwrap();
    drop(wal);

    // Multiple WAL files should exist on disk
    let files = wal_files(dir);
    assert!(
        files.len() > 1,
        "Expected multiple WAL files, got {}",
        files.len(),
    );

    // Read all entries back — every single one should survive (no GC)
    let wal = build_wal(dir, 8);
    let entries: Vec<Log> = wal
        .iter()
        .unwrap()
        .into_iter()
        .map(|e| e.to_struct::<Log>().unwrap())
        .collect();

    assert_eq!(entries.len(), total_entries);
    for (i, entry) in entries.iter().enumerate() {
        assert_eq!(entry.seq, i, "Wrong seq at position {}", i);
        let expected_byte = (i % 256) as u8;
        assert!(
            entry.payload.iter().all(|&b| b == expected_byte),
            "Payload mismatch at seq {}",
            i,
        );
    }
}

/// Trigger GC with a small storage limit. Verify surviving entries are ordered,
/// intact, and that the most recent entry always survives.
#[test]
fn gc_preserves_recent_entries() {
    let dir = "./tmp/testing_mf_gc";
    std::fs::remove_dir_all(dir).ok();

    // 4MB storage can hold ~7000 entries; writing 20K forces heavy GC
    let storage_mb = 4;
    let wal = build_wal(dir, storage_mb);

    let total_entries = 20_000;
    for i in 0..total_entries {
        wal.append_struct(Log {
            seq: i,
            payload: vec![(i % 256) as u8; 500],
        })
        .unwrap();
    }
    wal.flush().unwrap();
    drop(wal);

    // Read back surviving entries
    let wal = build_wal(dir, storage_mb);
    let entries: Vec<Log> = wal
        .iter()
        .unwrap()
        .into_iter()
        .map(|e| e.to_struct::<Log>().unwrap())
        .collect();

    // Some entries must survive
    assert!(!entries.is_empty(), "Expected surviving entries after GC");
    // GC should have removed older entries
    assert!(
        entries.len() < total_entries,
        "GC should have removed some entries, but all {} survived",
        total_entries,
    );

    // Entries must be in strictly increasing sequence order
    for window in entries.windows(2) {
        assert!(
            window[1].seq > window[0].seq,
            "Entries out of order: seq {} followed by {}",
            window[0].seq,
            window[1].seq,
        );
    }

    // Each surviving entry must have correct payload
    for entry in &entries {
        let expected_byte = (entry.seq % 256) as u8;
        assert_eq!(entry.payload.len(), 500);
        assert!(
            entry.payload.iter().all(|&b| b == expected_byte),
            "Payload mismatch at seq {}",
            entry.seq,
        );
    }

    // The most recent entry should always survive
    assert_eq!(entries.last().unwrap().seq, total_entries - 1);
}

/// Verify files on disk match meta.toml and storage limit is respected after GC.
#[test]
fn files_match_meta_after_gc() {
    let dir = "./tmp/testing_mf_meta";
    std::fs::remove_dir_all(dir).ok();

    let storage_mb = 4;
    let wal = build_wal(dir, storage_mb);

    for i in 0..20_000 {
        wal.append_struct(Log {
            seq: i,
            payload: vec![0xAB; 500],
        })
        .unwrap();
    }
    wal.flush().unwrap();
    drop(wal);

    let segments = read_meta_segments(dir);
    assert!(!segments.is_empty());

    // Every segment in meta must have a corresponding file on disk
    let logs_dir = PathBuf::from(dir).join("logs");
    let tracked_ids: HashSet<u32> = segments
        .iter()
        .map(|s| s["file_id"].as_integer().unwrap() as u32)
        .collect();

    for &id in &tracked_ids {
        let width = u32::MAX.to_string().len();
        let filename = format!("wal_{:0width$}.bin", id, width = width);
        assert!(
            logs_dir.join(&filename).exists(),
            "Segment {} in meta.toml has no file on disk",
            id,
        );
    }

    // No orphan WAL files (every file on disk must be tracked in meta)
    for file in wal_files(dir) {
        let id = parse_file_id(&file);
        assert!(
            tracked_ids.contains(&id),
            "Orphan file {} not tracked in meta.toml",
            file,
        );
    }

    // Total tracked size must not exceed the storage limit
    let total_size: usize = segments
        .iter()
        .map(|s| s["file_size"].as_integer().unwrap() as usize)
        .sum();
    let storage_limit = storage_mb * 1024 * 1024;
    assert!(
        total_size <= storage_limit,
        "Total storage {} bytes exceeds limit {} bytes",
        total_size,
        storage_limit,
    );

    // GC must have run — first segment ID should be > 1 (older files deleted)
    let first_id = segments[0]["file_id"].as_integer().unwrap();
    assert!(
        first_id > 1,
        "GC should have removed oldest files, but first segment ID is {}",
        first_id,
    );
}
