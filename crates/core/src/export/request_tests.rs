use super::*;
#[test]
fn missing_fonts_and_invalid_sizes_have_specific_failures() {
    assert!(matches!(
        require_export_fonts(&resvg::usvg::fontdb::Database::new()),
        Err(ExportError::MissingFont)
    ));
    let mut a = PlotiumApp::new_with_settings(Default::default());
    a.doc
        .canvases
        .push(CanvasDocument::new("native".into(), [180.0, 90.0]));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("x.png");
    let mut p = FigureExportParams {
        canvas: TargetRef::resource(a.doc.canvases[0].resource_id.into()),
        outputs: vec![ExportOutput {
            format: "png".into(),
            path,
            overwrite: false,
        }],
        size_mm: Some([180.0, 80.0]),
        dpi: 300,
    };
    assert!(
        prepare_figure_export(&a, p.clone())
            .unwrap_err()
            .contains("aspect")
    );
    p.size_mm = None;
    p.dpi = 2400;
    assert!(
        prepare_figure_export(&a, p.clone())
            .unwrap_err()
            .contains("budget")
    );
    p.dpi = 300;
    p.outputs.push(p.outputs[0].clone());
    assert!(
        prepare_figure_export(&a, p)
            .unwrap_err()
            .contains("Duplicate")
    );
}
#[test]
fn output_installation_checks_revocation_drift_and_owned_staging() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("figure.svg");
    let sibling = dir.path().join("figure.svg.tmp");
    std::fs::write(&path, b"original").unwrap();
    std::fs::write(&sibling, b"untouched sibling").unwrap();
    let target = ExportTarget::inspect(&path, "svg").unwrap();
    let permit = ExportAuthorization::default();
    assert!(permit.grant());
    permit.revoke();
    assert!(target.install_export(b"replacement", &permit).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"original");
    assert_eq!(std::fs::read(&sibling).unwrap(), b"untouched sibling");
    let permit = ExportAuthorization::default();
    assert!(permit.grant());
    std::fs::write(&path, b"externally changed later").unwrap();
    assert!(target.install_export(b"replacement", &permit).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"externally changed later");
    let target = ExportTarget::inspect(&path, "svg").unwrap();
    assert!(target.install_export(b"replacement", &permit).is_ok());
    assert_eq!(std::fs::read(&path).unwrap(), b"replacement");
    assert_eq!(std::fs::read(&sibling).unwrap(), b"untouched sibling");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
}
#[cfg(unix)]
#[test]
fn output_symlink_is_never_followed() {
    let dir = tempfile::tempdir().unwrap();
    let original = dir.path().join("original");
    let link = dir.path().join("linked.svg");
    std::fs::write(&original, b"real bytes").unwrap();
    std::os::unix::fs::symlink(&original, &link).unwrap();
    assert!(ExportTarget::inspect(&link, "svg").is_err());
    assert_eq!(std::fs::read(original).unwrap(), b"real bytes");
}
