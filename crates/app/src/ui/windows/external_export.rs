//! Consent for an exact frozen canvas and exact output paths, not edit permission.
use super::*;
pub(in crate::ui) fn external_export_window(app: &mut PlotiumApp, ctx: &egui::Context) {
    app.session.external_exports.reconcile();
    let Some(receipt) = app.session.external_exports.pending().cloned() else {
        return;
    };
    let mut allow = false;
    let mut deny = false;
    let key = egui::Id::new(("external_export_replace", &receipt.export_id));
    let mut replace = ctx.data(|d| d.get_temp::<bool>(key)).unwrap_or(false);
    let overwrite = receipt
        .files
        .iter()
        .any(|f| f.status == "pending" && f.overwrite_required);
    let modal = super::modal(ctx, "external_figure_export", ModalKind::Dialog).show(ctx, |ui| {
        ui.set_width(500.0);
        ui.heading("Allow AI to export this figure?");
        ui.label(format!(
            "Canvas {} · revision {} · {} × {} mm · {} DPI PNG",
            receipt.canvas.resource.id,
            receipt.render_revision,
            receipt.size_mm[0],
            receipt.size_mm[1],
            receipt.dpi
        ));
        for f in &receipt.files {
            ui.label(format!("{}: {}", f.format.to_uppercase(), f.path.display()));
            if f.status == "failed" {
                ui.label(&f.message);
            }
        }
        if overwrite {
            ui.checkbox(&mut replace, "Replace the listed existing output files");
        }
        ui.label("This export only. Undo cannot restore disk files.");
        ui.horizontal(|ui| {
            allow = ui
                .add_enabled(!overwrite || replace, egui::Button::new("Allow export"))
                .clicked();
            deny = ui.button("Deny").clicked();
        });
    });
    ctx.data_mut(|d| d.insert_temp(key, replace));
    if allow || deny || modal.should_close() {
        app.decide_external_export(&receipt.export_id, allow, replace);
        ctx.data_mut(|d| d.remove::<bool>(key));
    }
}
