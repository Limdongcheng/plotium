use super::*;
use plotium_figure::{
    Axis, AxisDisplay, CurveAppearance, LineStyle, Marker, PositiveFiniteF32, Series,
};
#[test]
fn r6_display_ticks_convert_units_without_moving_projected_points() {
    // A valid scientific range remains meaningful below machine epsilon.
    let tiny = Axis::new("Signal", 0., 1e-20);
    assert_eq!(tiny.normalize(1e-20), 1.);
    assert_eq!(tiny.normalize(5e-21), 0.5);
    assert_eq!(axis_ticks_for(&tiny, 5).values.last(), Some(&1e-20));
    let mut axis = Axis::new("Current", 0., 10.);
    let base = axis_ticks_for(&axis, 5);
    axis.display = Some(AxisDisplay {
        scale: 0.001,
        offset: 0.,
        unit: "A".into(),
    });
    let shown = axis_ticks_for(&axis, 5);
    assert_eq!(base.values, shown.values);
    assert_ne!(base.labels, shown.labels);
    assert!(shown.labels.iter().any(|s| s.contains("0.01")));
    axis.min = 273.15;
    axis.max = 303.15;
    axis.display = Some(AxisDisplay {
        scale: 1.,
        offset: -273.15,
        unit: "°C".into(),
    });
    let ticks = axis_ticks_for(&axis, 4);
    assert!(ticks.labels.contains(&"0".to_owned()));
    assert!((ticks.values[0] - 273.15).abs() < 1e-10);
}
#[test]
fn r6_dash_segments_are_page_bounded_and_preserve_gaps() {
    let clip = Rect::new(0., 0., 100., 100.);
    for style in [LineStyle::Dashed, LineStyle::Dotted, LineStyle::DashDot] {
        let segs = curve_style::dashed_segments(&[(-100., 50.), (200., 50.)], style, 1., clip);
        assert!(!segs.is_empty());
        assert!(segs.len() < 100);
        assert!(segs.iter().flatten().all(|p| p.0 >= -0.01 && p.0 <= 100.01));
        let enormous = curve_style::dashed_segments(&[(-1e30, 50.), (1e30, 50.)], style, 1., clip);
        assert!(enormous.len() < 100);
    }
}
#[test]
fn r6_native_svg_styles_and_legend_use_the_same_appearance() {
    let mut figure = Figure::new("", Axis::new("x", 0., 3.), Axis::new("y", 0., 3.));
    let mut series = Series::line("Authored", vec![[0.5, 1.], [1.5, 2.], [2.5, 1.]]);
    series.appearance = CurveAppearance {
        line_style: LineStyle::DashDot,
        marker: Some(Marker::Diamond),
        marker_size: Some(PositiveFiniteF32::new(6.).unwrap()),
    };
    series.width = 1.5;
    figure.series.push(series);
    figure.guide_visibility = plotium_figure::GuideVisibility::Show;
    let svg = svg::export(&figure);
    assert_eq!(svg.matches("stroke-dasharray").count(), 2);
    assert!(svg.matches("<polygon").count() >= 4);
    assert!(svg.contains("Authored"));
    let entries = legend_entries(&figure);
    assert!(matches!(entries[0].2, LegendMark::Curve { .. }));
    let mut unstyled = Series::line("Authored", vec![[0., 0.], [1., 1.]]);
    unstyled.color = figure.series[0].color;
    figure.series.push(unstyled);
    assert_eq!(
        legend_entries(&figure).len(),
        2,
        "Distinct appearance must not merge in either insertion order"
    );
    figure.series.swap(0, 1);
    assert_eq!(legend_entries(&figure).len(), 2);
    for marker in [
        Marker::Square,
        Marker::Diamond,
        Marker::TriangleUp,
        Marker::TriangleDown,
    ] {
        assert!(!curve_style::marker_polygon(marker, (0., 0.), 3.).is_empty());
    }
    for marker in [Marker::Plus, Marker::Cross] {
        assert_eq!(curve_style::marker_segments(marker, (0., 0.), 3.).len(), 2);
    }
}
