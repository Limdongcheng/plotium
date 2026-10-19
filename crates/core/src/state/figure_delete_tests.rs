use super::*;
use crate::automation::*;
use serde_json::{Value, json};

pub(super) fn fixture() -> PlotiumApp {
    let mut app = super::curve_tests::fixture(true);
    for i in 0..2 {
        let table = app.doc.datasets[0].as_table().unwrap();
        let edit = MaterializeCurveEdit {
            target: app.series_targets(0, app.doc.canvases[0].objects[i].id)[0].clone(),
            retain_y_column_id: table.series_bindings[0].value_column,
            table_revision: table.typed_state.envelope.revision.id,
        };
        let plan = app
            .plan_materialize_curves(DocumentRevision(app.doc.automation_revision), &[edit])
            .unwrap();
        app.commit_curve_bindings(plan).unwrap();
        let plot = app.doc.canvases[0].objects[i].plot_mut().unwrap();
        for s in &mut plot.binding.series {
            s.label = Some("same".into());
        }
        plot.axis_overrides.x_range = Some(AxisRange::new(0., 3.));
        plot.axis_overrides.y_range = Some(AxisRange::new(-1., 6.));
    }
    let page = &mut app.doc.canvases[0];
    page.x_viewport_links.push(XViewportLinkGroup {
        id: XViewportLinkId::new(),
        members: page.objects.iter().map(|o| o.id).collect(),
    });
    page.groups.push(LayoutGroup {
        id: 1,
        members: page
            .panels
            .iter()
            .map(|p| GroupMember::Panel(p.id))
            .collect(),
    });
    page.next_group_id = 2;
    page.create_panel("Empty".into(), ObjectFrame::new(5., 5., 10., 10.));
    app.rebuild_canvas(0);
    app.clear_history();
    app
}
pub(super) fn target(app: &PlotiumApp, i: usize) -> TargetRef {
    TargetRef::resource(canvas_object_ref(
        app.doc.canvases[0].resource_id,
        app.doc.canvases[0].objects[i].id,
    ))
}
pub(super) fn plan(app: &PlotiumApp, edits: Value) -> Result<FigureEditPlan, String> {
    app.plan_figure_edit(DocumentRevision(app.doc.automation_revision),&serde_json::from_value(json!({"canvas":TargetRef::resource(app.doc.canvases[0].resource_id.into()),"edits":edits})).unwrap())
}
fn snapshot(app: &PlotiumApp) -> Value {
    let p = &app.doc.canvases[0];
    json!({"objects":p.objects.iter().map(|o|json!({"id":o.id,"name":o.name,"frame":[o.frame.x,o.frame.y,o.frame.width,o.frame.height],"locked":o.locked,"visible":o.visible,
        "plot":o.plot().map(|p|json!({"binding":format!("{:?}",p.binding),"axes":format!("{:?}",p.axis_overrides),"viewport":format!("{:?}",p.viewport),"next":p.next_series_id}))})).collect::<Vec<_>>(),
        "panels":format!("{:?}",p.panels),"groups":format!("{:?}",p.groups),"links":format!("{:?}",p.x_viewport_links),"next":p.next_object_id,"group_next":p.next_group_id,"label_next":p.next_panel_label_slot,
        "selected":p.selected_object,"selection":format!("{:?}",app.session.ui.selection),"hierarchy":format!("{:?}",app.session.ui.hierarchical_selection),"assets":app.doc.assets.len(),"datasets":app.doc.datasets.len()})
}

#[test]
fn exact_delete_first_named_series_preserves_remaining_plot_and_coordinates() {
    let mut app = fixture();
    // The surviving curve uses ms/A input but stays in the original s/mA coordinates.
    let table = app.doc.datasets[0].as_table().unwrap();
    let r = &table.typed_state.envelope.revision;
    let columns = &r.snapshot.schema.columns;
    let rebind = CurveBindingEdit {
        target: app.series_targets(0, app.doc.canvases[0].objects[0].id)[1].clone(),
        source: TableCurveSource {
            dataset_id: table.resource_id,
            field_id: app.doc.datasets[0].default_field_id().unwrap(),
            table_id: r.table_id,
            table_revision: r.id,
            x_column_id: columns[7].id,
            y_column_id: columns[6].id,
            y_error_column_id: Some(columns[8].id),
        },
    };
    let p = app
        .plan_curve_bindings(DocumentRevision(app.doc.automation_revision), &[rebind])
        .unwrap();
    app.commit_curve_bindings(p).unwrap();
    app.clear_history();
    let before = app.doc.canvases[0].objects[0].plot().unwrap().clone();
    let sibling = app.doc.canvases[0].objects[1]
        .plot()
        .unwrap()
        .binding
        .clone();
    let t = app.series_targets(0, app.doc.canvases[0].objects[0].id)[0].clone();
    let initial = snapshot(&app);
    let rev = app.doc.automation_revision;
    let p = plan(&app, json!([{"op":"series_delete","target":t}])).unwrap();
    assert_eq!(p.result["deleted_targets"], json!([t]));
    assert_eq!(snapshot(&app), initial);
    app.commit_figure_edit(p).unwrap();
    let after = app.doc.canvases[0].objects[0].plot().unwrap();
    assert_eq!(after.binding.series, &before.binding.series[1..]);
    assert_eq!(after.binding.coordinates, before.binding.coordinates);
    assert_eq!(after.axis_overrides, before.axis_overrides);
    assert_eq!(after.viewport, before.viewport);
    assert_eq!(after.next_series_id, before.next_series_id);
    assert_eq!(
        after.figure().series[0].points,
        before.figure().series[1].points
    );
    assert_eq!(
        after.figure().series[0].color,
        before.figure().series[1].color
    );
    assert_eq!(
        app.doc.canvases[0].objects[1].plot().unwrap().binding,
        sibling
    );
    assert_eq!(app.doc.automation_revision, rev + 1);
    assert_eq!(app.session.undo_stack.len(), 1);
    let remaining = app.series_targets(0, app.doc.canvases[0].objects[0].id)[0].clone();
    let deleted = snapshot(&app);
    assert!(
        plan(&app, json!([{"op":"series_delete","target":remaining}]))
            .unwrap_err()
            .contains("last Series")
    );
    assert_eq!(snapshot(&app), deleted);
    app.undo();
    assert_eq!(snapshot(&app), initial);
    app.redo();
    assert_eq!(snapshot(&app), deleted);
}

#[test]
fn delete_plot_keeps_parent_dataset_and_restores_links_groups_selection() {
    let mut app = fixture();
    let id = app.doc.canvases[0].objects[0].id;
    app.select_content(0, id);
    let before = snapshot(&app);
    let parent = app.doc.canvases[0].parent_panel(id).unwrap();
    let p = plan(
        &app,
        json!([{"op":"content_delete","target":target(&app,0)}]),
    )
    .unwrap();
    assert_eq!(p.result["parent_changes"].as_array().unwrap().len(), 1);
    assert_eq!(
        p.result["viewport_link_changes"].as_array().unwrap().len(),
        1
    );
    assert!(!p.result["selection_change"].is_null());
    app.commit_figure_edit(p).unwrap();
    assert!(
        app.doc.canvases[0]
            .panel(parent)
            .unwrap()
            .item_order
            .is_empty()
    );
    assert_eq!(app.doc.canvases[0].objects.len(), 1);
    assert_eq!(app.doc.datasets.len(), 1);
    assert!(app.doc.canvases[0].x_viewport_links.is_empty());
    assert_eq!(app.doc.canvases[0].groups.len(), 1);
    assert!(app.session.ui.selection.objects().is_empty());
    app.doc.canvases[0].validate_structure().unwrap();
    let after = snapshot(&app);
    app.undo();
    assert_eq!(snapshot(&app), before);
    app.redo();
    assert_eq!(snapshot(&app), after);
}

#[test]
fn empty_panel_delete_prunes_only_necessary_group_and_restores_identity() {
    let mut app = fixture();
    let page = &mut app.doc.canvases[0];
    let empty = page.panels[2].id;
    page.groups.clear();
    page.groups.push(LayoutGroup {
        id: 8,
        members: vec![
            GroupMember::Panel(page.panels[1].id),
            GroupMember::Panel(empty),
        ],
    });
    page.next_group_id = 9;
    app.select_panel(0, empty);
    let before = snapshot(&app);
    let t = TargetRef::resource(panel_resource(
        &app.doc.canvases[0],
        &app.doc.canvases[0].panels[2],
    ));
    let p = plan(&app, json!([{"op":"panel_delete","target":t}])).unwrap();
    assert_eq!(
        p.result["group_changes"][0]["before"][1]["target"],
        json!(t)
    );
    app.commit_figure_edit(p).unwrap();
    assert!(app.doc.canvases[0].groups.is_empty());
    assert_eq!(app.doc.canvases[0].objects.len(), 2);
    app.undo();
    assert_eq!(snapshot(&app), before);
    app.redo();
    assert!(app.doc.canvases[0].panel(empty).is_none());
}

#[test]
fn invalid_deletion_batches_are_atomic_and_never_infer_cascade() {
    let mut app = fixture();
    let a = target(&app, 0);
    let b = target(&app, 1);
    let ts = app.series_targets(0, app.doc.canvases[0].objects[0].id);
    let panel = TargetRef::resource(panel_resource(
        &app.doc.canvases[0],
        &app.doc.canvases[0].panels[0],
    ));
    let mut stale = b.clone();
    stale.resource.local_id = Some("99999".into());
    let cases = vec![
        json!([{"op":"content_delete","target":a},{"op":"content_delete","target":stale}]),
        json!([{"op":"content_delete","target":a},{"op":"content_delete","target":a}]),
        json!([{"op":"content_delete","target":a},{"op":"series_delete","target":ts[0]}]),
        json!([{"op":"series_delete","target":ts[0]},{"op":"series_delete","target":ts[1]}]),
        json!([{"op":"content_delete","target":a},{"op":"panel_delete","target":panel}]),
        json!([{"op":"content_delete","target":a},{"op":"text_set","target":b,"text":"bad"}]),
        json!([{"op":"content_delete","target":ts[0]}]),
        json!([{"op":"series_delete","target":a}]),
    ];
    for edits in cases {
        let before = snapshot(&app);
        let rev = app.doc.automation_revision;
        assert!(plan(&app, edits).is_err());
        assert_eq!(snapshot(&app), before);
        assert_eq!(app.doc.automation_revision, rev);
        assert!(app.session.undo_stack.is_empty());
    }
    app.doc.canvases[0].objects[1].locked = true;
    let before = snapshot(&app);
    assert!(
        plan(
            &app,
            json!([{"op":"content_delete","target":a},{"op":"content_delete","target":b}])
        )
        .unwrap_err()
        .contains("locked")
    );
    assert_eq!(snapshot(&app), before);
    app.doc.canvases[0].objects[1].locked = false;
    app.doc.canvases[0].panels[0].locked = true;
    assert!(plan(&app, json!([{"op":"series_delete","target":ts[0]}])).is_err());
}

#[test]
fn stale_revision_selection_and_cross_canvas_do_not_delete() {
    let mut app = fixture();
    let t = target(&app, 0);
    let p = plan(&app, json!([{"op":"content_delete","target":t}])).unwrap();
    app.select_content(0, app.doc.canvases[0].objects[0].id);
    let before = snapshot(&app);
    assert!(
        app.commit_figure_edit(p)
            .unwrap_err()
            .contains("selection effect changed")
    );
    assert_eq!(snapshot(&app), before);
    let p = plan(&app, json!([{"op":"content_delete","target":t}])).unwrap();
    app.doc.automation_revision += 1;
    assert!(app.commit_figure_edit(p).unwrap_err().contains("stale"));
    assert_eq!(snapshot(&app), before);
    let mut other = app.doc.canvases[0].clone();
    other.resource_id = CanvasId::new();
    let foreign = TargetRef::resource(canvas_object_ref(other.resource_id, other.objects[0].id));
    app.doc.canvases.push(other);
    assert!(
        plan(
            &app,
            json!([{"op":"content_delete","target":t},{"op":"content_delete","target":foreign}])
        )
        .is_err()
    );
}

#[test]
fn native_legacy_delete_round_trip_restores_parent_links_and_all_selection() {
    let mut app = fixture();
    let id = app.doc.canvases[0].objects[0].id;
    let parent = app.doc.canvases[0].parent_panel(id).unwrap();
    app.select_content(0, id);
    let before = snapshot(&app);
    let action = crate::actions::Action::delete_object(&app, 0, id).unwrap();
    app.try_execute_action(action).unwrap();
    assert!(app.doc.canvases[0].panel(parent).is_none());
    assert!(app.doc.canvases[0].groups.is_empty());
    app.doc.canvases[0].validate_structure().unwrap();
    app.undo();
    assert_eq!(snapshot(&app), before);
    app.redo();
    assert!(app.doc.canvases[0].panel(parent).is_none());
    app.undo();
    let action = app.delete_panel_action(0, parent).unwrap();
    app.try_execute_action(action).unwrap();
    app.doc.canvases[0].validate_structure().unwrap();
    app.undo();
    assert_eq!(snapshot(&app), before);
}

#[test]
fn deletion_high_water_survives_new_objects_groups_and_series_after_undo() {
    let mut app = fixture();
    let t = target(&app, 0);
    let id = app.doc.canvases[0].objects[0].id;
    let p = plan(&app, json!([{"op":"content_delete","target":t}])).unwrap();
    app.commit_figure_edit(p).unwrap();
    let retired_object = app.doc.canvases[0].allocate_object_id();
    app.doc.canvases[0].next_group_id = 40;
    app.doc.canvases[0].next_panel_label_slot = 90;
    app.undo();
    assert!(app.doc.canvases[0].allocate_object_id() > retired_object);
    assert_eq!(app.doc.canvases[0].next_group_id, 40);
    assert_eq!(app.doc.canvases[0].next_panel_label_slot, 90);
    let retired_series = app.doc.canvases[0]
        .object_mut(id)
        .unwrap()
        .plot_mut()
        .unwrap()
        .allocate_series_id();
    app.redo();
    app.undo();
    let next = app.doc.canvases[0]
        .object_mut(id)
        .unwrap()
        .plot_mut()
        .unwrap()
        .allocate_series_id();
    assert!(next > retired_series);
}

#[test]
fn raw_native_delete_entry_and_composites_capture_relationships_at_execution() {
    let mut app = fixture();
    let before = snapshot(&app);
    let mut children = Vec::new();
    for (index, object) in app.doc.canvases[0].objects.iter().enumerate() {
        children.push(crate::actions::Action::DeleteObject {
            canvas: 0,
            index,
            object: Box::new(object.clone()),
            selection_before: app.session.ui.selection.clone(),
        });
    }
    app.try_execute_action(crate::actions::Action::Composite(children))
        .unwrap();
    app.doc.canvases[0].validate_structure().unwrap();
    assert!(app.doc.canvases[0].objects.is_empty());
    assert!(app.doc.canvases[0].x_viewport_links.is_empty());
    app.undo();
    assert_eq!(snapshot(&app), before);
    app.redo();
    assert!(app.doc.canvases[0].objects.is_empty());
}

#[test]
fn constructed_native_deletions_capture_each_composite_step() {
    use crate::actions::Action;
    let mut app = fixture();
    app.select_content(0, app.doc.canvases[0].objects[0].id);
    let before = snapshot(&app);
    let children = app.doc.canvases[0]
        .objects
        .iter()
        .map(|object| Action::delete_object(&app, 0, object.id).unwrap())
        .collect();
    app.try_execute_action(Action::Composite(children)).unwrap();
    let page = &app.doc.canvases[0];
    assert!(page.objects.is_empty());
    assert_eq!(page.panels.len(), 1);
    assert!(page.groups.is_empty());
    assert!(page.x_viewport_links.is_empty());
    page.validate_structure().unwrap();
    assert_eq!(app.session.undo_stack.len(), 1);
    let deleted = snapshot(&app);
    app.undo();
    assert_eq!(snapshot(&app), before);
    app.redo();
    assert_eq!(snapshot(&app), deleted);
}

#[test]
fn constructed_native_deletion_preserves_prior_composite_edits() {
    use crate::actions::Action;
    let mut app = fixture();
    let before = snapshot(&app);
    let survivor = &app.doc.canvases[0].objects[0];
    let id = survivor.id;
    let before_frame = app.doc.canvases[0].layout_frame(id).unwrap();
    let frame = ObjectFrame {
        x: before_frame.x + 8.,
        ..before_frame
    };
    let children = vec![
        Action::rename_object(
            0,
            id,
            survivor.name.clone(),
            "Edited before deletion".into(),
        ),
        Action::move_resize_object(0, id, before_frame, frame),
        Action::delete_object(&app, 0, app.doc.canvases[0].objects[1].id).unwrap(),
    ];
    app.try_execute_action(Action::Composite(children)).unwrap();
    assert_eq!(app.doc.canvases[0].objects.len(), 1);
    let survivor = app.doc.canvases[0].object(id).unwrap();
    assert_eq!(survivor.name, "Edited before deletion");
    assert_eq!(app.doc.canvases[0].layout_frame(id), Some(frame));
    app.doc.canvases[0].validate_structure().unwrap();
    let edited = snapshot(&app);
    app.undo();
    assert_eq!(snapshot(&app), before);
    app.redo();
    assert_eq!(snapshot(&app), edited);
}

#[test]
fn failing_constructed_native_composite_restores_prior_deletion() {
    use crate::actions::Action;
    let mut app = fixture();
    let id = app.doc.canvases[0].objects[0].id;
    app.select_content(0, id);
    let before = snapshot(&app);
    let revision = app.doc.automation_revision;
    let delete = Action::delete_object(&app, 0, id).unwrap();
    assert!(
        app.try_execute_action(Action::Composite(vec![delete.clone(), delete]))
            .is_err()
    );
    assert_eq!(snapshot(&app), before);
    assert_eq!(app.doc.automation_revision, revision);
    assert!(!app.can_undo());
}

#[test]
fn inactive_canvas_deletion_does_not_capture_or_restore_unrelated_live_selection() {
    let mut app = fixture();
    let target_id = app.doc.canvases[0].objects[0].id;
    app.select_content(0, target_id);
    let mut other = app.doc.canvases[0].clone();
    other.resource_id = CanvasId::new();
    app.doc.canvases.push(other);
    app.session.active_canvas = Some(1);
    app.select_content(1, target_id);
    let p = plan(
        &app,
        json!([{"op":"content_delete","target":target(&app,0)}]),
    )
    .unwrap();
    assert!(
        p.result["selection_change"]["before"]
            .get("objects")
            .is_none()
    );
    let sibling = app.doc.canvases[1].objects[1].id;
    app.select_content(1, sibling);
    app.commit_figure_edit(p).unwrap();
    assert_eq!(app.session.active_canvas, Some(1));
    assert_eq!(app.session.ui.selection, Selection::single(sibling));
    app.undo();
    assert_eq!(app.doc.canvases[0].selected_object, Some(target_id));
    assert_eq!(app.session.active_canvas, Some(1));
    assert_eq!(app.session.ui.selection, Selection::single(sibling));
    app.redo();
    assert_eq!(app.session.ui.selection, Selection::single(sibling));
}

#[test]
fn deleting_an_earlier_series_keeps_active_curve_by_identity() {
    let mut app = fixture();
    app.doc.canvases[0].objects[0]
        .plot_mut()
        .unwrap()
        .stack
        .active = Some(1);
    let targets = app.series_targets(0, app.doc.canvases[0].objects[0].id);
    let active = app.doc.canvases[0].objects[0]
        .plot()
        .unwrap()
        .binding
        .series[1]
        .id;
    let p = plan(&app, json!([{"op":"series_delete","target":targets[0]}])).unwrap();
    assert_eq!(
        p.result["remaining_series"][0]["active_series"],
        json!({"before":active,"after":active})
    );
    app.commit_figure_edit(p).unwrap();
    let plot = app.doc.canvases[0].objects[0].plot().unwrap();
    assert_eq!(plot.stack.active, Some(0));
    assert_eq!(plot.binding.series[0].id, active);
    app.undo();
    assert_eq!(
        app.doc.canvases[0].objects[0].plot().unwrap().stack.active,
        Some(1)
    );
}

#[test]
fn failing_raw_native_composite_restores_complete_prior_relationships() {
    let mut app = fixture();
    let before = snapshot(&app);
    let rev = app.doc.automation_revision;
    let object = app.doc.canvases[0].objects[0].clone();
    let mut missing = object.clone();
    missing.id = ObjectId::new(99999);
    let actions = [object, missing]
        .into_iter()
        .map(|object| crate::actions::Action::DeleteObject {
            canvas: 0,
            index: 0,
            object: Box::new(object),
            selection_before: app.session.ui.selection.clone(),
        })
        .collect();
    assert!(
        app.try_execute_action(crate::actions::Action::Composite(actions))
            .is_err()
    );
    assert_eq!(snapshot(&app), before);
    assert_eq!(app.doc.automation_revision, rev);
    assert!(!app.can_undo());
}
