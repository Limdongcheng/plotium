use super::*;
use crate::state::*;
use plotium_data::{ColumnChunk, ColumnSchema, ColumnValues, LogicalType};

pub(crate) fn fixture() -> PlotiumApp {
    let mut app = PlotiumApp::new();
    for offset in [0.0, 5.0] {
        let columns: Vec<_> = ["x", "y_left", "error", "y_right"]
            .iter()
            .map(|name| ColumnSchema::new(*name, LogicalType::Float64))
            .collect();
        let x = columns[0].id;
        let bindings = vec![
            TableSeriesBinding {
                value_column: columns[1].id,
                uncertainty_column: Some(columns[2].id),
                fit: None,
            },
            TableSeriesBinding {
                value_column: columns[3].id,
                uncertainty_column: None,
                fit: None,
            },
        ];
        let data = [
            vec![0.0, 1.0, 2.0],
            vec![1.0 + offset, 2.0 + offset, 4.0 + offset],
            vec![0.1, 0.2, 0.3],
            vec![2.0 + offset, 3.0 + offset, 5.0 + offset],
        ];
        let mut table = TableDataset::from_materialized(
            columns
                .into_iter()
                .zip(data)
                .map(|(c, v)| (c, ColumnChunk::all_valid(ColumnValues::Float64(v))))
                .collect(),
            vec![],
            Some(x),
            bindings,
            "plotium.test.r1",
        )
        .unwrap();
        table.name = Some("Synthetic table".into());
        table.board_sheet_visible = false;
        app.doc.datasets.push(Dataset::Table(Box::new(table)));
    }
    let mut canvas = CanvasDocument::new("R1 synthetic acceptance".into(), [180.0, 90.0]);
    for left in [0.0, 250.0] {
        let id = canvas.allocate_object_id();
        let mut object = app.build_plot_object(
            0,
            ObjectFrame::new(left, 0.0, 235.0, 210.0),
            id,
            "Same plot".into(),
        );
        let plot = object.plot_mut().unwrap();
        let mut second = SeriesBinding::from_dataset(&app.doc.datasets[1]).unwrap();
        second.id = plot.allocate_series_id();
        plot.binding.series.push(second);
        for series in &mut plot.binding.series {
            series.label = Some("Same curve".into());
        }
        canvas.objects.push(object);
        canvas.create_panel_for_plot(id).unwrap();
    }
    let id = canvas.allocate_object_id();
    canvas.objects.push(ContentItem {
        id,
        name: "Loose caption".into(),
        frame: ObjectFrame::new(10.0, 220.0, 200.0, 20.0),
        locked: false,
        visible: true,
        kind: ContentKind::Text(TextBox::label("Synthetic R1 fixture".into())),
    });
    app.doc.canvases.push(canvas);
    app.session.active_canvas = Some(0);
    app.set_active_dataset(Some(0));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.compute_busy() && std::time::Instant::now() < deadline {
        app.poll_compute();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(!app.compute_busy());
    app
}
fn query(
    app: &PlotiumApp,
    target: TargetRef,
    view: DocumentView,
    offset: usize,
    limit: usize,
) -> ToolRequest {
    ToolRequest {
        tool_id: DOCUMENT_INSPECT.into(),
        tool_version: 1,
        parameters: json!(DocumentInspectParams {
            target: target.clone(),
            view,
            offset,
            limit
        }),
        targets: if target.component.is_some() {
            TargetSelector::Components {
                targets: vec![target],
            }
        } else {
            TargetSelector::Explicit {
                ids: vec![target.resource.id],
            }
        },
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Agent,
    }
}
fn read(
    app: &mut PlotiumApp,
    target: TargetRef,
    view: DocumentView,
    offset: usize,
    limit: usize,
) -> ToolResult {
    let p = plan_tool(app, query(app, target, view, offset, limit)).unwrap();
    execute_tool(app, p, ExecutionAuthority::Read).unwrap()
}
fn targets(app: &PlotiumApp) -> Vec<TargetRef> {
    app.doc.canvases[0]
        .objects
        .iter()
        .flat_map(|o| app.series_targets(0, o.id))
        .collect()
}
#[test]
fn traverse_same_named_panels_objects_series_and_actual_source_columns() {
    let mut app = fixture();
    let root = read(
        &mut app,
        document_root_target(),
        DocumentView::Children,
        0,
        50,
    );
    let canvas: TargetRef =
        serde_json::from_value(root.value["items"][0]["target"].clone()).unwrap();
    let list = read(&mut app, canvas, DocumentView::Children, 0, 50);
    assert_eq!(list.value["total"], 3);
    assert_eq!(list.value["items"][2]["kind"], "text");
    let mut found = vec![];
    for panel in list.value["items"].as_array().unwrap().iter().take(2) {
        let p: TargetRef = serde_json::from_value(panel["target"].clone()).unwrap();
        let object = read(&mut app, p.clone(), DocumentView::Children, 0, 50);
        assert_eq!(object.value["items"][0]["name"], "Same plot");
        let o: TargetRef =
            serde_json::from_value(object.value["items"][0]["target"].clone()).unwrap();
        let curves = read(&mut app, o.clone(), DocumentView::Children, 0, 50);
        assert_eq!(curves.value["total"], 2);
        for (index, curve) in curves.value["items"].as_array().unwrap().iter().enumerate() {
            let t: TargetRef = serde_json::from_value(curve["target"].clone()).unwrap();
            let detail = read(&mut app, t.clone(), DocumentView::SelfOnly, 0, 50);
            let n = &detail.value["node"];
            assert_eq!(n["ancestors"][2], json!(p));
            assert_eq!(n["ancestors"][3], json!(o));
            assert_eq!(
                n["source"]["dataset"],
                json!(ResourceRef::from(app.doc.datasets[index].resource_id()))
            );
            assert_eq!(
                n["source"]["field"]["local_id"],
                app.doc.datasets[index]
                    .field_descriptor(app.doc.datasets[index].default_field_id().unwrap())
                    .unwrap()
                    .local_id
            );
            assert_eq!(n["source"]["status"], "resolved");
            assert_eq!(n["source"]["binding_mode"], "table_default");
            assert_eq!(n["source"]["per_plot_explicit_binding_supported"], false);
            assert_eq!(n["source"]["series_columns"].as_array().unwrap().len(), 2);
            assert!(!n["source"]["series_columns"][0]["error_column"].is_null());
            let source_id = n["source"]["field"]["id"].as_str().unwrap();
            assert!(
                ProjectResourceProvider::new(&app)
                    .inspect(source_id)
                    .is_some()
            );
            assert_eq!(
                app.doc.automation_runs.last().unwrap().nodes[0]
                    .frozen_targets
                    .component_targets,
                vec![t.clone()]
            );
            found.push(t);
        }
    }
    assert_eq!(found, targets(&app));
    app.doc.canvases[0].objects.swap(0, 1);
    let result = read(&mut app, found[0].clone(), DocumentView::SelfOnly, 0, 50);
    assert_eq!(result.value["node"]["target"], json!(found[0]));
}
#[test]
fn invalid_identity_and_stale_revision_never_fall_back() {
    let mut app = fixture();
    let target = targets(&app)[0].clone();
    let mut cases = vec![];
    for key in ["id", "kind", "parent_id", "local_id"] {
        let mut t = target.clone();
        let mut r = json!(t.resource);
        r[key] = json!("forged");
        t.resource = serde_json::from_value(r).unwrap();
        cases.push(t);
    }
    let mut missing = target.clone();
    missing.component = Some(ComponentRef::Series(SeriesId::new(999)));
    cases.push(missing);
    let mut root = document_root_target();
    root.resource.parent_id = Some("forged".into());
    cases.push(root);
    let mut panel = TargetRef::resource(panel_resource(
        &app.doc.canvases[0],
        &app.doc.canvases[0].panels[0],
    ));
    panel.resource.id.push('x');
    cases.push(panel);
    for t in cases {
        assert!(matches!(
            plan_tool(&app, query(&app, t, DocumentView::SelfOnly, 0, 50)),
            Err(AutomationError::InvalidSelector(_))
        ));
    }
    let req = query(&app, target.clone(), DocumentView::SelfOnly, 0, 50);
    let plan = plan_tool(&app, req.clone()).unwrap();
    app.doc.automation_revision += 1;
    assert!(matches!(
        plan_tool(&app, req),
        Err(AutomationError::StaleRevision { .. })
    ));
    assert!(matches!(
        execute_tool(&mut app, plan, ExecutionAuthority::Read),
        Err(AutomationError::StaleRevision { .. })
    ));
    app.doc.canvases[0].objects.remove(0);
    assert!(plan_tool(&app, query(&app, target, DocumentView::SelfOnly, 0, 50)).is_err());
}
#[test]
fn pagination_budgets_and_single_detail_failure() {
    let mut app = fixture();
    let root = document_root_target();
    let first = read(&mut app, root.clone(), DocumentView::Children, 0, 1);
    assert_eq!(first.value["next_offset"], Value::Null);
    let canvas = canvas_target(&app.doc.canvases[0]);
    let first = read(&mut app, canvas.clone(), DocumentView::Children, 0, 1);
    let next = read(&mut app, canvas.clone(), DocumentView::Children, 1, 1);
    assert_ne!(
        first.value["items"][0]["target"],
        next.value["items"][0]["target"]
    );
    assert_eq!(first.value["next_offset"], 1);
    assert!(
        read(&mut app, canvas.clone(), DocumentView::Children, 99, 1).value["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    for limit in [0, 201] {
        assert!(
            plan_tool(
                &app,
                query(&app, root.clone(), DocumentView::Children, 0, limit)
            )
            .is_err()
        );
    }
    let leaf = targets(&app)[0].clone();
    let empty = read(&mut app, leaf, DocumentView::Children, 0, 50);
    assert_eq!(empty.value["total"], 0);
}
#[test]
fn response_budget_shortens_pages_without_skipping_and_rejects_oversized_nodes() {
    let mut app = fixture();
    app.doc.canvases[0].name = "a".repeat(130_000);
    let mut c = app.doc.canvases[0].clone();
    c.resource_id = CanvasId::new();
    app.doc.canvases.push(c);
    let mut c = app.doc.canvases[0].clone();
    c.resource_id = CanvasId::new();
    app.doc.canvases.push(c);
    let first = read(
        &mut app,
        document_root_target(),
        DocumentView::Children,
        0,
        50,
    );
    assert!(serde_json::to_vec(&first).unwrap().len() <= DOCUMENT_RESPONSE_BYTES);
    assert_eq!(first.value["items"].as_array().unwrap().len(), 2);
    assert_eq!(first.value["next_offset"], 2);
    let last = read(
        &mut app,
        document_root_target(),
        DocumentView::Children,
        2,
        50,
    );
    assert_eq!(last.value["items"].as_array().unwrap().len(), 1);
    let target = canvas_target(&app.doc.canvases[0]);
    app.doc.canvases[0].name = "x".repeat(DOCUMENT_RESPONSE_BYTES);
    let p = plan_tool(&app, query(&app, target, DocumentView::SelfOnly, 0, 50)).unwrap();
    assert!(matches!(
        execute_tool(&mut app, p, ExecutionAuthority::Read),
        Err(AutomationError::ResponseTooLarge)
    ));
}
#[test]
fn damaged_sources_are_observable_and_never_retargeted() {
    let mut app = fixture();
    let target = targets(&app)[0].clone();
    app.doc.datasets[0].as_table_mut().unwrap().x_binding = None;
    assert_eq!(
        read(&mut app, target.clone(), DocumentView::SelfOnly, 0, 50).value["node"]["source"]["status"],
        "unbound"
    );
    app.doc.datasets[0].as_table_mut().unwrap().x_binding = Some(plotium_data::ColumnId::new());
    assert_eq!(
        read(&mut app, target.clone(), DocumentView::SelfOnly, 0, 50).value["node"]["source"]["status"],
        "missing_column"
    );
    app.doc.canvases[0].objects[0]
        .plot_mut()
        .unwrap()
        .binding
        .series[0]
        .source
        .field = FieldId::new(999999);
    assert_eq!(
        read(&mut app, target.clone(), DocumentView::SelfOnly, 0, 50).value["node"]["source"]["status"],
        "missing_field"
    );
    app.doc.datasets.remove(0);
    assert_eq!(
        read(&mut app, target, DocumentView::SelfOnly, 0, 50).value["node"]["source"]["status"],
        "missing_dataset"
    );
}
#[test]
#[ignore = "Generate an isolated synthetic GUI acceptance project at PLOTIUM_R1_FIXTURE"]
fn write_r1_acceptance_project() {
    let path = std::env::var_os("PLOTIUM_R1_FIXTURE").expect("explicit output path required");
    let app = fixture();
    crate::project::save_project(&app, std::path::Path::new(&path), false).unwrap();
}

#[test]
fn content_details_and_damaged_item_are_explicit() {
    let mut app = fixture();
    let caption = object_target(&app.doc.canvases[0], &app.doc.canvases[0].objects[2]);
    let text = read(&mut app, caption, DocumentView::SelfOnly, 0, 50);
    assert_eq!(text.value["node"]["text"], "Synthetic R1 fixture");
    assert_eq!(text.value["node"]["frame_space"], "page_pt");
    let target = targets(&app)[0].clone();
    app.doc.canvases[0].objects[0]
        .plot_mut()
        .unwrap()
        .binding
        .series[0]
        .source
        .item = Some(plotium_data::TraceItemId::new());
    assert_eq!(
        read(&mut app, target.clone(), DocumentView::SelfOnly, 0, 50).value["node"]["source"]["status"],
        "missing_item"
    );
    let req = query(&app, target, DocumentView::SelfOnly, 0, 50);
    let mut implicit = req;
    implicit.targets = TargetSelector::CurrentSelection;
    assert!(matches!(
        plan_tool(&app, implicit),
        Err(AutomationError::InvalidSelector(_))
    ));
}
