//! Reproducible synthetic R12 project: duplicate plot/series labels, stable units,
//! six native curves and a deliberate x=0 error-cap boundary.
use crate::Session;
use plotium_core::{automation::*, data::*, properties::PropertyValue, state::*};
use serde_json::{Value, json};

pub(super) fn fixture() -> (PlotiumApp, Session, Vec<String>) {
    let mut app = PlotiumApp::new_with_settings(Default::default());
    app.start_new_project();
    let units = UnitRegistry::plotium_v1();
    for panel in 0..2 {
        let columns = (0..8)
            .map(|i| {
                let mut c = ColumnSchema::new(format!("Column {i}"), LogicalType::Float64);
                c.unit = Some(units.resolve(if i == 0 { "s" } else { "mA" }).unwrap());
                (
                    c,
                    ColumnChunk::optional_f64((0..4).map(|x| {
                        Some(if i == 0 {
                            x as f64
                        } else if i == 7 {
                            0.15
                        } else {
                            i as f64 + x as f64 * (0.2 + panel as f64 * 0.1)
                        })
                    })),
                )
            })
            .collect();
        let table =
            TableDataset::from_materialized(columns, vec![], None, vec![], "plotium.test.r12")
                .unwrap();
        app.doc.datasets.push(Dataset::Table(Box::new(table)));
    }
    let mut s = Session::default();
    s.allow_modify = true;
    let mut ids = vec![];
    for panel in 0..2 {
        let t = app.doc.datasets[panel].as_table().unwrap();
        let r = &t.typed_state.envelope.revision;
        let c = &r.snapshot.schema.columns;
        let series:Vec<_>=(1..=6).map(|i|json!({"kind":if i%2==0 {"scatter"} else {"line"},
            "source":{"dataset_id":t.resource_id,"field_id":app.doc.datasets[panel].default_field_id().unwrap(),
            "table_id":r.table_id,"table_revision":r.id,"x_column_id":c[0].id,"y_column_id":c[i].id,
            "y_error_column_id":if i==1 {Some(c[7].id)} else {None}}})).collect();
        let parent = if panel == 0 {
            document_root_target()
        } else {
            TargetRef::resource(app.doc.canvases[0].resource_id.into())
        };
        let mut parameters = json!({"parent":parent,"name":"Same plot","panel_name":format!("Panel {panel}"),
            "panel_frame_pt":[12. + panel as f64 * 250.,12.,240.,225.],"series":series});
        if panel == 0 {
            parameters["canvas"] = json!({"name":"R12 guide acceptance","size_mm":[180,90]});
        }
        let args = json!({"expected_project_context":app.session.project_context,"expected_revision":app.doc.automation_revision,
            "tool_id":"plot.create","parameters":parameters});
        let p = s.call(&mut app, "plotium_plan", args).unwrap();
        let result = s
            .call(&mut app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
            .unwrap();
        ids.push(
            result["value"]["creation"]["plot"]["resource"]["id"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    for index in 0..2 {
        let object = app.doc.canvases[0].objects[index].id;
        for curve in 0..6 {
            let sid = app.doc.canvases[0].objects[index]
                .plot()
                .unwrap()
                .binding
                .series[curve]
                .id;
            let target = app.series_target(0, object, sid).unwrap();
            let label = if curve < 2 {
                "Repeated".to_owned()
            } else {
                format!("Long sample label {}", curve + 1)
            };
            let commit = app
                .plan_property_write(
                    plotium_core::properties::line_display::LABEL,
                    &[target],
                    &PropertyValue::Text(label),
                )
                .unwrap();
            app.commit_property(commit);
            let palette = [
                "#0072b2", "#d55e00", "#009e73", "#cc79a7", "#e69f00", "#56b4e9",
            ];
            let target = app.series_target(0, object, sid).unwrap();
            let a = json!({"expected_project_context":app.session.project_context,"expected_revision":app.doc.automation_revision,
                "tool_id":"properties.set","targets":[target],"parameters":{"key":"series.line.color","value":palette[curve]}});
            let p = s.call(&mut app, "plotium_plan", a).unwrap();
            s.call(&mut app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
                .unwrap();
        }
        let mut o = app.doc.canvases[0].objects[index]
            .plot()
            .unwrap()
            .axis_overrides
            .clone();
        o.x_range = Some(if index == 0 {
            AxisRange::new(0., 3.)
        } else {
            AxisRange::new(-1., 4.)
        });
        o.y_range = Some(if index == 0 {
            AxisRange::new(0., 10.)
        } else {
            AxisRange::new(-2., 12.)
        });
        app.set_axis_overrides_value(0, object, &o);
    }
    (app, s, ids)
}

pub(super) fn args(app: &PlotiumApp, ids: &[String], key: &str, value: Value) -> Value {
    json!({"expected_project_context":app.session.project_context,"expected_revision":app.doc.automation_revision,
        "ids":ids,"tool_id":"properties.set","parameters":{"key":key,"value":value}})
}
pub(super) fn write(
    app: &mut PlotiumApp,
    s: &mut Session,
    ids: &[String],
    key: &str,
    value: Value,
) -> Value {
    let a = args(app, ids, key, value);
    let p = s.call(app, "plotium_plan", a).unwrap();
    s.call(app, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap()
}
pub(super) fn inspect(app: &mut PlotiumApp, s: &mut Session, id: &str, key: &str) -> Value {
    let a = json!({"expected_project_context":app.session.project_context,"expected_revision":app.doc.automation_revision,"ids":[id],"key":key});
    s.call(app, "plotium_properties_inspect", a).unwrap()["value"]["readings"][0].clone()
}
