use super::*;
use crate::automation::*;
use serde_json::{Value, json};

fn fixture() -> PlotiumApp {
    let mut a = super::curve_tests::fixture(true);
    let page = &mut a.doc.canvases[0];
    let parent = page.panels[0].id;
    for (name, kind, parented) in [
        (
            "same",
            ContentKind::Text(TextBox::label("Long caption".into())),
            true,
        ),
        (
            "same",
            ContentKind::Text(TextBox::label("Loose caption".into())),
            false,
        ),
        (
            "image",
            ContentKind::RasterImage(RasterImageContent::new(AssetId::new())),
            true,
        ),
        (
            "shared image",
            ContentKind::RasterImage(RasterImageContent::new(AssetId::new())),
            false,
        ),
    ] {
        let id = page.allocate_object_id();
        page.objects.push(ContentItem {
            id,
            name: name.into(),
            frame: ObjectFrame::new(5., 6., 40., 20.),
            locked: false,
            visible: true,
            kind,
        });
        if parented {
            page.panel_mut(parent).unwrap().item_order.push(id);
        }
    }
    for shape in [
        ShapeKind::Rect,
        ShapeKind::Ellipse,
        ShapeKind::Line,
        ShapeKind::Arrow,
    ] {
        let id = page.allocate_object_id();
        page.objects.push(ContentItem {
            id,
            name: "shape".into(),
            frame: ObjectFrame::new(10., 10., 30., 20.),
            locked: false,
            visible: true,
            kind: ContentKind::Shape(ShapeObject::new(shape)),
        });
    }
    let plot = page.objects[0].plot_mut().unwrap();
    plot.axis_overrides.x_range = Some(AxisRange::new(0., 3.));
    plot.axis_overrides.y_range = Some(AxisRange::new(-1., 6.));
    a.rebuild_canvas(0);
    a.clear_history();
    a
}
fn target(a: &PlotiumApp, index: usize) -> TargetRef {
    let p = &a.doc.canvases[0];
    TargetRef::resource(canvas_object_ref(p.resource_id, p.objects[index].id))
}
fn plan(a: &PlotiumApp, edits: Value) -> Result<FigureEditPlan, String> {
    a.plan_figure_edit(DocumentRevision(a.doc.automation_revision),&serde_json::from_value(json!({"canvas":TargetRef::resource(a.doc.canvases[0].resource_id.into()),"edits":edits})).unwrap())
}
fn params(a: &PlotiumApp, index: usize, f: [f32; 4]) -> Value {
    json!({"op":"content_frame","target":target(a,index),"frame_pt":f})
}
fn image(a: &PlotiumApp, index: usize) -> &RasterImageContent {
    let ContentKind::RasterImage(i) = &a.doc.canvases[0].objects[index].kind else {
        panic!()
    };
    i
}
fn install_images(a: &mut PlotiumApp, path: &std::path::Path) {
    let asset = super::figure_edit_image::prepare_image(path, &mut 0).unwrap();
    let mut raster = RasterImageContent::new(asset.id);
    raster.crop = [0.1, 0.2, 0.8, 0.9];
    raster.fit = ImageFit::Cover;
    raster.opacity = 0.6;
    raster.rotation = QuarterTurn::Clockwise90;
    raster.interpolation = ImageInterpolation::Nearest;
    a.doc.canvases[0].objects[4].kind = ContentKind::RasterImage(raster.clone());
    a.doc.canvases[0].objects[5].kind = ContentKind::RasterImage(raster);
    a.doc.assets.insert(asset.id, asset);
}

#[test]
fn content_frames_edit_only_items_and_preserve_plot_state_and_shapes() {
    let mut a = fixture();
    // Exercise a loose plot as well as one inside a panel.
    let parent = a.doc.canvases[0].panels[1].id;
    a.doc.canvases[0].panels.retain(|p| p.id != parent);
    let before = a.doc.canvases[0].clone();
    let mut edits = vec![];
    for i in [0, 1, 2, 3, 6, 7, 8, 9] {
        edits.push(params(&a, i, [-8., 12., 105., 31.]));
    }
    let p = plan(&a, json!(edits)).unwrap();
    assert_eq!(
        p.result["content_edits"][0]["frame_space"],
        "panel_local_pt"
    );
    assert_eq!(p.result["content_edits"][1]["frame_space"], "page_pt");
    assert_eq!(p.result["content_edits"][0]["page_frame"]["x"], 0.);
    assert_eq!(a.doc.canvases[0].objects[0].frame, before.objects[0].frame);
    a.commit_figure_edit(p).unwrap();
    let page = &a.doc.canvases[0];
    assert_eq!(page.panels, before.panels);
    assert_eq!(page.groups, before.groups);
    assert_eq!(page.objects[4].frame, before.objects[4].frame);
    assert_eq!(page.objects[5].frame, before.objects[5].frame);
    for i in [0, 1] {
        let plot = page.objects[i].plot().unwrap();
        let old = before.objects[i].plot().unwrap();
        assert_eq!(plot.binding, old.binding);
        assert_eq!(plot.next_series_id, old.next_series_id);
        assert_eq!(plot.axis_overrides, old.axis_overrides);
        assert_eq!(
            serde_json::to_value(&plot.figure().x).unwrap(),
            serde_json::to_value(&old.figure().x).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&plot.figure().y).unwrap(),
            serde_json::to_value(&old.figure().y).unwrap()
        );
        assert_eq!(plot.viewport, old.viewport);
        assert_eq!(page.objects[i].id, before.objects[i].id);
    }
    assert_eq!(page.objects[2].text(), before.objects[2].text());
    for i in 6..10 {
        assert_eq!(page.objects[i].shape(), before.objects[i].shape());
    }
    let high = page.next_object_id;
    a.undo();
    for (o, old) in a.doc.canvases[0].objects.iter().zip(&before.objects) {
        assert_eq!(o.frame, old.frame);
        assert_eq!(o.id, old.id);
    }
    assert!(!a.can_undo());
    a.redo();
    assert_eq!(a.doc.canvases[0].next_object_id, high);
    assert_eq!(
        a.doc.canvases[0].objects[2].frame,
        ObjectFrame::new(-8., 12., 105., 31.)
    );
}

#[test]
fn content_frame_aspect_reports_effective_size_and_never_inherits_panel_scope() {
    let mut a = fixture();
    a.doc.canvases[0].objects[2].locked = true; // unrelated sibling
    let before = a.doc.canvases[0].clone();
    let p = plan(
        &a,
        json!([
            params(&a, 4, [-10., -12., 40., 25.]),
            params(&a, 5, [20., 30., 40., 25.])
        ]),
    )
    .unwrap();
    assert_eq!(
        p.result["content_edits"][0]["requested_frame_pt"],
        json!([-10., -12., 40., 25.])
    );
    assert_eq!(
        p.result["content_edits"][0]["effective_frame_pt"],
        json!([-10., -12., 50., 25.])
    );
    a.commit_figure_edit(p).unwrap();
    assert_eq!(a.doc.canvases[0].panels, before.panels);
    assert_eq!(a.doc.canvases[0].objects[2].frame, before.objects[2].frame);
    for i in [4, 5] {
        assert_eq!(a.doc.canvases[0].objects[i].frame.width, 50.);
    }
    let ContentKind::RasterImage(r) = &mut a.doc.canvases[0].objects[4].kind else {
        panic!()
    };
    r.preserve_aspect = false;
    r.fit = ImageFit::Stretch;
    let p = plan(&a, json!([params(&a, 4, [1., 2., 1., 2.])])).unwrap();
    assert_eq!(
        p.result["content_edits"][0]["effective_frame_pt"],
        json!([1., 2., 1., 2.])
    );
}

#[test]
fn image_replace_freezes_deduplicates_retains_attributes_and_roundtrips_offline() {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("old.png");
    let new = dir.path().join("new.jpg");
    image::RgbImage::from_pixel(20, 10, image::Rgb([0, 80, 160]))
        .save(&old)
        .unwrap();
    image::RgbImage::from_pixel(8, 16, image::Rgb([220, 100, 10]))
        .save(&new)
        .unwrap();
    let mut a = fixture();
    install_images(&mut a, &old);
    let before = a.doc.canvases[0].clone();
    let raster = image(&a, 4).clone();
    let old_assets = a.doc.assets.clone();
    let p=plan(&a,json!([params(&a,4,[-10.,-12.,80.,40.]),{"op":"image_replace","target":target(&a,4),"path":new}])).unwrap();
    let value = p.result.clone();
    let new_id: AssetId =
        serde_json::from_value(value["content_edits"][1]["asset_change"]["after"].clone()).unwrap();
    std::fs::remove_file(&new).unwrap();
    std::fs::write(&old, b"changed").unwrap();
    a.commit_figure_edit(p).unwrap();
    let mut expected = raster.clone();
    expected.asset = new_id;
    assert_eq!(image(&a, 4), &expected);
    assert_eq!(image(&a, 5), &raster);
    assert_eq!(a.doc.canvases[0].objects[4].id, before.objects[4].id);
    assert_eq!(a.doc.canvases[0].panels, before.panels);
    assert_eq!(a.doc.assets.len(), 2);
    assert_eq!(a.doc.assets[&raster.asset], old_assets[&raster.asset]);
    let project = dir.path().join("offline.plotium");
    crate::project::save_project(&a, &project, false).unwrap();
    let loaded = crate::project::load_project(&project).unwrap();
    assert_eq!(loaded.doc.assets, a.doc.assets);
    let ContentKind::RasterImage(reopened) = &loaded.doc.canvases[0].objects[4].kind else {
        panic!()
    };
    assert_eq!(reopened, &expected);
    a.undo();
    assert_eq!(a.doc.assets, old_assets);
    assert_eq!(image(&a, 4), &raster);
    assert_eq!(a.doc.canvases[0].objects[4].frame, before.objects[4].frame);
    a.redo();
    assert_eq!(image(&a, 4), &expected);
    assert_eq!(
        a.doc.canvases[0].objects[4].frame,
        ObjectFrame::new(-10., -12., 80., 40.)
    );
    let same = dir.path().join("same.jpg");
    std::fs::write(&same, &a.doc.assets[&new_id].bytes).unwrap();
    let p=plan(&a,json!([{"op":"image_replace","target":target(&a,5),"path":same},{"op":"image_create","parent":TargetRef::resource(a.doc.canvases[0].resource_id.into()),"name":"dedup","frame_pt":[0,0,40,20],"path":same}])).unwrap();
    a.commit_figure_edit(p).unwrap();
    assert_eq!(a.doc.assets.len(), 2);
    assert_eq!(image(&a, 5).asset, new_id);
}

#[test]
fn content_edits_bad_batches_and_forged_targets_leave_no_partial_state() {
    let mut a = fixture();
    for size in [
        [0., 0., 0., 2.],
        [0., 0., 0.5, 2.],
        [f32::MAX, 0., f32::MAX, 2.],
    ] {
        assert!(
            plan(
                &a,
                json!([params(&a, 3, [1., 2., 50., 20.]), params(&a, 2, size)])
            )
            .is_err()
        );
    }
    let original = a.doc.canvases[0].objects[3].frame;
    for key in ["id", "kind", "parent_id", "local_id"] {
        let mut t = serde_json::to_value(target(&a, 3)).unwrap();
        t["resource"][key] = json!("forged");
        assert!(plan(&a,json!([params(&a,2,[1.,2.,50.,20.]),{"op":"content_frame","target":t,"frame_pt":[1,2,50,20]}])).is_err());
    }
    let id = a.doc.canvases[0].objects[3].id;
    for size in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(
            a.doc.canvases[0]
                .set_content_frame(
                    id,
                    ObjectFrame {
                        x: size,
                        y: 0.,
                        width: 40.,
                        height: 20.
                    }
                )
                .is_err()
        );
    }
    a.doc.canvases[0].objects[3].locked = true;
    assert!(
        plan(
            &a,
            json!([
                params(&a, 2, [1., 2., 50., 20.]),
                params(&a, 3, [1., 2., 50., 20.])
            ])
        )
        .is_err()
    );
    a.doc.canvases[0].objects[3].locked = false;
    a.doc.canvases[0].panels[0].locked = true;
    assert!(plan(&a, json!([params(&a, 2, [1., 2., 50., 20.])])).is_err());
    a.doc.canvases[0].panels[0].locked = false;
    a.doc.canvases[0].panels[0].layout = PanelLayout::VerticalStack;
    assert!(plan(&a, json!([params(&a, 2, [1., 2., 50., 20.])])).is_err());
    a.doc.canvases[0].auto_height = true;
    assert!(plan(&a, json!([params(&a, 3, [1., 2., 50., 20.])])).is_err());
    assert_eq!(a.doc.canvases[0].objects[3].frame, original);
    assert!(!a.can_undo());
    assert!(a.doc.assets.is_empty());
}

#[test]
fn image_replace_shares_creation_budgets_and_does_not_reset_incompatible_pages() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("new.png");
    image::RgbImage::from_pixel(4, 8, image::Rgb([10, 20, 30]))
        .save(&path)
        .unwrap();
    let mut a = fixture();
    install_images(&mut a, &path);
    a.doc.canvases[0].auto_height = true;
    a.doc.canvases[0].panels[0].layout = PanelLayout::VerticalStack;
    let edit = json!({"op":"image_replace","target":target(&a,4),"path":path});
    assert!(plan(&a, json!([edit])).is_ok());
    assert!(
        plan(&a, json!(vec![edit.clone(); 9]))
            .unwrap_err()
            .contains("8 local images")
    );
    let ContentKind::RasterImage(r) = &mut a.doc.canvases[0].objects[4].kind else {
        panic!()
    };
    r.page_index = 1;
    assert!(
        plan(&a, json!([edit]))
            .unwrap_err()
            .contains("page_index 0")
    );
    a.doc.canvases[0].auto_height = false;
    for bytes in [vec![0; 1024 * 1024 + 1], b"corrupt".to_vec()] {
        std::fs::write(&path, bytes).unwrap();
        assert!(plan(&a,json!([params(&a,3,[1.,2.,50.,20.]),{"op":"image_replace","target":target(&a,5),"path":path}])).is_err());
        assert!(!a.can_undo());
    }
}

#[test]
fn content_final_frames_include_later_explicit_parent_resize() {
    let a = fixture();
    let page = &a.doc.canvases[0];
    let parent = TargetRef::resource(panel_resource(page, &page.panels[0]));
    let before = page.panels[0].frame;
    let p=plan(&a,json!([
        params(&a,2,[1.,2.,40.,20.]),
        {"op":"panel_frame","target":parent,"frame_pt":[before.x,before.y,before.width*2.,before.height*3.]}
    ])).unwrap();
    assert_eq!(
        p.result["content_edits"][0]["effective_frame_pt"],
        json!([1., 2., 40., 20.])
    );
    assert_eq!(
        p.result["content_frames"][0]["effective_frame_pt"],
        json!([2., 6., 80., 60.])
    );
}

#[test]
fn content_frame_rejects_page_overflow_from_later_parent_move_atomically() {
    let mut a = fixture();
    a.doc.canvases[0].panels[0].frame.x = 0.;
    a.doc.canvases[0].panels[0].frame.y = 0.;
    let page = &a.doc.canvases[0];
    let parent = TargetRef::resource(panel_resource(page, &page.panels[0]));
    let before = page.clone();
    let size = page.panels[0].frame;
    let revision = a.doc.automation_revision;
    let assets = a.doc.assets.clone();
    // Exercise both page origins and far edges, in both coordinate axes.
    for (frame, offset) in [
        ([2e38, 0., 1e38, 20.], [2e38, 0.]),
        ([1e38, 0., 1e38, 20.], [2e38, 0.]),
        ([0., 2e38, 20., 1e38], [0., 2e38]),
        ([0., 1e38, 20., 1e38], [0., 2e38]),
    ] {
        let content = params(&a, 2, frame);
        let panel = json!({"op":"panel_frame","target":parent,
            "frame_pt":[offset[0],offset[1],size.width,size.height]});
        assert!(plan(&a, json!([content])).is_ok());
        assert!(plan(&a, json!([panel])).is_ok());
        let error = plan(
            &a,
            json!([params(&a, 3, [1., 2., 50., 20.]), content, panel]),
        )
        .unwrap_err();
        assert!(error.contains("finite"), "{error}");
        let page = &a.doc.canvases[0];
        assert_eq!(page.panels, before.panels);
        assert_eq!(page.objects.len(), before.objects.len());
        for (item, old) in page.objects.iter().zip(&before.objects) {
            assert_eq!(item.id, old.id);
            assert_eq!(item.frame, old.frame);
        }
        assert_eq!(a.doc.automation_revision, revision);
        assert_eq!(a.doc.assets, assets);
        assert!(!a.can_undo());
        assert!(!a.can_redo());
    }
    // Finite, off-canvas page geometry remains valid and is reported without clamping.
    let p = plan(
        &a,
        json!([
            params(&a, 2, [1e37, 0., 1e37, 20.]),
            {"op":"panel_frame","target":parent,
                "frame_pt":[1e37,0.,size.width,size.height]}
        ]),
    )
    .unwrap();
    assert_eq!(
        p.result["content_edits"][0]["page_frame"]["x"],
        json!(1e37_f32)
    );
    assert_eq!(
        p.result["content_frames"][0]["page_frame"]["x"],
        json!(2e37_f32)
    );
}

#[test]
fn content_image_encoded_batch_and_decoded_budgets_are_shared() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("padded.png");
    image::RgbImage::from_pixel(4, 8, image::Rgb([10, 20, 30]))
        .save(&path)
        .unwrap();
    let mut a = fixture();
    install_images(&mut a, &path);
    let mut bytes = std::fs::read(&path).unwrap();
    bytes.resize(900_000, 0);
    std::fs::write(&path, bytes).unwrap();
    let edit = json!({"op":"image_replace","target":target(&a,4),"path":path});
    let error = plan(&a, json!(vec![edit; 5])).unwrap_err();
    assert!(error.contains("4 MiB per batch"), "{error}");
    image::RgbImage::from_pixel(1025, 1025, image::Rgb([10, 20, 30]))
        .save(&path)
        .unwrap();
    let error = plan(
        &a,
        json!([{"op":"image_replace","target":target(&a,4),"path":path}]),
    )
    .unwrap_err();
    assert!(error.contains("4 MiB decoded"), "{error}");
    let bmp = dir.path().join("unsupported.bmp");
    image::RgbImage::from_pixel(4, 8, image::Rgb([10, 20, 30]))
        .save(&bmp)
        .unwrap();
    assert!(
        plan(
            &a,
            json!([{"op":"image_replace","target":target(&a,4),"path":bmp}])
        )
        .is_err()
    );
    assert!(
        plan(
            &a,
            json!([{"op":"image_replace","target":target(&a,4),"path":dir.path()}])
        )
        .is_err()
    );
    assert!(
        plan(
            &a,
            json!([{"op":"image_replace","target":target(&a,4),"path":"relative.png"}])
        )
        .is_err()
    );
    assert!(!a.can_undo());
    assert_eq!(a.doc.assets.len(), 1);
}
