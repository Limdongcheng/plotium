use super::*;
use crate::export::{ExportOutput, FigureExportParams, prepare_figure_export};
fn app() -> PlotiumApp {
    let mut a = PlotiumApp::new_with_settings(Default::default());
    a.doc
        .canvases
        .push(CanvasDocument::new("snapshot".into(), [180.0, 90.0]));
    a
}
#[test]
fn background_snapshot_and_completion_keep_original_project_identity() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("captured.svg");
    let mut a = app();
    let target = TargetRef::resource(a.doc.canvases[0].resource_id.into());
    let prepared = prepare_figure_export(
        &a,
        FigureExportParams {
            canvas: target,
            outputs: vec![ExportOutput {
                format: "svg".into(),
                path: path.clone(),
                overwrite: false,
            }],
            size_mm: None,
            dpi: 300,
        },
    )
    .unwrap();
    let original_context = a.session.project_context;
    let revision = a.doc.automation_revision;
    a.request_external_export("snapshot".into(), prepared, revision)
        .unwrap();
    a.decide_external_export("snapshot", true, false);
    a.doc.canvases[0].background = plotium_figure::Color::rgb(255, 0, 0);
    a.doc.automation_revision += 1;
    while !a
        .session
        .external_exports
        .flight
        .as_ref()
        .unwrap()
        .handle
        .is_finished()
    {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    a.session.project_context = Default::default();
    a.session.external_exports.revoke();
    a.session.status = "New project status".into();
    let generation = a.doc.edit_generation;
    a.poll_external_export();
    let receipt = a.session.external_exports.receipt("snapshot").unwrap();
    assert_eq!(receipt.project_context, original_context);
    assert_eq!(receipt.render_revision, revision);
    assert_eq!(receipt.files[0].status, "succeeded");
    assert_eq!(a.session.status, "New project status");
    assert_eq!(a.doc.edit_generation, generation);
    let svg = std::fs::read_to_string(path).unwrap();
    assert!(!svg.contains("#ff0000"));
    assert_eq!(
        a.doc.canvases[0].background,
        plotium_figure::Color::rgb(255, 0, 0)
    );
}

#[test]
fn panicked_export_worker_releases_permission_and_keeps_unknown_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = app();
    let prepared = prepare_figure_export(
        &a,
        FigureExportParams {
            canvas: TargetRef::resource(a.doc.canvases[0].resource_id.into()),
            outputs: vec![ExportOutput {
                format: "svg".into(),
                path: dir.path().join("unknown.svg"),
                overwrite: false,
            }],
            size_mm: None,
            dpi: 300,
        },
    )
    .unwrap();
    let permission = a
        .request_external_export("panic".into(), prepared, a.doc.automation_revision)
        .unwrap();
    assert!(permission.grant());
    a.session
        .external_exports
        .requests
        .back_mut()
        .unwrap()
        .receipt
        .status = ExportStatus::Rendering;
    // Exercise the real shell poll/join path without manufacturing a successful
    // result for a worker whose output state cannot be known after a panic.
    a.session.external_exports.flight = Some(Flight {
        id: "panic".into(),
        handle: std::thread::spawn(|| panic!("controlled export worker failure")),
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while a.poll_external_export() {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let receipt = a.session.external_exports.receipt("panic").unwrap();
    assert_eq!(receipt.status, ExportStatus::Completed);
    assert_eq!(receipt.files[0].status, "unknown");
    assert_eq!(
        receipt.files[0].error_code.as_deref(),
        Some("worker_failed")
    );
    assert!(permission.is_revoked());
}
