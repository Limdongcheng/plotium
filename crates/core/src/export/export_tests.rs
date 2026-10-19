use super::*;
use crate::state::{
    CanvasDocument, CanvasObject, CanvasObjectKind, ObjectFrame, ObjectId, ShapeKind, ShapeObject,
};

fn canvas(name: &str, size_mm: [f32; 2]) -> CanvasDocument {
    CanvasDocument::new(name.to_owned(), size_mm)
}

fn document_svg(canvas: &CanvasDocument, target_width_mm: Option<f32>) -> String {
    document_svg_with_assets(
        canvas,
        &BTreeMap::new(),
        target_width_mm,
        MissingImagePolicy::Block,
    )
    .expect("a document without image assets renders")
}

fn test_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("plotium-export-tests"))
}

fn canvas_with_shape(frame: ObjectFrame) -> CanvasDocument {
    let mut canvas = canvas("page", [100.0, 80.0]);
    canvas.objects.push(CanvasObject {
        id: ObjectId::new(1),
        name: "shape".into(),
        frame,
        locked: false,
        visible: true,
        kind: CanvasObjectKind::Shape(ShapeObject::new(ShapeKind::Rect)),
    });
    canvas
}

#[test]
fn svg_export_is_invariant_to_board_pos() {
    let mut c = canvas("page", [80.0, 60.0]);
    let baseline = document_svg(&c, None);
    c.board_pos = [1234.0, -567.0];
    assert_eq!(document_svg(&c, None), baseline);
}

#[test]
fn bitmap_multi_page_paths_are_deterministic() {
    let paths = export_output_paths(Path::new("figure.png"), ExportFormat::Png, 2);
    assert_eq!(
        paths,
        vec![
            PathBuf::from("figure-001.png"),
            PathBuf::from("figure-002.png")
        ]
    );
    let paths = export_output_paths(Path::new("figure.jpeg"), ExportFormat::Jpeg, 2);
    assert_eq!(
        paths,
        vec![
            PathBuf::from("figure-001.jpg"),
            PathBuf::from("figure-002.jpg")
        ]
    );
}

#[test]
fn pdf_single_page_starts_with_pdf_marker() {
    let canvases = vec![canvas("page", [25.4, 25.4])];
    std::fs::create_dir_all(test_dir()).unwrap();
    let out = test_dir().join("single.pdf");
    let paths = export_canvases(
        &canvases,
        Some(0),
        &ExportSettings {
            format: ExportFormat::Pdf,
            scope: ExportPageScope::Current,
            dpi: DEFAULT_BITMAP_DPI,
            target_width_mm: None,
            trim_to_visible_content: false,
            allow_missing_images: false,
        },
        &out,
    )
    .unwrap();
    let bytes = std::fs::read(&paths[0]).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
}

#[test]
fn pdf_multi_page_contains_selected_page_count() {
    let canvases = vec![canvas("one", [25.4, 25.4]), canvas("two", [25.4, 25.4])];
    std::fs::create_dir_all(test_dir()).unwrap();
    let out = test_dir().join("multi.pdf");
    let paths = export_canvases(
        &canvases,
        Some(0),
        &ExportSettings {
            format: ExportFormat::Pdf,
            scope: ExportPageScope::All,
            dpi: DEFAULT_BITMAP_DPI,
            target_width_mm: None,
            trim_to_visible_content: false,
            allow_missing_images: false,
        },
        &out,
    )
    .unwrap();
    let bytes = std::fs::read(&paths[0]).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert_eq!(text.matches("/Type /Page\n").count(), 2);
}

#[test]
fn bitmaps_decode_with_expected_pixel_dimensions() {
    let canvases = vec![canvas("page", [25.4, 12.7])];
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    for format in [ExportFormat::Png, ExportFormat::Jpeg, ExportFormat::Tiff] {
        let out = dir.join(format!("bitmap.{}", format.extension()));
        let paths = export_canvases(
            &canvases,
            Some(0),
            &ExportSettings {
                format,
                scope: ExportPageScope::Current,
                dpi: DEFAULT_BITMAP_DPI,
                target_width_mm: None,
                trim_to_visible_content: false,
                allow_missing_images: false,
            },
            &out,
        )
        .unwrap();
        let image = image::open(&paths[0]).unwrap();
        assert_eq!((image.width(), image.height()), (300, 150));
    }
}

#[test]
fn preset_target_width_drives_pixel_dimensions() {
    // 200 mm wide page, downscaled to an 89 mm column at 600 dpi:
    // width_px = 89/25.4 * 600 ≈ 2102, height preserves the 2:1 aspect ratio.
    let canvases = vec![canvas("page", [200.0, 100.0])];
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("column.tif");
    let paths = export_canvases(
        &canvases,
        Some(0),
        &ExportSettings {
            format: ExportFormat::Tiff,
            scope: ExportPageScope::Current,
            dpi: 600,
            target_width_mm: Some(89.0),
            trim_to_visible_content: false,
            allow_missing_images: false,
        },
        &out,
    )
    .unwrap();
    let image = image::open(&paths[0]).unwrap();
    assert_eq!((image.width(), image.height()), (2102, 1051));
}

#[test]
fn preset_scales_vector_physical_size() {
    let doc = canvas("page", [200.0, 100.0]);
    let [w, h] = doc.size_pt();
    let natural = document_svg(&doc, None);
    assert!(natural.contains(&format!(r#"width="{w}pt" height="{h}pt""#)));

    // Half-width target halves the declared physical size; the viewBox — and
    // hence all geometry — is untouched.
    let scaled = document_svg(&doc, Some(100.0));
    assert!(scaled.contains(&format!(r#"width="{}pt" height="{}pt""#, w * 0.5, h * 0.5)));
    assert!(scaled.contains(&format!(r#"viewBox="0 0 {w} {h}""#)));
}

#[test]
fn trimmed_svg_uses_painted_bounds_and_keeps_page_background() {
    let doc = canvas_with_shape(ObjectFrame::new(100.0, 80.0, 40.0, 30.0));
    let svg = trim::trim_document_svg(&doc, None).unwrap();
    let [page_width, page_height] = doc.size_pt();
    assert!(svg.contains("<rect x=\""));
    assert!(!svg.contains(&format!("viewBox=\"0 0 {page_width} {page_height}\"")));
    assert!(svg.contains("viewBox=\""));
}

fn svg_number(svg: &str, attribute: &str) -> f32 {
    let start = svg.find(attribute).unwrap() + attribute.len();
    let end = svg[start..].find('"').unwrap() + start;
    svg[start..end].trim_end_matches("pt").parse().unwrap()
}

fn pdf_media_boxes(bytes: &[u8]) -> Vec<[f32; 2]> {
    let text = String::from_utf8_lossy(bytes);
    text.match_indices("/MediaBox [")
        .filter_map(|(start, _)| {
            let values = text[start + 11..].split_once(']')?.0;
            let numbers = values
                .split_whitespace()
                .map(str::parse::<f32>)
                .collect::<Result<Vec<_>, _>>()
                .ok()?;
            (numbers.len() == 4).then_some([numbers[2], numbers[3]])
        })
        .collect()
}

#[test]
fn preset_scale_precedes_trim_without_refitting_and_padding_is_physical_point() {
    let doc = canvas_with_shape(ObjectFrame::new(100.0, 80.0, 40.0, 30.0));
    let svg = trim::trim_document_svg(&doc, Some(50.0)).unwrap();
    let width_pt = svg_number(&svg, "width=\"");
    let preset_width_pt = 50.0 * 72.0 / 25.4;
    assert!(width_pt < preset_width_pt);

    let view_box = svg
        .split_once("viewBox=\"")
        .unwrap()
        .1
        .split_once('"')
        .unwrap()
        .0
        .split_whitespace()
        .map(|value| value.parse::<f32>().unwrap())
        .collect::<Vec<_>>();
    let authored_scale = 0.5;
    assert!((width_pt - view_box[2] * authored_scale).abs() < 0.01);
    let bounds_svg =
        plotium_render::svg::export_document_for_bounds(&crate::state::build_render_document(&doc));
    let mut options = resvg::usvg::Options::default();
    fonts::load_system_fonts(options.fontdb_mut());
    let tree = resvg::usvg::Tree::from_str(&bounds_svg, &options).unwrap();
    let painted = tree.root().abs_stroke_bounding_box();
    let painted_x = painted.x() * doc.size_pt()[0] / tree.size().width();
    // Two authored points at 0.5 scale are one physical point.
    assert!(((painted_x - view_box[0]) * authored_scale - 1.0).abs() < 0.01);
}

#[test]
fn empty_trimmed_svg_keeps_original_page() {
    let doc = canvas("empty", [100.0, 80.0]);
    assert_eq!(
        trim::trim_document_svg(&doc, None).unwrap(),
        document_svg(&doc, None)
    );
}

#[test]
fn bitmap_formats_encode_the_shared_trimmed_dimensions() {
    let canvases = vec![canvas_with_shape(ObjectFrame::new(100.0, 80.0, 40.0, 30.0))];
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let mut dimensions = Vec::new();
    for format in [ExportFormat::Png, ExportFormat::Jpeg, ExportFormat::Tiff] {
        let out = dir.join(format!("trimmed.{}", format.extension()));
        let paths = export_canvases(
            &canvases,
            Some(0),
            &ExportSettings {
                format,
                scope: ExportPageScope::Current,
                dpi: 72,
                target_width_mm: None,
                trim_to_visible_content: true,
                allow_missing_images: false,
            },
            &out,
        )
        .unwrap();
        let image = image::open(&paths[0]).unwrap();
        dimensions.push((image.width(), image.height()));
        assert!(image.width() < 284 && image.height() < 227);
    }
    assert!(dimensions.windows(2).all(|pair| pair[0] == pair[1]));
}

#[test]
fn pdf_media_boxes_follow_each_trimmed_svg_page_and_empty_page_stays_full_size() {
    let shaped = canvas_with_shape(ObjectFrame::new(100.0, 80.0, 40.0, 30.0));
    let mut wider = canvas_with_shape(ObjectFrame::new(60.0, 50.0, 100.0, 35.0));
    wider.name = "wider".into();
    let empty = canvas("empty", [100.0, 80.0]);
    let canvases = vec![shaped, wider, empty];
    let dir = test_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("trimmed-pages.pdf");
    let paths = export_canvases(
        &canvases,
        Some(0),
        &ExportSettings {
            format: ExportFormat::Pdf,
            scope: ExportPageScope::All,
            dpi: DEFAULT_BITMAP_DPI,
            target_width_mm: None,
            trim_to_visible_content: true,
            allow_missing_images: false,
        },
        &out,
    )
    .unwrap();
    let boxes = pdf_media_boxes(&std::fs::read(&paths[0]).unwrap());
    assert_eq!(boxes.len(), 3);
    assert_ne!(boxes[0], boxes[1]);
    let [page_width, page_height] = canvases[2].size_pt();
    assert!((boxes[2][0] - page_width).abs() < 0.01);
    assert!((boxes[2][1] - page_height).abs() < 0.01);
}

fn assert_page_size_pt(actual: [f32; 2], expected_mm: [f32; 2]) {
    for (got, mm) in actual.into_iter().zip(expected_mm) {
        assert!(
            (got - mm * 72.0 / 25.4).abs() < 0.001,
            "PDF/SVG physical points {actual:?} must describe {expected_mm:?} mm"
        );
    }
}

#[test]
fn native_pdf_and_svg_keep_physical_size_for_single_and_multi_page() {
    let canvases = vec![canvas("one", [180.0, 90.0]), canvas("two", [180.0, 90.0])];
    let dir = tempfile::tempdir().unwrap();
    for width in [None, Some(90.0)] {
        let expected = if width.is_some() {
            [90.0, 45.0]
        } else {
            [180.0, 90.0]
        };
        let svg = document_svg(&canvases[0], width);
        assert_page_size_pt(
            [svg_number(&svg, "width=\""), svg_number(&svg, "height=\"")],
            expected,
        );
        for scope in [ExportPageScope::Current, ExportPageScope::All] {
            let paths = export_canvases(
                &canvases,
                Some(0),
                &ExportSettings {
                    format: ExportFormat::Pdf,
                    scope,
                    dpi: 300,
                    target_width_mm: width,
                    trim_to_visible_content: false,
                    allow_missing_images: false,
                },
                &dir.path().join("physical.pdf"),
            )
            .unwrap();
            let boxes = pdf_media_boxes(&std::fs::read(&paths[0]).unwrap());
            assert_eq!(
                boxes.len(),
                if matches!(scope, ExportPageScope::All) {
                    2
                } else {
                    1
                }
            );
            for size in boxes {
                assert_page_size_pt(size, expected);
            }
        }
    }
}

#[test]
fn mcp_pdf_and_svg_keep_native_or_explicit_physical_size() {
    let mut app = crate::state::PlotiumApp::new_with_settings(Default::default());
    app.doc.canvases.push(canvas("native", [180.0, 90.0]));
    let c = &app.doc.canvases[0];
    let dir = tempfile::tempdir().unwrap();
    for size_mm in [None, Some([180.0, 90.0]), Some([90.0, 45.0])] {
        let prepared = prepare_figure_export(
            &app,
            FigureExportParams {
                canvas: crate::automation::TargetRef::resource(c.resource_id.into()),
                outputs: vec![ExportOutput {
                    format: "pdf".into(),
                    path: dir.path().join("mcp.pdf"),
                    overwrite: false,
                }],
                size_mm,
                dpi: 300,
            },
        )
        .unwrap();
        let expected = size_mm.unwrap_or([180.0, 90.0]);
        let assets = BTreeMap::new();
        let svg = String::from_utf8(
            request::encode_canvas(c, &assets, "svg", prepared.size_mm, 300).unwrap(),
        )
        .unwrap();
        assert_page_size_pt(
            [svg_number(&svg, "width=\""), svg_number(&svg, "height=\"")],
            expected,
        );
        let pdf = request::encode_canvas(c, &assets, "pdf", prepared.size_mm, 300).unwrap();
        let boxes = pdf_media_boxes(&pdf);
        assert_eq!(boxes.len(), 1);
        assert_page_size_pt(boxes[0], expected);
    }
}
