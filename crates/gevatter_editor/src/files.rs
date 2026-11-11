use bridge::{
    TyCtxt,
    hir::def_id::LOCAL_CRATE,
    span::source_map::{FileLoader, RealFileLoader},
};

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, OnceLock};

pub static CARGO_METADATA: LazyLock<Option<cargo_metadata::Metadata>> =
    LazyLock::new(|| cargo_metadata::MetadataCommand::new().exec().ok());
pub static WORKSPACE_ROOT: LazyLock<PathBuf> = LazyLock::new(|| {
    if let Some(path) = CARGO_METADATA
        .as_ref()
        .map(|v| fs::canonicalize(v.workspace_root.as_std_path().to_path_buf()).ok())
        .flatten()
    {
        path
    } else {
        env::current_dir().unwrap()
    }
});
pub static TARGET_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    if let Some(v) = env::var("CARGO_TARGET_DIR")
        .ok()
        .map(|v| fs::canonicalize(v).ok())
        .flatten()
    {
        v
    } else if let Some(path) = CARGO_METADATA
        .as_ref()
        .map(|v| fs::canonicalize(v.target_directory.as_std_path().to_path_buf()).ok())
        .flatten()
    {
        path
    } else {
        tracing::warn!("failed to read target directory; fallback to ./target");
        WORKSPACE_ROOT.join("target")
    }
});

static UNIT: OnceLock<String> = OnceLock::new();
fn ensure_unit_path(crate_name: &str, extra_filename: &str) {
    UNIT.get_or_init(|| format!("{crate_name}{extra_filename}"));
}

fn unit_out_dir() -> PathBuf {
    TARGET_DIR
        .join("gevatter")
        .join(UNIT.get().expect("temporary file path is not initialized"))
}
pub fn edit_out_dir() -> PathBuf {
    unit_out_dir().join("source")
}
pub fn report_path() -> PathBuf {
    unit_out_dir().join("report.json")
}
pub fn mirror_of(path: &Path) -> io::Result<Option<(PathBuf, PathBuf)>> {
    let original = fs::canonicalize(path)?;
    if original.starts_with(&*TARGET_DIR) {
        return Ok(None);
    }
    Ok(original
        .strip_prefix(&*WORKSPACE_ROOT)
        .ok()
        .map(|rel| (original.clone(), edit_out_dir().join(rel))))
}
fn ensure_mirror(path: &Path) -> io::Result<Option<PathBuf>> {
    let Some((original, mirror)) = mirror_of(path)? else {
        return Ok(None);
    };
    if !mirror.is_file() {
        let parent = mirror.parent().expect("a mirror lies below the out dir");
        fs::create_dir_all(parent)
            .inspect_err(|e| tracing::error!("failed to create out dir {parent:?}: {e:?}"))?;
        fs::copy(&original, &mirror)
            .inspect_err(|e| tracing::error!("failed to copy {original:?} to {mirror:?}: {e:?}"))?;
    }
    Ok(Some(mirror))
}

pub struct TemporaryFileLoader;
impl TemporaryFileLoader {
    pub fn ensure_unit_path(conf: &bridge::interface::Config) -> PathBuf {
        let krate = conf
            .opts
            .crate_name
            .as_deref()
            .unwrap_or_else(|| conf.input.filestem());
        let extra_filename = &conf.opts.cg.extra_filename;
        ensure_unit_path(&krate, extra_filename);
        unit_out_dir()
    }
    pub fn ensure_temp_refresh() -> PathBuf {
        let out_dir = unit_out_dir();
        let _ = fs::remove_dir_all(&out_dir);
        out_dir
    }
    pub fn write_file(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let mirror = ensure_mirror(path)?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{path:?} is outside the workspace or a build product"),
            )
        })?;
        fs::write(&mirror, bytes)
            .inspect_err(|e| tracing::error!("failed to write file {mirror:?}: {e:?}"))
    }
}
impl FileLoader for TemporaryFileLoader {
    fn file_exists(&self, path: &Path) -> bool {
        RealFileLoader.file_exists(path)
    }
    fn read_file(&self, path: &Path) -> io::Result<String> {
        match ensure_mirror(path)? {
            Some(mirror) => RealFileLoader.read_file(&mirror),
            None => RealFileLoader.read_file(path),
        }
    }
    fn read_binary_file(&self, path: &Path) -> io::Result<Arc<[u8]>> {
        RealFileLoader.read_binary_file(path)
    }
    fn current_directory(&self) -> io::Result<PathBuf> {
        RealFileLoader.current_directory()
    }
}
