use super::*;

fn revoked_save_preserves_sibling(kind: &str) {
    let dir = std::env::temp_dir().join(format!("plotium-save-boundary-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let target = dir.join("target.plotium");
    let other = dir.join("other.data");
    let sibling = temporary_path(&target);
    std::fs::write(&target, b"authorized old target").unwrap();
    std::fs::write(&other, b"unrelated sentinel").unwrap();
    match kind {
        "regular" => std::fs::write(&sibling, b"unrelated sentinel").unwrap(),
        "hardlink" => std::fs::hard_link(&other, &sibling).unwrap(),
        "target_hardlink" => std::fs::hard_link(&target, &sibling).unwrap(),
        #[cfg(unix)]
        "symlink" => std::os::unix::fs::symlink(&other, &sibling).unwrap(),
        _ => unreachable!(),
    }
    let mut app = PlotiumApp::new_with_settings(Default::default());
    let id = "revoked-after-capture";
    let permission = app
        .request_external_save(
            id.into(),
            SaveTarget::inspect(&target).unwrap(),
            app.doc.automation_revision,
            false,
        )
        .unwrap();
    app.decide_external_save(id, true, true);
    let request = app.capture_external_save(id).unwrap();
    let progress = request.progress();
    permission.revoke();
    let result = save_project_snapshot(request);
    assert_eq!(save_result_phase(&result), SavePhase::NotInstalled);
    assert_eq!(progress.phase(), SavePhase::NotInstalled);
    app.finish_external_save(
        id,
        save_result_phase(&result),
        false,
        Some(result.unwrap_err().to_string()),
    );
    assert_eq!(
        app.session.external_saves.receipt(id).unwrap().phase,
        Some(SavePhase::NotInstalled)
    );
    assert_eq!(std::fs::read(&target).unwrap(), b"authorized old target");
    assert_eq!(std::fs::read(&other).unwrap(), b"unrelated sentinel");
    assert_eq!(
        std::fs::read(&sibling).unwrap(),
        if kind == "target_hardlink" {
            b"authorized old target".as_slice()
        } else {
            b"unrelated sentinel".as_slice()
        }
    );
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 3);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn revoked_capture_preserves_regular_tmp_sentinel() {
    revoked_save_preserves_sibling("regular");
}
#[test]
fn revoked_capture_preserves_hardlinked_other_file() {
    revoked_save_preserves_sibling("hardlink");
}
#[test]
fn revoked_capture_preserves_hardlinked_target() {
    revoked_save_preserves_sibling("target_hardlink");
}
#[cfg(unix)]
#[test]
fn revoked_capture_preserves_symlinked_other_file() {
    revoked_save_preserves_sibling("symlink");
}

#[test]
fn native_save_ignores_existing_fixed_tmp_sibling() {
    let dir = std::env::temp_dir().join(format!("plotium-native-sibling-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let target = dir.join("target.plotium");
    let sibling = temporary_path(&target);
    std::fs::write(&sibling, b"unrelated sentinel").unwrap();
    let app = PlotiumApp::new_with_settings(Default::default());
    save_project(&app, &target, false).unwrap();
    assert_eq!(std::fs::read(&sibling).unwrap(), b"unrelated sentinel");
    assert!(load_project(&target).is_ok());
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
    std::fs::remove_dir_all(dir).unwrap();
}
