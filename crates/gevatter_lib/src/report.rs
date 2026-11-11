use crate::hir::def_id::LOCAL_CRATE;
use crate::middle::ty::TyCtxt;
use models::CrateReport;
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct ReportFile {
    path: PathBuf,
    krate: String,
}
impl ReportFile {
    pub fn of(tcx: TyCtxt<'_>) -> Self {
        Self {
            path: editor::files::report_path(),
            krate: tcx.crate_name(LOCAL_CRATE).to_string(),
        }
    }

    pub fn empty(&self) -> CrateReport {
        CrateReport::new(self.krate.clone())
    }

    pub fn load(&self) -> CrateReport {
        fs::read(&self.path)
            .and_then(|bytes| serde_json::from_slice(&bytes).map_err(io::Error::from))
            .unwrap_or_else(|e| {
                tracing::warn!("failed to load report {:?}: {e:?}", self.path);
                self.empty()
            })
    }

    pub fn store(&self, report: &CrateReport) {
        let stored = self
            .path
            .parent()
            .map_or(Ok(()), fs::create_dir_all)
            .and_then(|()| serde_json::to_vec_pretty(report).map_err(io::Error::from))
            .and_then(|bytes| fs::write(&self.path, bytes));
        if let Err(e) = stored {
            tracing::error!("failed to store report {:?}: {e:?}", self.path);
        }
    }
}
