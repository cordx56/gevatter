use serde::{Serialize, de::DeserializeOwned};
use std::fs;
use std::io;
use std::path::Path;

pub fn load<T: DeserializeOwned>(path: &Path) -> io::Result<T> {
    fs::read(path).and_then(|bytes| serde_json::from_slice(&bytes).map_err(io::Error::from))
}

pub fn store<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    path.parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| serde_json::to_vec_pretty(value).map_err(io::Error::from))
        .and_then(|bytes| fs::write(path, bytes))
}
