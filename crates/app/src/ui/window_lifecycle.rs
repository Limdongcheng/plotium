//! Closing a macOS fullscreen window must finish its native Space transition
//! before ordering it out; otherwise AppKit cannot reliably reopen it.

use plotium_core::state::PlotiumApp;

#[derive(Clone, Copy)]
enum PendingClose {
    AwaitingFullscreenExit,
    Ready,
}

#[derive(Clone, Default)]
struct WindowLifecycle {
    transitioning: bool,
    pending: Option<PendingClose>,
}

fn state_id() -> egui::Id {
    egui::Id::new("native_window_lifecycle")
}

pub(super) fn cancel_close(ctx: &egui::Context) {
    ctx.data_mut(|data| {
        let mut state = data
            .get_temp::<WindowLifecycle>(state_id())
            .unwrap_or_default();
        state.pending = None;
        data.insert_temp(state_id(), state);
    });
}

pub(super) fn fullscreen_notification(ctx: &egui::Context, transitioning: bool, exited: bool) {
    ctx.data_mut(|data| {
        let mut state = data
            .get_temp::<WindowLifecycle>(state_id())
            .unwrap_or_default();
        state.transitioning = transitioning;
        if exited && state.pending.is_some() {
            state.pending = Some(PendingClose::Ready);
        }
        data.insert_temp(state_id(), state);
    });
    ctx.request_repaint();
}

pub(super) fn finish_close(ctx: &egui::Context) {
    let ready = ctx.data_mut(|data| {
        let mut state = data
            .get_temp::<WindowLifecycle>(state_id())
            .unwrap_or_default();
        let ready = matches!(state.pending, Some(PendingClose::Ready));
        if ready {
            state.pending = None;
            data.insert_temp(state_id(), state);
        }
        ready
    });
    if ready {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
    }
}

pub(super) fn close_window(app: &mut PlotiumApp, ctx: &egui::Context) {
    app.session.present_mode = false;
    app.session.present_fullscreen_on = false;
    let fullscreen = ctx.input(|input| input.viewport().fullscreen.unwrap_or(false));
    let (defer, already_pending) = ctx.data_mut(|data| {
        let mut state = data
            .get_temp::<WindowLifecycle>(state_id())
            .unwrap_or_default();
        let already_pending = state.pending.is_some();
        let defer = fullscreen || state.transitioning || already_pending;
        if defer && !already_pending {
            state.pending = Some(PendingClose::AwaitingFullscreenExit);
        }
        data.insert_temp(state_id(), state);
        (defer, already_pending)
    });
    if defer {
        if !already_pending {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        }
    } else {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        ctx: &egui::Context,
        fullscreen: bool,
        f: impl FnOnce(&egui::Context),
    ) -> egui::FullOutput {
        let mut input = egui::RawInput::default();
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .fullscreen = Some(fullscreen);
        let mut f = Some(f);
        ctx.run_ui(input, |ui| f.take().unwrap()(ui.ctx()))
    }

    fn has(output: &egui::FullOutput, command: egui::ViewportCommand) -> bool {
        output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&command)
    }

    #[test]
    fn close_waits_for_native_exit_completion_not_the_early_winit_flag() {
        let ctx = egui::Context::default();
        let mut app = PlotiumApp::new_with_settings(Default::default());
        let output = frame(&ctx, true, |ctx| close_window(&mut app, ctx));
        assert!(has(&output, egui::ViewportCommand::Fullscreen(false)));
        assert!(!has(&output, egui::ViewportCommand::Visible(false)));
        // A repeated close cannot bypass the native-completion gate even if
        // winit's flag and the interleaved notification state look windowed.
        fullscreen_notification(&ctx, false, false);
        let output = frame(&ctx, false, |ctx| close_window(&mut app, ctx));
        assert!(!has(&output, egui::ViewportCommand::Visible(false)));
        let output = frame(&ctx, false, finish_close);
        assert!(!has(&output, egui::ViewportCommand::Visible(false)));
        fullscreen_notification(&ctx, false, true);
        let output = frame(&ctx, false, finish_close);
        assert!(has(&output, egui::ViewportCommand::Visible(false)));
    }

    #[test]
    fn close_during_exit_animation_waits_and_reopen_cancels_the_hide() {
        let ctx = egui::Context::default();
        let mut app = PlotiumApp::new_with_settings(Default::default());
        fullscreen_notification(&ctx, true, false);
        let output = frame(&ctx, false, |ctx| close_window(&mut app, ctx));
        assert!(!has(&output, egui::ViewportCommand::Visible(false)));
        cancel_close(&ctx);
        fullscreen_notification(&ctx, false, true);
        let output = frame(&ctx, false, finish_close);
        assert!(!has(&output, egui::ViewportCommand::Visible(false)));
    }
}
