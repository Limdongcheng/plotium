//! Present mode: a transient slideshow that hides every editing panel and renders
//! one canvas full screen, letterboxed and centred, with keyboard/click paging.

use egui::{Align2, Color32, Key, Sense};
use plotium_core::state::PlotiumApp;
use plotium_render::{Document, DocumentViewport, Rect as PlotRect};

const LETTERBOX: Color32 = Color32::from_rgb(0x0a, 0x0a, 0x0a);

/// Which page a navigation key moves to, clamped to `[0, count)` with no wrap.
#[derive(Clone, Copy)]
pub(crate) enum PresentNav {
    Next,
    Prev,
    First,
    Last,
}

pub(crate) fn present_page_after(current: usize, count: usize, nav: PresentNav) -> usize {
    if count == 0 {
        return 0;
    }
    let last = count - 1;
    match nav {
        PresentNav::Next => (current + 1).min(last),
        PresentNav::Prev => current.saturating_sub(1),
        PresentNav::First => 0,
        PresentNav::Last => last,
    }
}

/// Enter or leave present mode from the toolbar. Entering with no canvases is a
/// no-op with a hint; the fullscreen request is edge-driven in `sync_fullscreen`.
pub(crate) fn toggle_present_mode(app: &mut PlotiumApp) {
    if app.session.present_mode {
        app.session.present_mode = false;
        return;
    }
    if app.doc.canvases.is_empty() {
        app.session.status = "Nothing to present — open a spectrum first.".to_owned();
        return;
    }
    app.session.present_page = app
        .session
        .active_canvas
        .unwrap_or(0)
        .min(app.doc.canvases.len() - 1);
    app.session.present_mode = true;
}

/// Drive the window's fullscreen state from `present_mode` on its rising/falling
/// edge, so the viewport command is sent once per transition rather than per frame.
pub(crate) fn sync_fullscreen(app: &mut PlotiumApp, ctx: &egui::Context) {
    // Quit confirmation lives in the editing UI. Leave the slideshow before
    // its early return can hide Save / Discard / Cancel.
    if app
        .session
        .ui
        .project_transition
        .as_ref()
        .is_some_and(|pending| pending.target == plotium_core::state::ProjectTransition::Quit)
    {
        app.session.present_mode = false;
    }
    if app.session.present_mode && !app.session.present_fullscreen_on {
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        app.session.present_fullscreen_on = true;
    } else if !app.session.present_mode && app.session.present_fullscreen_on {
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        app.session.present_fullscreen_on = false;
    }
}

pub(crate) fn render(app: &mut PlotiumApp, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    handle_input(app, &ctx);
    if !app.session.present_mode {
        // Esc (or an empty deck) exited this frame; draw nothing and let the
        // normal UI resume — `sync_fullscreen` restores the window next frame.
        ctx.request_repaint();
        return;
    }
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(LETTERBOX))
        .show_inside(ui, |ui| paint(app, ui));
}

fn handle_input(app: &mut PlotiumApp, ctx: &egui::Context) {
    let (mut next, mut prev, mut first, mut last, mut exit) = (false, false, false, false, false);
    ctx.input(|i| {
        exit = i.key_pressed(Key::Escape);
        next = i.key_pressed(Key::ArrowRight)
            || i.key_pressed(Key::ArrowDown)
            || i.key_pressed(Key::Space)
            || i.key_pressed(Key::PageDown);
        prev = i.key_pressed(Key::ArrowLeft)
            || i.key_pressed(Key::ArrowUp)
            || i.key_pressed(Key::PageUp);
        first = i.key_pressed(Key::Home);
        last = i.key_pressed(Key::End);
    });
    if exit || app.doc.canvases.is_empty() {
        app.session.present_mode = false;
        return;
    }
    let count = app.doc.canvases.len();
    if next {
        app.session.present_page =
            present_page_after(app.session.present_page, count, PresentNav::Next);
    }
    if prev {
        app.session.present_page =
            present_page_after(app.session.present_page, count, PresentNav::Prev);
    }
    if first {
        app.session.present_page =
            present_page_after(app.session.present_page, count, PresentNav::First);
    }
    if last {
        app.session.present_page =
            present_page_after(app.session.present_page, count, PresentNav::Last);
    }
}

fn paint(app: &mut PlotiumApp, ui: &mut egui::Ui) {
    let count = app.doc.canvases.len();
    let page = app.session.present_page.min(count.saturating_sub(1));
    let area = ui.available_rect_before_wrap();
    if ui.allocate_rect(area, Sense::click()).clicked() {
        app.session.present_page =
            present_page_after(app.session.present_page, count, PresentNav::Next);
    }
    let painter = ui.painter_at(area);

    let canvas = &app.doc.canvases[page];
    let [w, h] = canvas.size_pt();
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let zoom = (area.width() / w).min(area.height() / h);
    let pan = [
        (area.width() - w * zoom) * 0.5,
        (area.height() - h * zoom) * 0.5,
    ];
    let document = Document {
        width: w,
        height: h,
        background: canvas.background,
        items: plotium_core::state::document_items(canvas),
    };
    plotium_render::screen::paint_document(
        &painter,
        PlotRect::new(area.left(), area.top(), area.width(), area.height()),
        &document,
        DocumentViewport { zoom, pan },
    );

    if count > 1 {
        painter.text(
            egui::pos2(area.right() - 14.0, area.bottom() - 12.0),
            Align2::RIGHT_BOTTOM,
            format!("{} / {}", page + 1, count),
            crate::typography::headline_font(),
            Color32::from_gray(150),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quitting_a_dirty_slideshow_restores_the_confirmation_ui() {
        let mut app = PlotiumApp::new_with_settings(Default::default());
        app.mark_document_dirty();
        app.session.present_mode = true;
        app.session.present_fullscreen_on = true;
        app.request_project_transition(plotium_core::state::ProjectTransition::Quit);
        let ctx = egui::Context::default();
        let output = ctx.run_ui(Default::default(), |ui| sync_fullscreen(&mut app, ui.ctx()));
        assert!(!app.session.present_mode);
        assert!(!app.session.present_fullscreen_on);
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .contains(&egui::ViewportCommand::Fullscreen(false))
        );
        assert_eq!(
            app.session.ui.project_transition.unwrap().phase,
            plotium_core::state::ProjectTransitionPhase::NeedsConfirmation
        );
    }

    #[test]
    fn present_nav_clamps_at_both_ends() {
        assert_eq!(present_page_after(0, 3, PresentNav::Prev), 0);
        assert_eq!(present_page_after(2, 3, PresentNav::Next), 2);
        assert_eq!(present_page_after(1, 3, PresentNav::Next), 2);
        assert_eq!(present_page_after(1, 3, PresentNav::Prev), 0);
        assert_eq!(present_page_after(1, 3, PresentNav::First), 0);
        assert_eq!(present_page_after(1, 3, PresentNav::Last), 2);
        assert_eq!(present_page_after(0, 0, PresentNav::Next), 0);
    }
}
