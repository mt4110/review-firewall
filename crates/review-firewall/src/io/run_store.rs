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
    let timestamp = current_timestamp()?;
    let directory = run_root.join(&timestamp);
    fs::create_dir_all(&directory)?;
    let run = RunDirectory {
        timestamp,
        directory,
        latest: run_root.join("latest.json"),
    };
    write_latest(&run)?;
    Ok(run)
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
            "[year][month][day]T[hour][minute][second]Z"
        ))
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))
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
}
