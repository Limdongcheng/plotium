use super::super::super::*;
use super::*;
use std::{cell::RefCell, path::PathBuf, rc::Rc};

struct FailedSave {
    _directory: tempfile::TempDir,
    target: PathBuf,
    backup: PathBuf,
    candidate: PathBuf,
    persisted_revision: String,
    message: String,
}

#[derive(Clone, Copy)]
enum Interference {
    None,
    TargetReappears,
    BackupBlocked,
}

// Model the documented ReplaceFileW side effects, not a second save state machine.
// The real serializer, admission, backup staging/rollback and TempPath drop run.
fn partial_save(code: i32, backups: u8, external: bool, interference: Interference) -> FailedSave {
    let directory = tempfile::tempdir().unwrap();
    let target = directory.path().join("project.plotium");
    let backup = super::super::backup_path(&target, 0);
    std::fs::write(&target, b"original target").unwrap();
    if backups > 0 {
        std::fs::write(&backup, b"previous backup").unwrap();
    }
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.settings.general.project_backup_generations = backups;
    app.doc.project_path = Some(target.clone());
    app.doc.project_revision = Some("original revision".into());
    app.mark_document_dirty();
    let context = app.session.project_context;
    let id = "partial-replacement";
    let request = if external {
        app.request_external_save(
            id.into(),
            SaveTarget::inspect(&target).unwrap(),
            app.doc.automation_revision,
            false,
        )
        .unwrap();
        app.decide_external_save(id, true, true);
        app.capture_external_save(id).unwrap()
    } else {
        prepare_project_save(&app, &target, false)
    };
    let identity = request.identity().clone();
    let persisted_revision = request.persisted_revision().to_owned();
    let progress = request.progress();
    let candidate = Rc::new(RefCell::new(None));
    let captured = candidate.clone();
    REPLACE_TEST.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(move |temp, target, backup| {
            *captured.borrow_mut() = Some(temp.to_owned());
            match code {
                1176 if backup.is_none() => std::fs::remove_file(target).unwrap(),
                1177 => {
                    let unknown = target.with_file_name("windows-selected-name");
                    std::fs::rename(target, backup.unwrap_or(&unknown)).unwrap();
                    if matches!(interference, Interference::TargetReappears) {
                        std::fs::write(target, b"intervening target").unwrap();
                    }
                }
                _ => {}
            }
            let error =
                windows_replace_error(io::Error::from_raw_os_error(code), temp, target, backup);
            if matches!(interference, Interference::BackupBlocked) {
                std::fs::write(backup.unwrap(), b"intervening backup").unwrap();
            }
            Err(error)
        }))
    });
    let result = save_project_snapshot(request);
    let phase = save_result_phase(&result);
    assert_eq!(phase, SavePhase::NotInstalled);
    assert_eq!(progress.phase(), SavePhase::NotInstalled);
    let message = result.as_ref().unwrap_err().to_string();
    let operation = app.session.begin_operation();
    assert!(!app.complete_captured_project_save(
        operation,
        &target,
        false,
        &identity,
        &persisted_revision,
        result
    ));
    assert!(app.doc.dirty);
    assert_eq!(app.session.project_context, context);
    assert_eq!(app.doc.project_path.as_ref(), Some(&target));
    assert_eq!(
        app.doc.project_revision.as_deref(),
        Some("original revision")
    );
    assert!(app.session.status.contains(&message));
    if external {
        app.finish_external_save(id, phase, false, Some(message.clone()));
        let receipt = app.session.external_saves.receipt(id).unwrap();
        assert_eq!(receipt.status, crate::state::ExternalSaveStatus::Completed);
        assert_eq!(receipt.phase, Some(SavePhase::NotInstalled));
        assert!(!receipt.state_applied);
        assert!(receipt.message.contains(&message));
        assert_eq!(
            receipt.persisted_revision.as_deref(),
            Some(persisted_revision.as_str())
        );
        assert!(receipt.snapshot.is_some());
        assert_eq!(receipt.dirty_after_completion, None);
        assert!(app.capture_external_save(id).is_none());
    }
    let candidate = candidate.borrow_mut().take().unwrap();
    FailedSave {
        _directory: directory,
        target,
        backup,
        candidate,
        persisted_revision,
        message,
    }
}

fn assert_retained_snapshot(save: &FailedSave) {
    assert!(
        save.candidate.exists(),
        "serialized candidate was removed: {}",
        save.candidate.display()
    );
    let loaded = load_project(&save.candidate).unwrap();
    assert_eq!(
        loaded.doc.project_revision.as_deref(),
        Some(save.persisted_revision.as_str())
    );
    assert!(save.message.contains(&save.candidate.display().to_string()));
    assert!(save.message.contains(&save.target.display().to_string()));
}

#[test]
fn partial_1176_without_backup_retains_serialized_snapshot() {
    for external in [false, true] {
        let save = partial_save(1176, 0, external, Interference::None);
        assert!(!save.target.exists());
        assert_retained_snapshot(&save);
    }
}

#[test]
fn partial_1177_restores_original_and_previous_backup() {
    for external in [false, true] {
        let save = partial_save(1177, 1, external, Interference::None);
        assert_eq!(
            std::fs::read(&save.target).ok().as_deref(),
            Some(b"original target".as_slice())
        );
        assert_eq!(std::fs::read(&save.backup).unwrap(), b"previous backup");
        assert_retained_snapshot(&save);
    }
}

#[test]
fn partial_1177_preserves_all_recovery_files_without_overwriting_new_target() {
    for external in [false, true] {
        let save = partial_save(1177, 1, external, Interference::TargetReappears);
        assert_eq!(std::fs::read(&save.target).unwrap(), b"intervening target");
        assert_eq!(std::fs::read(&save.backup).unwrap(), b"original target");
        let staged = std::fs::read_dir(save.target.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(".plotium-backup-")
            })
            .unwrap();
        assert_eq!(std::fs::read(&staged).unwrap(), b"previous backup");
        assert!(save.message.contains(&staged.display().to_string()));
        assert!(save.message.contains(&save.backup.display().to_string()));
        assert_retained_snapshot(&save);
    }
}

#[test]
fn partial_1177_without_backup_reports_unknown_original_location() {
    let save = partial_save(1177, 0, false, Interference::None);
    assert!(!save.target.exists());
    assert_eq!(
        std::fs::read(save.target.with_file_name("windows-selected-name")).unwrap(),
        b"original target"
    );
    assert!(save.message.contains("recovery path was not returned"));
    assert_retained_snapshot(&save);
}

#[test]
fn unchanged_name_errors_restore_backup_and_remove_private_snapshot() {
    for code in [1176, 1175, 5] {
        for external in [false, true] {
            let save = partial_save(code, 1, external, Interference::None);
            assert_eq!(std::fs::read(&save.target).unwrap(), b"original target");
            assert_eq!(std::fs::read(&save.backup).unwrap(), b"previous backup");
            assert!(!save.candidate.exists());
            assert!(!save.message.contains("snapshot retained"));
            assert_eq!(
                std::fs::read_dir(save.target.parent().unwrap())
                    .unwrap()
                    .count(),
                2
            );
        }
    }
}

#[test]
fn partial_1177_retains_candidate_if_previous_backup_rollback_fails() {
    let save = partial_save(1177, 1, true, Interference::BackupBlocked);
    assert_eq!(std::fs::read(&save.target).unwrap(), b"original target");
    assert_eq!(std::fs::read(&save.backup).unwrap(), b"intervening backup");
    let staged = std::fs::read_dir(save.target.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".plotium-backup-")
        })
        .unwrap();
    assert_eq!(std::fs::read(&staged).unwrap(), b"previous backup");
    assert!(save.message.contains(&staged.display().to_string()));
    assert!(save.message.contains("could not finish restoring"));
    assert_retained_snapshot(&save);
}
