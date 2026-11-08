use std::env;
use std::path::PathBuf;

pub fn target_dir() -> PathBuf {
    use cargo_metadata::MetadataCommand;

    if let Ok(v) = env::var("CARGO_TARGET_DIR") {
        PathBuf::from(v)
    } else if let Ok(meta) = MetadataCommand::new().exec() {
        meta.target_directory.as_std_path().to_path_buf()
    } else {
        tracing::warn!("failed to read target directory; fallback to ./target");
        PathBuf::from("./target")
    }
    .join("gevatter")
}
