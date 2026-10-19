use super::*;
use plotium_core::{
    project::{RecoveryManager, SavePhase, SaveTarget},
    state::{ExternalSaveStatus, ProjectTransition, ProjectTransitionPhase},
};
use std::sync::mpsc::{Sender, channel};

fn dirty_project() -> PlotiumApp {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.start_new_project();
    app.mark_document_dirty();
    app
}

fn paused_recovery(app: &PlotiumApp) -> (RecoveryManager, Option<RecoveryJob>, Sender<()>) {
    let manager = RecoveryManager::new().unwrap();
    let request = plotium_core::project::prepare_recovery_snapshot(app).unwrap();
    let target = manager.target();
    let (release, wait) = channel();
    let job = RecoveryJob {
        generation: app.doc.edit_generation,
        handle: std::thread::spawn(move || {
            wait.recv().unwrap();
            plotium_core::project::save_recovery_snapshot(request, target)
        }),
    };
    (manager, Some(job), release)
}

fn finish_recovery(job: &mut Option<RecoveryJob>, release: Sender<()>) {
    release.send(()).unwrap();
    job.take().unwrap().handle.join().unwrap().unwrap();
}

fn request_transition(app: &mut PlotiumApp, target: &ProjectTransition, ctx: &egui::Context) {
    match target {
        ProjectTransition::Quit => {
            ui::commands::execute_without_clipboard(ui::commands::CommandId::Quit, app, ctx)
        }
        ProjectTransition::New => {
            ui::commands::execute_without_clipboard(ui::commands::CommandId::NewProject, app, ctx)
        }
        // The native Open dialog hands this exact transition to core after a
        // path is chosen; the regression must not open a real file picker.
        ProjectTransition::Open(_) => app.request_project_transition(target.clone()),
        _ => unreachable!(),
    }
}

#[test]
fn queued_external_save_cancellation_releases_project_transition() {
    crate::test_settings::isolated(
        "project_save::tests::queued_external_save_cancellation_releases_project_transition",
        || {
            let dir = tempfile::tempdir().unwrap();
            for (index, target) in [
                ProjectTransition::Quit,
                ProjectTransition::New,
                ProjectTransition::Open(dir.path().join("next.plotium")),
            ]
            .into_iter()
            .enumerate()
            {
                let mut app = dirty_project();
                let context = app.session.project_context;
                let path = dir.path().join(format!("queued-{index}.plotium"));
                let ctx = egui::Context::default();
                let (recovery, mut recovery_job, release) = paused_recovery(&app);
                let authorization = app
                    .request_external_save(
                        "queued".into(),
                        SaveTarget::inspect(&path).unwrap(),
                        app.doc.automation_revision,
                        false,
                    )
                    .unwrap();
                app.decide_external_save("queued", true, false);
                let mut save_job = None;
                start_pending_project_save(&mut app, &mut save_job, recovery_job.as_ref(), &ctx);
                let receipt = app.session.external_saves.receipt("queued").unwrap();
                assert_eq!(receipt.status, ExternalSaveStatus::Queued);
                assert!(receipt.snapshot.is_none());
                assert!(save_job.is_none());
                assert!(!recovery_job.as_ref().unwrap().handle.is_finished());

                request_transition(&mut app, &target, &ctx);
                assert!(authorization.is_revoked());
                start_pending_project_save(&mut app, &mut save_job, recovery_job.as_ref(), &ctx);
                assert!(
                    ready_project_transition(&mut app, &mut save_job, recovery_job.as_ref(), &ctx)
                        .is_none()
                );
                assert_eq!(
                    app.session.ui.project_transition.as_ref().unwrap().phase,
                    ProjectTransitionPhase::Saving
                );
                assert!(
                    app.session
                        .ui
                        .pending_project_save
                        .as_ref()
                        .unwrap()
                        .continue_transition
                );

                finish_recovery(&mut recovery_job, release);
                start_pending_project_save(&mut app, &mut save_job, recovery_job.as_ref(), &ctx);
                assert!(
                    ready_project_transition(&mut app, &mut save_job, recovery_job.as_ref(), &ctx)
                        .is_none()
                );
                recovery.shutdown().unwrap();
                let receipt = app.session.external_saves.receipt("queued").unwrap();
                assert_eq!(receipt.status, ExternalSaveStatus::Completed);
                assert_eq!(receipt.phase, Some(SavePhase::NotInstalled));
                assert!(receipt.snapshot.is_none());
                assert!(receipt.persisted_revision.is_none());
                assert!(!receipt.state_applied);
                assert!(!path.exists());
                assert!(app.doc.dirty);
                assert_eq!(app.session.project_context, context);
                assert!(save_job.is_none());
                assert!(app.session.ui.pending_project_save.is_none());
                assert!(!app.session.ui.project_save_in_progress);
                let pending = app.session.ui.project_transition.as_ref().unwrap();
                assert_eq!(pending.target, target);
                assert_eq!(pending.phase, ProjectTransitionPhase::NeedsConfirmation);
                // The existing confirmation UI enables Save/Discard/Cancel in
                // NeedsConfirmation, and native Save is no longer busy-gated.
                assert!(ui::commands::describe(&app, ui::commands::CommandId::SaveProject).enabled);
            }
        },
    );
}

#[test]
fn ordinary_queued_save_still_completes_its_project_transition() {
    crate::test_settings::isolated(
        "project_save::tests::ordinary_queued_save_still_completes_its_project_transition",
        || {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("native.plotium");
            let mut app = dirty_project();
            let ctx = egui::Context::default();
            let (recovery, mut recovery_job, release) = paused_recovery(&app);
            let mut save_job = None;
            app.queue_project_save(path.clone(), false, false);
            start_pending_project_save(&mut app, &mut save_job, recovery_job.as_ref(), &ctx);
            request_transition(&mut app, &ProjectTransition::Quit, &ctx);
            assert!(
                ready_project_transition(&mut app, &mut save_job, recovery_job.as_ref(), &ctx)
                    .is_none()
            );
            assert_eq!(
                app.session.ui.project_transition.as_ref().unwrap().phase,
                ProjectTransitionPhase::Saving
            );
            finish_recovery(&mut recovery_job, release);
            start_pending_project_save(&mut app, &mut save_job, recovery_job.as_ref(), &ctx);
            assert!(save_job.as_ref().unwrap().continue_transition);
            let deadline = Instant::now() + Duration::from_secs(5);
            while !save_job.as_ref().unwrap().handle.is_finished() {
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
            shell::poll_manual_project_save(&mut app, &mut save_job, &ctx);
            assert!(save_job.is_none());
            assert!(!app.doc.dirty);
            assert!(!app.session.ui.project_save_in_progress);
            assert!(plotium_core::project::load_project(&path).is_ok());
            assert_eq!(
                ready_project_transition(&mut app, &mut save_job, recovery_job.as_ref(), &ctx),
                Some(ProjectTransition::Quit)
            );
            recovery.shutdown().unwrap();
        },
    );
}
