//! Local binding controls; planning and validation are the same service MCP uses.
use super::*;
use plotium_core::automation::DocumentRevision;
use plotium_core::data::{ColumnId, LogicalType};
use plotium_core::state::{
    CurveBindingEdit, MaterializeCurveEdit, SeriesBinding, SeriesSelection, TableCurveSource,
};

#[derive(Clone)]
struct Draft {
    source: TableCurveSource,
}
#[derive(Clone)]
struct Feedback {
    message: String,
    failed: bool,
}
pub(super) fn column_choice(
    ui: &mut Ui,
    label: &str,
    id: &mut ColumnId,
    columns: &[(ColumnId, String)],
    salt: impl std::hash::Hash,
) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(
            columns
                .iter()
                .find(|c| c.0 == *id)
                .map(|c| format!("{label}: {}", c.1))
                .unwrap_or_else(|| format!("{label}: missing column")),
        )
        .show_ui(ui, |ui| {
            for (c, name) in columns {
                if ui.selectable_value(id, *c, name).clicked() {
                    ui.close();
                }
            }
        });
}
pub(super) fn show(
    app: &mut PlotiumApp,
    ci: usize,
    object: ObjectId,
    series: &SeriesBinding,
    ui: &mut Ui,
) -> bool {
    let Some(dataset) = app.doc.dataset_by_id(series.source.resource) else {
        return false;
    };
    let Some(table) = dataset.as_table() else {
        return false;
    };
    let schema = &table.typed_state.envelope.revision.snapshot.schema;
    let columns = schema
        .columns
        .iter()
        .filter(|c| matches!(c.logical_type, LogicalType::Int64 | LogicalType::Float64))
        .map(|c| (c.id, c.name.clone()))
        .collect::<Vec<_>>();
    if columns.is_empty() {
        return false;
    }
    let revision = table.typed_state.envelope.revision.id;
    let canvas = app.doc.canvases[ci].resource_id;
    let Some(target) = app.series_target(ci, object, series.id) else {
        return false;
    };
    let current = series.source.columns();
    let initial = TableCurveSource {
        dataset_id: series.source.resource,
        field_id: series.source.field,
        table_id: table.typed_state.envelope.revision.table_id,
        table_revision: revision,
        x_column_id: current
            .map(|c| c.x_column_id)
            .or(table.x_binding)
            .unwrap_or(columns[0].0),
        y_column_id: current
            .map(|c| c.y_column_id)
            .or_else(|| table.series_bindings.first().map(|b| b.value_column))
            .unwrap_or(columns[columns.len().saturating_sub(1).min(1)].0),
        y_error_column_id: current.and_then(|c| c.y_error_column_id),
    };
    let defaults = table
        .series_bindings
        .iter()
        .map(|b| b.value_column)
        .collect::<Vec<_>>();
    let key = egui::Id::new((
        "curve_binding_draft",
        canvas,
        object,
        series.id,
        app.doc.automation_revision,
    ));
    let mut draft = ui
        .ctx()
        .data_mut(|d| d.get_temp::<Draft>(key))
        .unwrap_or(Draft { source: initial });
    // Always use the current snapshot when an author explicitly presses Apply.
    draft.source.table_revision = revision;
    let feedback_key = |revision| {
        egui::Id::new((
            canvas,
            object,
            series.id,
            revision,
            "curve_binding_feedback",
        ))
    };
    let current_feedback_key = feedback_key(app.doc.automation_revision);
    let mut apply = false;
    let mut materialize = false;
    ui.push_id((canvas, object, series.id, "binding"), |ui| {
        ui.collapsing("Curve columns", |ui| {
            if matches!(series.source.selection, SeriesSelection::FieldDefault)
                && defaults.len() > 1
            {
                ui.label("Expand these default curves before rebinding one curve.");
                let available = columns
                    .iter()
                    .filter(|c| defaults.contains(&c.0))
                    .cloned()
                    .collect::<Vec<_>>();
                if !defaults.contains(&draft.source.y_column_id) {
                    draft.source.y_column_id = defaults[0];
                }
                column_choice(
                    ui,
                    "Keep Series ID for",
                    &mut draft.source.y_column_id,
                    &available,
                    "retain_y",
                );
                materialize = ui.button("Expand default curves").clicked();
            } else {
                column_choice(ui, "X", &mut draft.source.x_column_id, &columns, "x");
                column_choice(ui, "Y", &mut draft.source.y_column_id, &columns, "y");
                egui::ComboBox::from_id_salt("error")
                    .selected_text(
                        draft
                            .source
                            .y_error_column_id
                            .and_then(|id| {
                                columns
                                    .iter()
                                    .find(|c| c.0 == id)
                                    .map(|c| format!("Y error: {}", c.1))
                            })
                            .unwrap_or_else(|| "Y error: none".into()),
                    )
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_value(&mut draft.source.y_error_column_id, None, "None")
                            .clicked()
                        {
                            ui.close();
                        }
                        for (id, name) in &columns {
                            if ui
                                .selectable_value(
                                    &mut draft.source.y_error_column_id,
                                    Some(*id),
                                    name,
                                )
                                .clicked()
                            {
                                ui.close();
                            }
                        }
                    });
                apply = ui.button("Apply curve binding").clicked();
            }
            if let Some(feedback) = ui
                .ctx()
                .data_mut(|d| d.get_temp::<Feedback>(current_feedback_key))
            {
                if feedback.failed {
                    ui.colored_label(ui.visuals().error_fg_color, feedback.message);
                } else {
                    ui.label(feedback.message);
                }
            }
        });
    });
    ui.ctx().data_mut(|d| d.insert_temp(key, draft.clone()));
    if !apply && !materialize {
        return false;
    }
    app.finish_series_presentation_edit();
    app.finish_axis_overrides_edit();
    let revision = DocumentRevision(app.doc.automation_revision);
    let plan = if materialize {
        app.plan_materialize_curves(
            revision,
            &[MaterializeCurveEdit {
                target,
                retain_y_column_id: draft.source.y_column_id,
                table_revision: draft.source.table_revision,
            }],
        )
    } else {
        app.plan_curve_bindings(
            revision,
            &[CurveBindingEdit {
                target,
                source: draft.source,
            }],
        )
    };
    let diagnostic=plan.as_ref().map(|p| {
        let valid=p.bindings.iter().map(|b|b.diagnostics.plotted_rows).sum::<usize>();
        let missing=p.bindings.iter().map(|b|b.diagnostics.missing_xy).sum::<usize>();
        let nonfinite=p.bindings.iter().map(|b|b.diagnostics.non_finite_xy).sum::<usize>();
        let errors=p.bindings.iter().map(|b|b.diagnostics.missing_error).sum::<usize>();
        format!("Updated curve bindings: {valid} valid X/Y rows; {missing} missing, {nonfinite} non-finite, {errors} missing error values.")
    }).unwrap_or_default();
    match plan.and_then(|p| app.commit_curve_bindings(p)) {
        Ok(()) => {
            ui.ctx().data_mut(|d| d.remove::<Draft>(key));
            ui.ctx().data_mut(|d| {
                d.insert_temp(
                    feedback_key(app.doc.automation_revision),
                    Feedback {
                        message: diagnostic.clone(),
                        failed: false,
                    },
                )
            });
            app.session.status = diagnostic;
            ui.ctx().request_repaint();
            true
        }
        Err(e) => {
            app.session.status = format!("Curve binding: {e}");
            ui.ctx().data_mut(|d| {
                d.insert_temp(
                    feedback_key(app.doc.automation_revision),
                    Feedback {
                        message: app.session.status.clone(),
                        failed: true,
                    },
                )
            });
            ui.ctx().request_repaint();
            false
        }
    }
}
