use super::*;
use crate::automation::table_read::execute_table_read;
use plotium_data::*;
use std::path::PathBuf;

fn req(app: &PlotiumApp, tool: &str, params: Value, ids: Vec<String>) -> ToolRequest {
    ToolRequest {
        tool_id: tool.into(),
        tool_version: 1,
        parameters: params,
        targets: TargetSelector::Explicit { ids },
        expected_revision: DocumentRevision(app.doc.automation_revision),
        caller: CallerType::Agent,
    }
}
fn fixture(extension: &str) -> PlotiumApp {
    let path =
        std::env::temp_dir().join(format!("plotium-r8-{}.{}", uuid::Uuid::new_v4(), extension));
    let csv = "x_left,y_left,err_left,x_right,y_right\n0,1,0.1,0,2\n1,2,0.2,0.2,3\n2,4,0.3,0.4,5\n";
    std::fs::write(
        &path,
        if extension == "tsv" {
            csv.replace(',', "\t")
        } else {
            csv.into()
        },
    )
    .unwrap();
    let schema = ["x_left", "y_left", "err_left", "x_right", "y_right"]
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let mut c = ColumnSchema::new(
                *n,
                if i < 2 || i == 4 {
                    LogicalType::Int64
                } else {
                    LogicalType::Float64
                },
            );
            c.unit = Some(unit(if i == 0 {
                "s"
            } else if i == 3 {
                "V"
            } else {
                "mA"
            }));
            c
        })
        .collect::<Vec<_>>();
    let mut app = PlotiumApp::new();
    let params = json!({"mode":"table_data_only","paths":[path],"header":"present","schema":TableSchema::new(schema).unwrap()});
    let plan = plan_tool(&app, req(&app, "data.import", params.clone(), vec![])).unwrap();
    let p =
        PreparedTableImport::prepare(serde_json::from_value(params).unwrap(), || false).unwrap();
    execute_prepared_table_import(&mut app, plan, p, ExecutionAuthority::ReversibleModify).unwrap();
    std::fs::remove_file(path).unwrap();
    app
}
fn unit(s: &str) -> UnitSpec {
    let mut u = UnitSpec::dimensionless(s);
    u.quantity = s.into();
    u.canonical_unit = s.into();
    u.dimension.insert(s.into(), 1);
    u
}
fn params(app: &PlotiumApp, filtered: bool) -> Value {
    let t = app.doc.datasets[0].as_table().unwrap();
    let r = &t.typed_state.envelope.revision;
    let columns = &r.snapshot.schema.columns;
    let mut computed = ColumnSchema::new("signal_x2", LogicalType::Float64);
    computed.unit = columns[1].unit.clone();
    let ci = computed.id;
    let mut root = Relation::ComputedColumn {
        input: Box::new(Relation::SnapshotRead(SnapshotRead {
            table: r.table_id,
            revision: r.id,
            fingerprint: r.snapshot.fingerprint,
        })),
        column: computed,
        expression: Expression::call(
            "multiply.v1",
            vec![
                Expression::Cast {
                    input: Box::new(Expression::column(columns[1].id)),
                    target: LogicalType::Float64,
                    failure: CastFailure::Error,
                },
                Expression::Literal {
                    value: LiteralValue::Float64(FiniteOrSpecial::new(2.)),
                },
            ],
        ),
    };
    if filtered {
        root = Relation::Filter {
            input: Box::new(root),
            predicate: Expression::call(
                "not.v1",
                vec![Expression::call(
                    "eq.v1",
                    vec![
                        Expression::column(columns[0].id),
                        Expression::Literal {
                            value: LiteralValue::Int64(0),
                        },
                    ],
                )],
            ),
        };
        root = Relation::StableSort {
            input: Box::new(root),
            keys: vec![SortKey {
                column: columns[0].id,
                direction: SortDirection::Descending,
                nulls: NullPlacement::Last,
            }],
        };
    }
    root = Relation::Project {
        input: Box::new(root),
        columns: vec![columns[0].id, columns[1].id, ci],
    };
    json!({"mode":"table_data_only","name":"Processed","memory_limit_bytes":16*1024*1024,"plan":RelPlanV1::new(root)})
}
fn transform(app: &mut PlotiumApp, p: Value) -> ToolResult {
    let request = req(
        app,
        "data.transform",
        p,
        vec![app.doc.datasets[0].resource_id().to_string()],
    );
    let plan = plan_tool(app, request).unwrap();
    let prepared = TableTransformSnapshot::capture(app, &plan.request)
        .unwrap()
        .prepare(&AtomicBool::new(false))
        .unwrap();
    assert_eq!(prepared.preflight().unwrap()["status"], "prepared");
    let candidate = prepared.preflight().unwrap()["output"]["dataset_id"].clone();
    let result =
        execute_prepared_table_transform(app, plan, prepared, ExecutionAuthority::ReversibleModify)
            .unwrap();
    assert_eq!(result.value["output"]["dataset_id"], candidate);
    result
}
fn read(app: &PlotiumApp, index: usize, provenance: bool) -> ToolResult {
    let t = app.doc.datasets[index].as_table().unwrap();
    let p = json!({"dataset_id":t.resource_id,"table_revision":t.typed_state.envelope.revision.id,"include_provenance":provenance});
    let plan = plan_tool(
        app,
        req(app, TABLE_READ_ROWS, p, vec![t.resource_id.to_string()]),
    )
    .unwrap();
    execute_table_read(app, &plan).unwrap()
}
#[test]
fn csv_tsv_complete_results_identity_units_sources_undo_and_reopen() {
    for ext in ["csv", "tsv"] {
        let mut app = fixture(ext);
        let source = read(&app, 0, false).value;
        let source_id = app.doc.datasets[0].resource_id();
        let p = params(&app, false);
        let r = transform(&mut app, p.clone());
        let values = read(&app, 1, true).value;
        assert_eq!(
            values["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["values"][2]["value"].as_f64().unwrap())
                .collect::<Vec<_>>(),
            [2., 4., 8.]
        );
        assert_eq!(
            r.value["output"]["columns"][2]["logical_type"]["kind"],
            "float64"
        );
        assert_eq!(
            r.value["output"]["columns"][2]["unit"]["display_unit"],
            "mA"
        );
        assert_eq!(
            r.value["output"]["provenance"]["operation"]["plan"],
            p["plan"]
        );
        assert_eq!(
            r.value["output"]["provenance"]["operation"]["parameters"]["backend"],
            "plotium.reference.v1"
        );
        assert_eq!(
            r.value["output"]["provenance"]["operation"]["parameters"]["memory_limit_bytes"],
            16 * 1024 * 1024
        );
        assert!(app.doc.canvases.is_empty());
        let p = params(&app, true);
        let result = transform(&mut app, p);
        let output_id = app.doc.datasets[2].resource_id();
        let rev = app.doc.datasets[2]
            .as_table()
            .unwrap()
            .typed_state
            .envelope
            .revision
            .id;
        let page = read(&app, 2, true).value;
        assert_eq!(page["rows"].as_array().unwrap().len(), 2);
        for (i, (x, y, z, pos)) in [("2", "4", 8., 2), ("1", "2", 4., 1)].iter().enumerate() {
            let row = &page["rows"][i];
            assert_eq!(row["values"][0]["value"], *x);
            assert_eq!(row["values"][1]["value"], *y);
            assert_eq!(row["values"][2]["value"], *z);
            assert_eq!(row["row_id"], source["rows"][*pos]["row_id"]);
            assert_eq!(row["sources"][0]["row_id"], row["row_id"]);
            assert_eq!(row["sources"][0]["input_position"], *pos);
            assert_eq!(row["sources"][0]["dataset_id"], source_id.to_string());
        }
        let lineage = &result.value["output"]["provenance"]["operation"]["column_lineage"];
        assert_eq!(
            lineage[2]["inputs"],
            json!([result.value["output"]["columns"][1]["column_id"]])
        );
        let mut after_source = read(&app, 0, false).value;
        after_source["revision"] = source["revision"].clone();
        assert_eq!(after_source, source);
        let run = app.doc.automation_runs.last().unwrap();
        assert_eq!(run.table_plans.len(), 1);
        assert_eq!(run.table_revisions.len(), 2);
        assert!(run.errors.is_empty());
        app.undo();
        assert_eq!(app.doc.datasets.len(), 2);
        app.redo();
        assert_eq!(app.doc.datasets[2].resource_id(), output_id);
        assert_eq!(
            app.doc.datasets[2]
                .as_table()
                .unwrap()
                .typed_state
                .envelope
                .revision
                .id,
            rev
        );
        let path =
            std::env::temp_dir().join(format!("plotium-r8-{}.plotium", uuid::Uuid::new_v4()));
        crate::project::save_project(&app, &path, false).unwrap();
        let reopened = crate::project::load_project(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        let mut reopened_page = read(&reopened, 2, true).value;
        reopened_page["revision"] = page["revision"].clone();
        assert_eq!(reopened_page, page);
        assert_eq!(reopened.doc.datasets[2].resource_id(), output_id);
    }
}
#[test]
fn rejects_missing_columns_wrong_types_unpinned_inputs_and_out_of_scope_plans() {
    let app = fixture("csv");
    let p = params(&app, true);
    let id = app.doc.datasets[0].resource_id().to_string();
    let mut bad = p.clone();
    bad["plan"]["root"]["columns"][0] = json!(ColumnId::new());
    assert!(plan_tool(&app, req(&app, "data.transform", bad, vec![id.clone()])).is_err());
    let mut bad = p.clone();
    bad["plan"]["root"]["input"]["input"]["input"]["expression"]["args"][0] =
        json!({"expr":"column","column":p["plan"]["root"]["columns"][1]});
    assert!(plan_tool(&app, req(&app, "data.transform", bad, vec![id.clone()])).is_err());
    let mut bad = p.clone();
    bad["plan"]["root"]["input"]["input"]["input"]["input"]["revision"] = json!(RevisionId::new());
    assert!(matches!(
        plan_tool(&app, req(&app, "data.transform", bad, vec![id.clone()])),
        Err(AutomationError::StaleTableRevision { .. })
    ));
    let mut bad = p.clone();
    bad["plan"]["root"] = json!({"op":"patch","input":p["plan"]["root"].clone(),"edits":[]});
    assert!(plan_tool(&app, req(&app, "data.transform", bad, vec![id])).is_err());
    assert!(
        app.doc
            .automation_runs
            .iter()
            .all(|r| r.nodes.iter().all(|n| n.tool_id != "data.transform"))
    );
}
#[test]
fn write_schema_rejects_ignored_plan_fields_and_depth_overflow() {
    let app = fixture("csv");
    let p = params(&app, false);
    let mut bad = p.clone();
    bad["plan"]["root"]["filters"] = json!([]);
    assert!(parse_table_transform(bad).is_err());
    let mut bad = p.clone();
    bad["plan"]["root"]["input"]["expression"]["arguments"] = json!([]);
    assert!(parse_table_transform(bad).is_err());
    let mut bad = p.clone();
    for _ in 0..17 {
        bad["plan"]["root"] = json!({"op":"project","input":bad["plan"]["root"].clone(),"columns":p["plan"]["root"]["columns"]});
    }
    assert!(parse_table_transform(bad).is_err());
    let mut bad = p;
    bad["name"] = json!("x".repeat(65536));
    assert!(parse_table_transform(bad).is_err());
}

#[test]
fn budget_cancel_and_commit_revision_permission_fail_without_partial_table() {
    let mut app = fixture("csv");
    let id = app.doc.datasets[0].resource_id().to_string();
    let mut p = params(&app, true);
    p["memory_limit_bytes"] = json!(1);
    let plan = plan_tool(&app, req(&app, "data.transform", p, vec![id.clone()])).unwrap();
    assert!(matches!(
        TableTransformSnapshot::capture(&app, &plan.request)
            .unwrap()
            .prepare(&AtomicBool::new(false)),
        Err(AutomationError::TableTransformBudget(_))
    ));
    let p = params(&app, true);
    let request = req(&app, "data.transform", p, vec![id]);
    assert!(matches!(
        TableTransformSnapshot::capture(&app, &request)
            .unwrap()
            .prepare(&AtomicBool::new(true)),
        Err(AutomationError::TableTransformCancelled)
    ));
    for permission in [true, false] {
        let plan = plan_tool(&app, request.clone()).unwrap();
        let prepared = TableTransformSnapshot::capture(&app, &plan.request)
            .unwrap()
            .prepare(&AtomicBool::new(false))
            .unwrap();
        if permission {
            app.doc.automation_revision += 1;
        }
        let result = execute_prepared_table_transform(
            &mut app,
            plan,
            prepared,
            if permission {
                ExecutionAuthority::ReversibleModify
            } else {
                ExecutionAuthority::Read
            },
        );
        assert!(result.is_err());
        assert_eq!(app.doc.datasets.len(), 1);
        assert!(app.doc.automation_runs.last().unwrap().errors.len() == 1);
        if permission {
            app.doc.automation_revision -= 1;
        }
    }
}
#[test]
#[ignore = "writes the isolated R8 GUI acceptance project only when explicitly requested"]
fn write_r8_acceptance_fixture() {
    let mut app = fixture("csv");
    add_canvas(&mut app);
    let path = PathBuf::from(std::env::var("PLOTIUM_R8_FIXTURE").expect("explicit output path"));
    crate::project::save_project(&app, &path, false).unwrap();
}

fn add_canvas(app: &mut PlotiumApp) {
    let table = app.doc.datasets[0].as_table_mut().unwrap();
    let cols = &table.typed_state.envelope.revision.snapshot.schema.columns;
    table.x_binding = Some(cols[0].id);
    table.series_bindings = vec![TableSeriesBinding {
        value_column: cols[1].id,
        uncertainty_column: Some(cols[2].id),
        fit: None,
    }];
    table.board_sheet_visible = false;
    let mut canvas =
        crate::workflow::build_default_canvas(&app.doc.datasets[0], "R8 source → derived");
    let plot = canvas.objects[0].plot_mut().unwrap();
    plot.axis_overrides.x_range = Some(AxisRange::new(0., 3.));
    plot.axis_overrides.y_range = Some(AxisRange::new(0., 10.));
    if let plotium_figure::SeriesEncoding::Line(line) = &mut plot.binding.series[0].encoding {
        line.width = plotium_figure::PositiveFiniteF32::new(1.5).unwrap();
    }
    app.doc.canvases.push(canvas);
    app.session.active_canvas = Some(0);
    app.session.view = PrimaryView::Canvas;
    // Freeze the GUI's actual default chart encoding through the merged R4 service.
    let t = app.doc.datasets[0].as_table().unwrap();
    let r = &t.typed_state.envelope.revision;
    let cols = &r.snapshot.schema.columns;
    let plot = app.doc.canvases[0].objects[0].plot().unwrap();
    let target = app
        .series_target(
            0,
            app.doc.canvases[0].objects[0].id,
            plot.binding.series[0].id,
        )
        .unwrap();
    let plan=plan_tool(app,req(app,CURVE_BIND,json!({"edits":[{"target":target,"source":{"dataset_id":t.resource_id,"field_id":plot.binding.series[0].source.field,"table_id":r.table_id,"table_revision":r.id,"x_column_id":cols[0].id,"y_column_id":cols[1].id,"y_error_column_id":cols[2].id}}]}),vec![])).unwrap();
    execute_tool(app, plan, ExecutionAuthority::ReversibleModify).unwrap();
}
#[test]
fn derived_columns_bind_through_r4_render_and_preserve_authored_style_and_source() {
    let mut app = fixture("csv");
    add_canvas(&mut app);
    let p = params(&app, true);
    let original = app.doc.canvases[0].objects[0].plot().unwrap().clone();
    let source = app.doc.datasets[0]
        .as_table()
        .unwrap()
        .typed_state
        .envelope
        .clone();
    let object_id = app.doc.canvases[0].objects[0].id;
    let target = app
        .series_target(0, object_id, original.binding.series[0].id)
        .unwrap();
    transform(&mut app, p);
    assert_eq!(app.doc.canvases.len(), 1);
    assert_eq!(
        app.doc.canvases[0].objects[0].plot().unwrap().binding,
        original.binding
    );
    let t = app.doc.datasets[1].as_table().unwrap();
    let r = &t.typed_state.envelope.revision;
    let columns = &r.snapshot.schema.columns;
    let binding = json!({"edits":[{"target":target,"source":{"dataset_id":t.resource_id,"field_id":app.doc.datasets[1].default_field_id().unwrap(),"table_id":r.table_id,"table_revision":r.id,"x_column_id":columns[0].id,"y_column_id":columns[2].id,"y_error_column_id":null}}]});
    let plan = plan_tool(&app, req(&app, CURVE_BIND, binding, vec![])).unwrap();
    execute_tool(&mut app, plan, ExecutionAuthority::ReversibleModify).unwrap();
    let plot = app.doc.canvases[0].objects[0].plot().unwrap();
    assert_eq!(
        plot.binding.series[0].encoding,
        original.binding.series[0].encoding
    );
    assert_eq!(plot.binding.series[0].id, original.binding.series[0].id);
    assert_eq!(plot.axis_overrides.x_range, original.axis_overrides.x_range);
    assert_eq!(plot.axis_overrides.y_range, original.axis_overrides.y_range);
    assert_eq!(plot.figure().series[0].points, [[2., 8.], [1., 4.]]);
    assert_eq!(
        app.doc.datasets[0].as_table().unwrap().typed_state.envelope,
        source
    );
    let canvas_target = TargetRef::resource(app.doc.canvases[0].resource_id.into());
    let plan = plan_tool(
        &app,
        req(
            &app,
            IMAGE_PREVIEW,
            json!({"target":canvas_target,"max_edge":480}),
            vec![app.doc.canvases[0].resource_id.to_string()],
        ),
    )
    .unwrap();
    let image = ImagePreviewSnapshot::capture(&app, &plan)
        .unwrap()
        .render()
        .unwrap();
    assert!(image.value["png_base64"].as_str().unwrap().len() > 1000);
}
