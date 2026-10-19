//! Desktop consent and immutable native export workers. Never mutates the project.
use super::{AssetId, AssetRecord, CanvasDocument, CanvasObjectKind, PlotiumApp, ProjectContextId};
use crate::{
    automation::TargetRef,
    export::{self, ExportAuthorization, PreparedFigureExport},
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::PathBuf,
    thread::JoinHandle,
};
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExportStatus {
    AwaitingAuthorization,
    Rendering,
    Completed,
}
#[derive(Clone, Debug, Serialize)]
pub struct ExportFileReceipt {
    pub format: String,
    pub path: PathBuf,
    pub overwrite_required: bool,
    pub status: String,
    pub error_code: Option<String>,
    pub message: String,
    pub bytes: Option<usize>,
}
#[derive(Clone, Debug, Serialize)]
pub struct ExportReceipt {
    pub export_id: String,
    pub project_context: ProjectContextId,
    pub render_revision: u64,
    pub canvas: TargetRef,
    pub size_mm: [f32; 2],
    pub dpi: u16,
    pub png_dimensions: [u32; 2],
    pub status: ExportStatus,
    pub files: Vec<ExportFileReceipt>,
}
struct Request {
    receipt: ExportReceipt,
    prepared: PreparedFigureExport,
    permission: ExportAuthorization,
}
struct Flight {
    id: String,
    handle: JoinHandle<Vec<ExportFileReceipt>>,
}
#[derive(Default)]
pub struct ExternalExports {
    requests: VecDeque<Request>,
    flight: Option<Flight>,
}
impl ExternalExports {
    pub fn receipt(&self, id: &str) -> Option<&ExportReceipt> {
        self.requests
            .iter()
            .find(|r| r.receipt.export_id == id)
            .map(|r| &r.receipt)
    }
    pub fn pending(&self) -> Option<&ExportReceipt> {
        self.requests
            .iter()
            .find(|r| {
                r.receipt.status == ExportStatus::AwaitingAuthorization
                    && !r.permission.is_revoked()
            })
            .map(|r| &r.receipt)
    }
    pub fn busy(&self) -> bool {
        self.flight.is_some()
            || self
                .requests
                .iter()
                .any(|r| r.receipt.status != ExportStatus::Completed)
    }
    pub fn revoke(&mut self) {
        for r in &mut self.requests {
            r.permission.revoke();
            if r.receipt.status == ExportStatus::AwaitingAuthorization {
                deny(
                    &mut r.receipt,
                    "request_cancelled",
                    "Connection, permissions or project changed; no output installed.",
                );
            }
        }
    }
    pub fn reconcile(&mut self) {
        for r in &mut self.requests {
            if r.permission.is_revoked() && r.receipt.status == ExportStatus::AwaitingAuthorization
            {
                deny(
                    &mut r.receipt,
                    "request_cancelled",
                    "Export permission revoked; no output installed.",
                );
            }
        }
    }
}
fn deny(receipt: &mut ExportReceipt, code: &str, message: &str) {
    receipt.status = ExportStatus::Completed;
    for f in &mut receipt.files {
        if f.status == "pending" {
            f.status = "failed".into();
            f.error_code = Some(code.into());
            f.message = message.into();
        }
    }
}
impl PlotiumApp {
    pub fn request_external_export(
        &mut self,
        id: String,
        prepared: PreparedFigureExport,
        revision: u64,
    ) -> Result<ExportAuthorization, String> {
        self.session.external_exports.reconcile();
        if revision != self.doc.automation_revision || self.session.ui.project_transition.is_some()
        {
            return Err("Project changed; rediscover and replan".into());
        }
        if self.session.external_exports.busy() {
            return Err("Wait for the pending export".into());
        }
        let permission = ExportAuthorization::default();
        let files = prepared
            .parameters
            .outputs
            .iter()
            .zip(&prepared.outputs)
            .map(|(o, t)| ExportFileReceipt {
                format: o.format.clone(),
                path: t
                    .as_ref()
                    .map(|t| t.path.clone())
                    .unwrap_or_else(|_| o.path.clone()),
                overwrite_required: t.as_ref().is_ok_and(|t| t.exists()),
                status: if t.is_ok() { "pending" } else { "failed" }.into(),
                error_code: t.as_ref().err().map(|e| {
                    if e.starts_with("overwrite_not_requested") {
                        "overwrite_not_requested"
                    } else {
                        "invalid_output"
                    }
                    .into()
                }),
                message: t.as_ref().err().cloned().unwrap_or_else(|| {
                    "Awaiting separate desktop export authorization; no file written.".into()
                }),
                bytes: None,
            })
            .collect::<Vec<_>>();
        let status = if files.iter().all(|f| f.status == "failed") {
            permission.revoke();
            ExportStatus::Completed
        } else {
            ExportStatus::AwaitingAuthorization
        };
        while self.session.external_exports.requests.len() >= 16 {
            self.session.external_exports.requests.pop_front();
        }
        self.session.external_exports.requests.push_back(Request {
            receipt: ExportReceipt {
                export_id: id,
                project_context: self.session.project_context,
                render_revision: revision,
                canvas: prepared.parameters.canvas.clone(),
                size_mm: prepared.size_mm,
                dpi: prepared.parameters.dpi,
                png_dimensions: prepared.pixels,
                status,
                files,
            },
            prepared,
            permission: permission.clone(),
        });
        if let Some(receipt) = self
            .session
            .external_exports
            .requests
            .back()
            .map(|r| &r.receipt)
            && receipt.status == ExportStatus::Completed
            && let Some(failure) = receipt.files.iter().find(|f| f.status == "failed")
        {
            self.session.status = format!("Figure export: {}", failure.message);
        }
        Ok(permission)
    }
    pub fn decide_external_export(&mut self, id: &str, allow: bool, replace: bool) {
        let Some(index) = self
            .session
            .external_exports
            .requests
            .iter()
            .position(|r| r.receipt.export_id == id)
        else {
            return;
        };
        let r = &self.session.external_exports.requests[index];
        if r.receipt.status != ExportStatus::AwaitingAuthorization {
            return;
        }
        let error = if !allow {
            Some((
                "permission_denied",
                "Export denied in Plotium; no output installed.".to_owned(),
            ))
        } else if r.permission.is_revoked() {
            Some((
                "request_cancelled",
                "Export permission revoked; no output installed.".into(),
            ))
        } else if r.receipt.project_context != self.session.project_context
            || r.receipt.render_revision != self.doc.automation_revision
        {
            Some((
                "stale_revision",
                "Project changed before authorization; rediscover and replan.".into(),
            ))
        } else if r
            .receipt
            .files
            .iter()
            .any(|f| f.status == "pending" && f.overwrite_required)
            && !replace
        {
            Some((
                "permission_denied",
                "Confirm replacement of every listed existing output.".into(),
            ))
        } else {
            None
        };
        if let Some((code, message)) = error {
            let r = &mut self.session.external_exports.requests[index];
            r.permission.revoke();
            deny(&mut r.receipt, code, &message);
            self.session.status = format!("Figure export: {message}");
            return;
        }
        if !r.permission.grant() {
            let r = &mut self.session.external_exports.requests[index];
            deny(
                &mut r.receipt,
                "request_cancelled",
                "Export authorization was already used or revoked.",
            );
            return;
        }
        let capture = self.capture_export(&r.prepared);
        let r = &mut self.session.external_exports.requests[index];
        let (canvas, assets) = match capture {
            Ok(v) => v,
            Err(e) => {
                r.permission.revoke();
                deny(&mut r.receipt, "export_unavailable", &e);
                self.session.status = format!("Figure export unavailable: {e}");
                return;
            }
        };
        let prepared = r.prepared.clone();
        let files = r.receipt.files.clone();
        let permission = r.permission.clone();
        match std::thread::Builder::new()
            .name("plotium-export".into())
            .spawn(move || run_export(canvas, assets, prepared, files, permission))
        {
            Ok(handle) => {
                r.receipt.status = ExportStatus::Rendering;
                self.session.external_exports.flight = Some(Flight {
                    id: id.into(),
                    handle,
                });
            }
            Err(e) => {
                r.permission.revoke();
                deny(&mut r.receipt, "worker_failed", &e.to_string());
                self.session.status = format!("Figure export worker unavailable: {e}");
            }
        }
    }
    fn capture_export(
        &self,
        p: &PreparedFigureExport,
    ) -> Result<(CanvasDocument, BTreeMap<AssetId, AssetRecord>), String> {
        let c = self
            .doc
            .canvases
            .iter()
            .find(|c| TargetRef::resource(c.resource_id.into()) == p.parameters.canvas)
            .ok_or("Canvas disappeared")?;
        for o in &c.objects {
            if let Some(plot) = o.plot()
                && plot.binding.coordinates.is_some()
            {
                self.build_explicit_binding_figure(
                    &plot.binding,
                    &plot.stack,
                    [
                        o.frame.width / super::MM_TO_PT,
                        o.frame.height / super::MM_TO_PT,
                    ],
                )
                .map_err(|e| format!("Explicit curve unavailable: {e}"))?;
            }
        }
        let ids = c
            .objects
            .iter()
            .filter_map(|o| match &o.kind {
                CanvasObjectKind::RasterImage(i) => Some(i.asset),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let mut assets = BTreeMap::new();
        let mut bytes = 0usize;
        for id in ids {
            let a = self.doc.assets.get(&id).ok_or_else(|| {
                format!("Image asset {id} is missing; replace it before exporting")
            })?;
            bytes = bytes.saturating_add(a.bytes.len());
            if bytes > 16 * 1024 * 1024 {
                return Err("Export referenced assets exceed 16 MiB".into());
            }
            assets.insert(id, a.clone());
        }
        Ok((c.clone(), assets))
    }
    /// Called by the shell even when the window is hidden. Old-project receipts
    /// stay truthful, but their completion never updates a new project's status.
    pub fn poll_external_export(&mut self) -> bool {
        self.session.external_exports.reconcile();
        if !self
            .session
            .external_exports
            .flight
            .as_ref()
            .is_some_and(|f| f.handle.is_finished())
        {
            return self.session.external_exports.flight.is_some();
        }
        let flight = self
            .session
            .external_exports
            .flight
            .take()
            .expect("finished export flight");
        let Some(r) = self
            .session
            .external_exports
            .requests
            .iter_mut()
            .find(|r| r.receipt.export_id == flight.id)
        else {
            return false;
        };
        match flight.handle.join() {
            Ok(files) => r.receipt.files = files,
            Err(_) => {
                for f in &mut r.receipt.files {
                    if f.status == "pending" {
                        f.status = "unknown".into();
                        f.error_code = Some("worker_failed".into());
                        f.message="Worker failed; inspect this exact output. Do not replay an unknown export.".into();
                    }
                }
            }
        }
        // Joining ends every installation, including unknown outcomes after a
        // panic. Retained receipts no longer need a live connection permit.
        r.permission.revoke();
        r.receipt.status = ExportStatus::Completed;
        if r.receipt.project_context == self.session.project_context {
            let n = r
                .receipt
                .files
                .iter()
                .filter(|f| f.status == "succeeded")
                .count();
            let detail = r
                .receipt
                .files
                .iter()
                .find(|f| f.status != "succeeded")
                .map(|f| format!(" {}: {}", f.path.display(), f.message))
                .unwrap_or_default();
            self.session.status = format!(
                "Figure export: {n}/{} files installed.{detail}",
                r.receipt.files.len()
            );
        }
        false
    }
}
fn run_export(
    canvas: CanvasDocument,
    assets: BTreeMap<AssetId, AssetRecord>,
    prepared: PreparedFigureExport,
    mut files: Vec<ExportFileReceipt>,
    permission: ExportAuthorization,
) -> Vec<ExportFileReceipt> {
    for (file, target) in files.iter_mut().zip(&prepared.outputs) {
        let Ok(target) = target else {
            continue;
        };
        let result = (|| {
            if permission.is_revoked() {
                return Err("Export authorization revoked; this output was not installed".into());
            }
            let bytes = export::encode_figure_export(
                &canvas,
                &assets,
                &file.format,
                prepared.size_mm,
                prepared.parameters.dpi,
            )
            .map_err(|e| e.to_string())?;
            if bytes.len() > 64 * 1024 * 1024 {
                return Err("Encoded output exceeds 64 MiB budget".into());
            }
            target.install_export(&bytes, &permission)?;
            Ok(bytes.len())
        })();
        match result {
            Ok(bytes) => {
                file.status = "succeeded".into();
                file.bytes = Some(bytes);
                file.message =
                    "Captured canvas installed atomically; Undo does not restore disk files."
                        .into();
            }
            Err(e) => {
                file.status = "failed".into();
                file.error_code = Some(
                    if permission.is_revoked() {
                        "request_cancelled"
                    } else {
                        "export_failed"
                    }
                    .into(),
                );
                file.message = e;
            }
        }
    }
    files
}

#[cfg(test)]
#[path = "external_export_tests.rs"]
mod tests;
