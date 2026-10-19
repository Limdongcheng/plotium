//! Per-request file-write consent, independent of reversible editing access.
use super::*;
pub(in crate::ui) fn external_save_window(app: &mut PlotiumApp, ctx: &egui::Context) {
    app.session.external_saves.reconcile();
    let Some(receipt) = app.session.external_saves.pending().cloned() else {
        return;
    };
    let mut allow = false;
    let mut deny = false;
    let key = egui::Id::new(("external_save_replace", &receipt.save_id));
    let mut replace = ctx.data(|d| d.get_temp::<bool>(key)).unwrap_or(false);
    let modal = super::modal(ctx, "external_project_save", ModalKind::Dialog).show(ctx, |ui| {
        ui.set_width(460.0);
        ui.heading("Allow AI to save this project?");
        ui.label("Write the current editable desktop project to:");
        ui.label(receipt.path.display().to_string());
        if receipt.overwrite_required {
            ui.checkbox(&mut replace, "Replace the existing file at this exact path");
        }
        ui.label("This permission applies to this save only. Undo cannot restore disk files.");
        ui.horizontal(|ui| {
            allow = ui
                .add_enabled(
                    !receipt.overwrite_required || replace,
                    egui::Button::new("Allow save"),
                )
                .clicked();
            deny = ui.button("Deny").clicked();
        });
    });
    ctx.data_mut(|d| d.insert_temp(key, replace));
    if allow || deny || modal.should_close() {
        app.decide_external_save(&receipt.save_id, allow, replace);
        ctx.data_mut(|d| d.remove::<bool>(key));
    }
}
