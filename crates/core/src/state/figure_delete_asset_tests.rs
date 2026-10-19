use super::figure_delete_tests::{fixture, plan, target};
use super::*;
use crate::automation::*;
use serde_json::json;

#[test]
fn delete_shared_raster_preserves_bytes_other_content_and_offline_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.png");
    image::RgbaImage::from_pixel(4, 2, image::Rgba([15, 75, 150, 255]))
        .save(&source)
        .unwrap();
    let source_bytes = std::fs::read(&source).unwrap();
    let mut app = fixture();
    let canvas = TargetRef::resource(app.doc.canvases[0].resource_id.into());
    let p=plan(&app,json!([
        {"op":"image_create","parent":canvas,"name":"shared A","frame_pt":[5,5,40,20],"path":source},
        {"op":"image_create","parent":canvas,"name":"shared B","frame_pt":[55,5,40,20],"path":source}
    ])).unwrap();
    app.commit_figure_edit(p).unwrap();
    app.clear_history();
    let id = app.doc.canvases[0].objects[2].id;
    let ContentKind::RasterImage(ref img) = app.doc.canvases[0].objects[2].kind else {
        panic!()
    };
    let asset = img.asset;
    let before = app.doc.assets[&asset].clone();
    let p = plan(
        &app,
        json!([{"op":"content_delete","target":target(&app,2)}]),
    )
    .unwrap();
    assert_eq!(p.result["retained_asset_ids"], json!([asset]));
    app.commit_figure_edit(p).unwrap();
    assert!(app.doc.canvases[0].object(id).is_none());
    assert_eq!(app.doc.assets[&asset], before);
    let ContentKind::RasterImage(ref other) = app.doc.canvases[0].objects[2].kind else {
        panic!()
    };
    assert_eq!(other.asset, asset);
    assert_eq!(std::fs::read(&source).unwrap(), source_bytes);
    let project = dir.path().join("deleted.plotium");
    crate::project::save_project(&app, &project, false).unwrap();
    // The source remains untouched; reopening needs only the embedded asset.
    std::fs::rename(&source, dir.path().join("offline-source.png")).unwrap();
    let reopened = crate::project::load_project(&project).unwrap();
    assert_eq!(reopened.doc.assets[&asset], before);
    assert_eq!(reopened.doc.canvases[0].objects.len(), 3);
    let png = crate::export::ExportSettings {
        format: crate::export::ExportFormat::Png,
        scope: crate::export::ExportPageScope::All,
        dpi: 72,
        target_width_mm: None,
        trim_to_visible_content: false,
        allow_missing_images: false,
    };
    crate::export::export_canvases_with_assets(
        &reopened.doc.canvases,
        &reopened.doc.assets,
        Some(0),
        &png,
        &dir.path().join("offline"),
    )
    .unwrap();
    app.undo();
    assert!(app.doc.canvases[0].object(id).is_some());
    assert_eq!(app.doc.assets[&asset], before);
    app.redo();
    assert!(app.doc.canvases[0].object(id).is_none());
}

#[test]
fn explicit_derived_table_series_delete_does_not_modify_immutable_source_or_recipe() {
    use plotium_data::{RelPlanV1, Relation, SnapshotRead, TableId};
    let mut app = fixture();
    let input = &app.doc.datasets[0].as_table().unwrap().typed_state;
    let rev = &input.envelope.revision;
    let recipe = RelPlanV1::new(Relation::SnapshotRead(SnapshotRead {
        table: rev.table_id,
        revision: rev.id,
        fingerprint: rev.snapshot.fingerprint,
    }));
    let output = execute_typed_plan(
        recipe.clone(),
        &[input],
        TableId::new(),
        16 * 1024 * 1024,
        &std::collections::BTreeSet::new(),
    )
    .unwrap();
    let mut derived = app.doc.datasets[0].as_table().unwrap().clone();
    derived.resource_id = DatasetId::new();
    derived.typed_state = output;
    derived.lineage = Some(DatasetLineage::new(
        DerivationKind::RelationalTransform,
        [app.doc.datasets[0].resource_id()],
    ));
    let table_id = derived.typed_state.envelope.revision.table_id;
    let dataset_id = derived.resource_id;
    app.doc.datasets.push(Dataset::Table(Box::new(derived)));
    let field = app.doc.datasets[1].default_field_id().unwrap();
    for s in &mut app.doc.canvases[0].objects[0]
        .plot_mut()
        .unwrap()
        .binding
        .series
    {
        s.source.resource = dataset_id;
        s.source.field = field;
        let SeriesSelection::TableColumns(ref mut c) = s.source.selection else {
            panic!()
        };
        c.table_id = table_id;
    }
    app.rebuild_canvas(0);
    let before =
        serde_json::to_value(&app.doc.datasets[1].as_table().unwrap().typed_state.envelope)
            .unwrap();
    let ts = app.series_targets(0, app.doc.canvases[0].objects[0].id);
    let p = plan(&app, json!([{"op":"series_delete","target":ts[0]}])).unwrap();
    app.commit_figure_edit(p).unwrap();
    assert_eq!(
        app.doc.canvases[0].objects[0]
            .plot()
            .unwrap()
            .binding
            .series[0]
            .source
            .resource,
        dataset_id
    );
    assert_eq!(
        serde_json::to_value(&app.doc.datasets[1].as_table().unwrap().typed_state.envelope)
            .unwrap(),
        before
    );
    assert_eq!(
        app.doc.datasets[1]
            .as_table()
            .unwrap()
            .typed_state
            .envelope
            .revision
            .operation
            .plan,
        Some(recipe)
    );
    assert_eq!(app.doc.datasets.len(), 2);
}

#[test]
fn hidden_shapes_and_nonfree_parents_delete_without_reflow_or_new_geometry_gates() {
    let mut app = fixture();
    let page = &mut app.doc.canvases[0];
    let parent = page.panels[0].id;
    page.panel_mut(parent).unwrap().layout = PanelLayout::VerticalStack;
    page.auto_height = true;
    for kind in [
        ShapeKind::Rect,
        ShapeKind::Ellipse,
        ShapeKind::Line,
        ShapeKind::Arrow,
    ] {
        let id = page.allocate_object_id();
        page.objects.push(ContentItem {
            id,
            name: "hidden".into(),
            frame: ObjectFrame::new(0., 0., 20., 10.),
            locked: false,
            visible: false,
            kind: ContentKind::Shape(ShapeObject::new(kind)),
        });
        page.panel_mut(parent).unwrap().item_order.push(id);
    }
    let ids = app.doc.canvases[0].objects[2..]
        .iter()
        .map(|o| o.id)
        .collect::<Vec<_>>();
    let sibling = app.doc.canvases[0].objects[0].frame;
    let edits=ids.iter().map(|id|json!({"op":"content_delete","target":TargetRef::resource(canvas_object_ref(app.doc.canvases[0].resource_id,*id))})).collect::<Vec<_>>();
    let p = plan(&app, json!(edits)).unwrap();
    assert!(!p.result["auto_height"].is_null());
    app.commit_figure_edit(p).unwrap();
    assert_eq!(app.doc.canvases[0].objects[0].frame, sibling);
    assert_eq!(
        app.doc.canvases[0].panels[0].layout,
        PanelLayout::VerticalStack
    );
    app.undo();
    assert_eq!(app.doc.canvases[0].objects.len(), 6);
    app.redo();
    assert_eq!(app.doc.canvases[0].objects.len(), 2);
}

#[test]
fn default_and_dynamic_series_are_rejected_without_materializing() {
    let mut app = super::curve_tests::fixture(true);
    let original = app.doc.canvases[0].objects[0]
        .plot()
        .unwrap()
        .binding
        .clone();
    let ts = app.series_targets(0, app.doc.canvases[0].objects[0].id);
    assert!(
        plan(&app, json!([{"op":"series_delete","target":ts[0]}]))
            .unwrap_err()
            .contains("table_default")
    );
    assert_eq!(
        app.doc.canvases[0].objects[0].plot().unwrap().binding,
        original
    );
    app = fixture();
    let dataset = app.doc.datasets[0].resource_id();
    app.doc.canvases[0].objects[0]
        .plot_mut()
        .unwrap()
        .display_owner = Some(dataset);
    let ts = app.series_targets(0, app.doc.canvases[0].objects[0].id);
    assert!(plan(&app, json!([{"op":"series_delete","target":ts[0]}])).is_err());
}
