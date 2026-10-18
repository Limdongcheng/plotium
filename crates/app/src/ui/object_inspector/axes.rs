use egui::Ui;
use plotium_core::properties::axis;
use plotium_core::state::{ObjectId, PlotiumApp};

pub(super) fn commit_if_target_changed(app: &mut PlotiumApp, target: Option<(usize, ObjectId)>) {
    let pending_target = app
        .session
        .ui
        .axis_overrides_before
        .as_ref()
        .map(|(canvas, object, _)| (*canvas, *object));
    if pending_target.is_some() && pending_target != target {
        commit_edit(app);
    }
}

pub(super) fn axes_section(
    app: &mut PlotiumApp,
    canvas: usize,
    object: ObjectId,
    ui: &mut Ui,
) -> bool {
    if ui
        .button("Automatic visibility")
        .on_hover_text("Clear all axis visibility overrides")
        .clicked()
        && let Some(target) = app.object_target(canvas, object)
    {
        match app.plan_property_resets(
            &[
                axis::X_SHOW_TICK_LABELS,
                axis::X_SHOW_LABEL,
                axis::Y_SHOW_TICK_LABELS,
                axis::Y_SHOW_LABEL,
            ],
            std::slice::from_ref(&target),
        ) {
            Ok(commit) => {
                app.commit_property(commit);
            }
            Err(error) => {
                app.session.status = format!("Could not reset axis visibility: {error}");
            }
        }
    }
    false
}

fn commit_edit(app: &mut PlotiumApp) {
    app.finish_axis_overrides_edit();
}
