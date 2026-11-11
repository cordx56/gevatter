use crate::json;
use bridge::TyCtxt;
use bridge::hir::def_id::LOCAL_CRATE;
use models::CrateReport;
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
        json::load(&self.path).unwrap_or_else(|e| {
            tracing::warn!("failed to load report {:?}: {e:?}", self.path);
            self.empty()
        })
    }

    pub fn store(&self, report: &CrateReport) {
        if let Err(e) = json::store(&self.path, report) {
            tracing::error!("failed to store report {:?}: {e:?}", self.path);
        }
    }
}
