use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rf_core::domain::LatestPointer;
use time::OffsetDateTime;
use time::macros::format_description;

use crate::io::artifacts;

#[derive(Debug, Clone)]
pub struct RunDirectory {
    pub timestamp: String,
    pub directory: PathBuf,
    pub latest: PathBuf,
}

pub fn create_new(repo_root: &Path) -> io::Result<RunDirectory> {
    let run_root = repo_root.join(".review-firewall").join("run");
    fs::create_dir_all(&run_root)?;
    let (timestamp, directory) = create_unique_run_directory(&run_root)?;
    let run = RunDirectory {
        timestamp,
        directory,
        latest: run_root.join("latest.json"),
    };
    write_latest(&run)?;
    Ok(run)
}

fn create_unique_run_directory(run_root: &Path) -> io::Result<(String, PathBuf)> {
    for _ in 0..5 {
        let timestamp = current_timestamp()?;
        let directory = run_root.join(&timestamp);
        match fs::create_dir(&directory) {
            Ok(()) => return Ok((timestamp, directory)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique run timestamp",
    ))
}

pub fn latest_or_create(repo_root: &Path) -> io::Result<RunDirectory> {
    load_latest(repo_root)?.map_or_else(|| create_new(repo_root), Ok)
}

pub fn load_latest(repo_root: &Path) -> io::Result<Option<RunDirectory>> {
    let run_root = repo_root.join(".review-firewall").join("run");
    let latest = run_root.join("latest.json");
    let pointer = artifacts::read_json::<LatestPointer>(latest.clone())?;
    let Some(pointer) = pointer else {
        return Ok(None);
    };
    if !is_safe_run_timestamp(&pointer.timestamp) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "latest.json contains an unsafe run timestamp",
        ));
    }
    let directory = run_root.join(&pointer.timestamp);
    Ok(Some(RunDirectory {
        timestamp: pointer.timestamp,
        directory,
        latest,
    }))
}

pub fn write_latest(run: &RunDirectory) -> io::Result<()> {
    artifacts::write_json(
        run.latest.clone(),
        &LatestPointer {
            timestamp: run.timestamp.clone(),
        },
    )
}

fn current_timestamp() -> io::Result<String> {
    OffsetDateTime::now_utc()
        .format(format_description!(
            "[year][month][day]T[hour][minute][second].[subsecond digits:9]Z"
        ))
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))
}

fn is_safe_run_timestamp(value: &str) -> bool {
    match value.len() {
        16 => {
            has_digits(value, 0..8)
                && value.as_bytes()[8] == b'T'
                && has_digits(value, 9..15)
                && value.as_bytes()[15] == b'Z'
        }
        26 => {
            has_digits(value, 0..8)
                && value.as_bytes()[8] == b'T'
                && has_digits(value, 9..15)
                && value.as_bytes()[15] == b'.'
                && has_digits(value, 16..25)
                && value.as_bytes()[25] == b'Z'
        }
        _ => false,
    }
}

fn has_digits(value: &str, range: std::ops::Range<usize>) -> bool {
    value
        .as_bytes()
        .get(range)
        .is_some_and(|bytes| bytes.iter().all(u8::is_ascii_digit))
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;

    use super::{create_new, load_latest};

    #[test]
    fn latest_pointer_uses_json() {
        let root = env::temp_dir().join(format!(
            "review-firewall-run-store-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix time")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("temp dir");

        let run = create_new(&root).expect("create run");
        let loaded = load_latest(&root)
            .expect("load latest")
            .expect("latest pointer");

        assert_eq!(run.timestamp, loaded.timestamp);
        assert!(run.latest.exists());
    }

    #[test]
    fn consecutive_runs_do_not_reuse_directory() {
        let root = env::temp_dir().join(format!(
            "review-firewall-run-store-unique-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix time")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("temp dir");

        let first = create_new(&root).expect("first run");
        let second = create_new(&root).expect("second run");

        assert_ne!(first.timestamp, second.timestamp);
        assert_ne!(first.directory, second.directory);
    }

    #[test]
    fn latest_pointer_rejects_path_traversal() {
        let root = env::temp_dir().join(format!(
            "review-firewall-run-store-traversal-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix time")
                .as_nanos()
        ));
        let run_root = root.join(".review-firewall").join("run");
        fs::create_dir_all(&run_root).expect("run dir");
        fs::write(
            run_root.join("latest.json"),
            "{\n  \"timestamp\": \"../outside\"\n}\n",
        )
        .expect("write latest");

        let error = load_latest(&root).expect_err("unsafe latest should error");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn latest_pointer_accepts_legacy_second_precision_timestamp() {
        let root = env::temp_dir().join(format!(
            "review-firewall-run-store-legacy-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix time")
                .as_nanos()
        ));
        let run_root = root.join(".review-firewall").join("run");
        fs::create_dir_all(&run_root).expect("run dir");
        fs::write(
            run_root.join("latest.json"),
            "{\n  \"timestamp\": \"20260328T203500Z\"\n}\n",
        )
        .expect("write latest");

        let loaded = load_latest(&root)
            .expect("load latest")
            .expect("latest pointer");

        assert_eq!(loaded.timestamp, "20260328T203500Z");
        assert_eq!(loaded.directory, run_root.join("20260328T203500Z"));
    }
}
