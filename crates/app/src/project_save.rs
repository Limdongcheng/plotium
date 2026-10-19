//! Native desktop save scheduling and the transition that may wait for it.
use super::*;

pub(super) fn start_pending_project_save(
    app: &mut PlotiumApp,
    save_job: &mut Option<ManualProjectSaveJob>,
    recovery_job: Option<&RecoveryJob>,
    ctx: &egui::Context,
) {
    app.session.external_saves.reconcile();
    if save_job.is_some() || recovery_job.is_some() {
        if app.session.ui.pending_project_save.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        return;
    }
    let Some(pending) = app.session.ui.pending_project_save.take() else {
        return;
    };
    let operation_id = app.session.begin_operation();
    let captured_generation = app.doc.edit_generation;
    let request = if let Some(id) = &pending.external_save_id {
        let Some(request) = app.capture_external_save(id) else {
            app.session.ui.project_save_in_progress = false;
            // A transition may have attached to this queued save while recovery
            // held it. Cancellation before capture has no worker completion to
            // release that wait, so restore the normal unsaved-changes choice.
            if pending.continue_transition
                && let Some(transition) = app.session.ui.project_transition.as_mut()
                && transition.phase == plotium_core::state::ProjectTransitionPhase::Saving
            {
                transition.phase = plotium_core::state::ProjectTransitionPhase::NeedsConfirmation;
            }
            ctx.request_repaint();
            return;
        };
        request
    } else {
        plotium_core::project::prepare_project_save(
            app,
            &pending.path,
            pending.include_view_snapshots,
        )
    };
    let identity = request.identity().clone();
    let persisted_revision = request.persisted_revision().to_owned();
    let progress = request.progress();
    app.session.status = format!("Saving project {}…", pending.path.display());
    *save_job = Some(ManualProjectSaveJob {
        operation_id,
        path: pending.path,
        include_view_snapshots: pending.include_view_snapshots,
        captured_generation,
        identity,
        persisted_revision,
        progress,
        external_save_id: pending.external_save_id,
        continue_transition: pending.continue_transition,
        handle: std::thread::spawn(move || plotium_core::project::save_project_snapshot(request)),
    });
    ctx.request_repaint_after(Duration::from_millis(100));
}

pub(super) fn ready_project_transition(
    app: &mut PlotiumApp,
    save_job: &mut Option<ManualProjectSaveJob>,
    recovery_job: Option<&RecoveryJob>,
    ctx: &egui::Context,
) -> Option<plotium_core::state::ProjectTransition> {
    use plotium_core::state::ProjectTransitionPhase;

    let mut transition = app.session.ui.project_transition.clone()?;
    let phase = shell::phase_after_polls(
        transition.phase,
        app.doc.dirty,
        save_job.is_some() || recovery_job.is_some(),
    );
    if phase != transition.phase {
        transition.phase = phase;
        if let Some(pending) = app.session.ui.project_transition.as_mut() {
            pending.phase = phase;
        }
        ctx.request_repaint();
    }
    if phase == ProjectTransitionPhase::CheckUnsaved {
        ctx.request_repaint_after(Duration::from_millis(100));
        return None;
    }
    if transition.phase == ProjectTransitionPhase::NeedsConfirmation
        && let Some(save) = save_job.as_mut()
    {
        save.continue_transition = true;
        transition.phase = ProjectTransitionPhase::Saving;
        if let Some(pending) = app.session.ui.project_transition.as_mut() {
            pending.phase = ProjectTransitionPhase::Saving;
        }
        ctx.request_repaint_after(Duration::from_millis(100));
        return None;
    }
    if transition.phase == ProjectTransitionPhase::NeedsConfirmation
        && let Some(save) = app.session.ui.pending_project_save.as_mut()
    {
        save.continue_transition = true;
        if let Some(pending) = app.session.ui.project_transition.as_mut() {
            pending.phase = ProjectTransitionPhase::Saving;
        }
        ctx.request_repaint_after(Duration::from_millis(100));
        return None;
    }
    if transition.phase != ProjectTransitionPhase::Ready {
        return None;
    }
    if save_job.is_some() || recovery_job.is_some() {
        ctx.request_repaint_after(Duration::from_millis(100));
        return None;
    }

    Some(transition.target)
}

#[cfg(all(test, unix))]
#[path = "project_save_tests.rs"]
mod tests;
