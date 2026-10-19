//! One reproducible native project for the complete stdio/GUI refinement path.
//! Run only with a new absolute PLOTIUM_CANDIDATE_FIXTURE directory.
use crate::{Session, guide_fixture};
use plotium_core::{automation::*, export::*, project::save_project, state::*};
use serde_json::json;
use std::path::Path;
fn write_image(path: &Path, colour: &str, portrait: bool) {
    let (w, h) = if portrait { (80, 160) } else { (160, 80) };
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}pt" height="{h}pt"><rect width="100%" height="100%" fill="{colour}"/><circle cx="40" cy="40" r="24" fill="white"/><path d="M0 70 L160 10" stroke="black" stroke-width="4"/></svg>"#
    );
    let png = rasterize_svg(&svg, [w as f32, h as f32], RasterOptions::new(72))
        .unwrap()
        .to_png()
        .unwrap();
    std::fs::write(path, png).unwrap();
}
fn fixture(dir: &Path) -> (PlotiumApp, Session) {
    let (mut a, mut s, ids) = guide_fixture::fixture();
    a.doc.canvases[0].name = "R16 local refinement — synthetic".into();
    for i in 0..2 {
        let c = &mut a.doc.canvases[0];
        let id = c.objects[i].id;
        c.set_content_frame(id, ObjectFrame::new(0., 0., 240., 170.))
            .unwrap();
        if let Dataset::Table(t) = &mut a.doc.datasets[i] {
            t.board_sheet_visible = false;
            t.name = Some(format!("Independent panel {} data", i + 1));
        }
    }
    a.rebuild_canvas(0);
    guide_fixture::write(
        &mut a,
        &mut s,
        &ids[..1],
        "object.guides.position",
        json!([0.4, 0.7]),
    );
    let source = dir.join("original.png");
    let replacement = dir.join("replacement.png");
    write_image(&source, "#2878b0", false);
    write_image(&replacement, "#dd7020", true);
    let canvas = TargetRef::resource(a.doc.canvases[0].resource_id.into());
    let panel = TargetRef::resource(panel_resource(
        &a.doc.canvases[0],
        &a.doc.canvases[0].panels[0],
    ));
    let q = json!({"expected_project_context":a.session.project_context,"expected_revision":a.doc.automation_revision,"tool_id":FIGURE_EDIT,"parameters":{"canvas":canvas,"edits":[
        {"op":"text_create","parent":panel,"name":"Caption to widen","frame_pt":[5,178,70,16],"text":"Synthetic caption: preserve font and wording.","font_pt":9},
        {"op":"image_create","parent":panel,"name":"Image to replace","frame_pt":[5,200,60,30],"path":source},
        {"op":"image_create","parent":canvas,"name":"Shared image to retain","frame_pt":[280,215,60,30],"path":source},
        {"op":"text_create","parent":canvas,"name":"Repeated note","frame_pt":[355,195,140,18],"text":"Keep this original note.","font_pt":9},
        {"op":"text_create","parent":canvas,"name":"Repeated note","frame_pt":[355,215,140,18],"text":"Duplicate: delete this note only.","font_pt":9},
        {"op":"text_create","parent":canvas,"name":"Intentional A","frame_pt":[350,237,90,15],"text":"Intentional A","font_pt":8},
        {"op":"text_create","parent":canvas,"name":"Intentional B","frame_pt":[390,237,90,15],"text":"Intentional B","font_pt":8}
    ]}});
    let p = s.call(&mut a, "plotium_plan", q).unwrap();
    s.call(&mut a, "plotium_execute", json!({"plan_id":p["plan_id"]}))
        .unwrap();
    a.session.active_canvas = Some(0);
    a.clear_history();
    (a, s)
}
#[test]
#[ignore = "explicit synthetic fixture output directory required"]
fn write_candidate_acceptance_fixture() {
    let dir = std::path::PathBuf::from(
        std::env::var_os("PLOTIUM_CANDIDATE_FIXTURE").expect("set new output directory"),
    );
    assert!(dir.is_absolute() && !dir.exists());
    std::fs::create_dir_all(&dir).unwrap();
    let (a, _) = fixture(&dir);
    save_project(&a, &dir.join("before.plotium"), false).unwrap();
    std::fs::write(dir.join("README.txt"),"180x90 mm; two same-name plots with independent data, six curves each. Preserve manual ranges, units, x=0 uncertainty clipping, Intentional A/B overlap, right plot and shared blue image. Refine the left legend, widen the caption without changing font/text, move/resize and replace Image to replace with replacement.png, delete only the Duplicate note and the second Repeated Series of the left plot. All plans must be actually previewed before explicit execute. No global layout or data changes.\n").unwrap();
}
