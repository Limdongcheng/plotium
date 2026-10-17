//! Frame driving and native application lifecycle.

use super::*;

pub(super) fn phase_after_polls(
    phase: plotium_core::state::ProjectTransitionPhase,
    dirty: bool,
    save_busy: bool,
) -> plotium_core::state::ProjectTransitionPhase {
    use plotium_core::state::ProjectTransitionPhase;
    if phase != ProjectTransitionPhase::CheckUnsaved || save_busy {
        return phase;
    }
    if dirty {
        ProjectTransitionPhase::NeedsConfirmation
    } else {
        ProjectTransitionPhase::Ready
    }
}

impl eframe::App for Shell {
    #[cfg(target_os = "macos")]
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Native menus and Dock events must also work with no visible window.
        if self.pending_recovery.is_none() {
            self.native_menu
                .poll(&mut self.app, &mut self.clipboard_table_paste, ctx);
        }
        // Fullscreen transitions can make the root temporarily non-visible:
        // eframe still calls logic, but skips ui and would otherwise exit.
        ui::handle_close_request(&mut self.app, ctx);
    }

    #[cfg(windows)]
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        ui::clipboard_native::restore_missing_paste_shortcut(raw_input);
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        observability::show_pending_crash_dialog();
        self.scale.drive(&mut self.app, &ctx, frame);
        let ribbon_chrome = ui::current_ribbon_chrome(&ctx, frame);
        let recovery_blocked = self.pending_recovery.is_some();
        if !recovery_blocked && let Some(driver) = &mut self.shot {
            driver.drive(&mut self.app, &ctx);
        }
        self.show_recovery_prompt(&ctx);
        self.app.poll_compute();
        if self.app.session.updates.tick() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        } else if let Some(delay) = self.app.session.updates.next_check_delay() {
            ctx.request_repaint_after(delay.max(std::time::Duration::from_millis(100)));
        }
        if let plotium_core::update::UpdateStatus::Installed { plan, .. } =
            self.app.session.updates.status()
        {
            *PENDING_INSTALL.lock().unwrap() = Some(plan.clone());
        }
        let fitting = self.app.poll_line_fit() | self.app.poll_xps_fit();
        let symmetry = self.app.poll_symmetry_audit();
        let transforming = self.app.poll_table_transform();
        ui::render(
            &mut self.app,
            &mut self.clipboard_table_paste,
            &mut self.batch_workflow,
            &mut self.last_window_title,
            ui,
            recovery_blocked,
            ribbon_chrome,
        );
        // Apply save completion after every edit-producing poll. This makes the
        // generation check below cover compute results that arrived this frame.
        self.poll_manual_project_save(&ctx);
        self.start_pending_project_save(&ctx);
        self.drive_project_transition(&ctx);
        if fitting {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        if symmetry {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        if transforming {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        if self.app.compute_busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
        if self.app.data_export_busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
        self.tick_recovery(&ctx);
    }

    #[cfg(windows)]
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.shutdown();
    }

    #[cfg(not(windows))]
    fn on_exit(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::phase_after_polls;
    use plotium_core::state::ProjectTransitionPhase::*;

    #[test]
    fn clean_quit_checks_changes_after_polls_and_save_barriers() {
        assert_eq!(phase_after_polls(CheckUnsaved, false, false), Ready);
        assert_eq!(
            phase_after_polls(CheckUnsaved, true, false),
            NeedsConfirmation
        );
        for dirty in [false, true] {
            assert_eq!(phase_after_polls(CheckUnsaved, dirty, true), CheckUnsaved);
        }
        let waiting = phase_after_polls(CheckUnsaved, false, true);
        // A background result arrives while the recovery/manual save finishes.
        assert_eq!(phase_after_polls(waiting, true, false), NeedsConfirmation);
    }

    #[test]
    fn confirmed_discard_and_save_phases_are_not_rechecked() {
        for phase in [Ready, Saving, NeedsConfirmation] {
            for save_busy in [false, true] {
                assert_eq!(phase_after_polls(phase, true, save_busy), phase);
            }
        }
    }
}
