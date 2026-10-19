use super::*;
use plotium_figure::{Axis, GuideLayout, GuideVisibility, Series};

fn figure() -> Figure {
    let mut f = Figure::new("", Axis::new("x", 0., 3.), Axis::new("y", 0., 10.));
    for i in 0..6 {
        f = f.with_series(Series::line(
            format!("Sample {i}"),
            vec![[0., i as f64], [3., i as f64 + 1.]],
        ));
    }
    f
}
#[test]
fn guide_corners_and_forward_inverse_mapping_agree_across_size_and_zoom() {
    for size in [[400., 300.], [700., 500.]] {
        for position in [[0., 0.], [0., 1.], [1., 0.], [1., 1.], [0.25, 0.75]] {
            let mut f = figure();
            f.legend_position = Some(position);
            let g = guide_geometry(&f, size);
            assert_eq!(g.fits_plot, Some([true, true]));
            let origin = g.legend.unwrap();
            let effective = g.effective_position.unwrap();
            for i in 0..2 {
                assert!((position[i] - effective[i]).abs() < 1e-6);
            }
            for zoom in [0.5, 1., 2.5] {
                let p = Rect::new(
                    g.plot.left * zoom,
                    g.plot.top * zoom,
                    g.plot.width * zoom,
                    g.plot.height * zoom,
                );
                let r = crate::legend_rect(&f, p, zoom).unwrap();
                assert!((r.left - origin.left * zoom).abs() < 1e-3);
                assert!((r.top - origin.top * zoom).abs() < 1e-3);
                let inverse =
                    crate::legend_position_for_origin(&f, p, zoom, [r.left, r.top]).unwrap();
                for i in 0..2 {
                    assert!((inverse[i] - position[i]).abs() < 1e-6);
                }
            }
        }
    }
}
#[test]
fn oversize_does_not_shrink_and_hidden_keeps_native_content() {
    let mut f = figure();
    f.guide_layout = GuideLayout::Horizontal;
    f.guide_title = "Title ".repeat(100);
    f.legend_position = Some([1., 1.]);
    let font = f.typography.legend_pt;
    let g = guide_geometry(&f, [150., 100.]);
    assert!(!g.fits_plot.unwrap()[0]);
    assert_eq!(g.available_travel.unwrap()[0], 0.);
    assert_eq!(g.legend.unwrap().left, g.plot.left);
    assert_eq!(g.effective_position.unwrap()[0], 0.);
    assert_eq!(f.typography.legend_pt, font);
    f.guide_visibility = GuideVisibility::Hide;
    let g = guide_geometry(&f, [150., 100.]);
    assert_eq!(g.entries, 6);
    assert_eq!(g.not_rendered_reason, Some("hidden"));
    assert!(g.legend.is_none());
}
