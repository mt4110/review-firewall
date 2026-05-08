use std::fs;
use std::io;
use std::path::PathBuf;

use serde::Serialize;
use serde::de::DeserializeOwned;

pub fn write_json<T: Serialize>(path: PathBuf, value: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let rendered = serde_json::to_string_pretty(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    fs::write(path, format!("{rendered}\n"))
}

pub fn read_json<T: DeserializeOwned>(path: PathBuf) -> io::Result<Option<T>> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content)
            .map(Some)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub fn write_text(path: PathBuf, value: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let body = if value.ends_with('\n') {
        value.to_owned()
    } else {
        format!("{value}\n")
    };
    fs::write(path, body)
}

pub fn read_text(path: PathBuf) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}
