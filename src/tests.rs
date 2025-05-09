use std::path::PathBuf;

const TESTING_DIR: &str = "./tmp/testing";

/// Utility function to remove the testing directory and create a new blank one for a new test
pub(crate) fn clean_test_dir() {
    let _ = std::fs::remove_dir_all(crate::TESTING_DIR);
    let mut path = PathBuf::from(crate::TESTING_DIR);
    path.push("logs");
    std::fs::create_dir_all(path).unwrap();
}
