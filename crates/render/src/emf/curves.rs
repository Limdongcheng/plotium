use super::*;
use crate::curve_style::{dashed_segments, marker_polygon, marker_segments};
use plotium_figure::{LineStyle, Marker, Series};
pub(super) fn write(dc: &mut Dc, proj: &Projector<'_>, series: &Series) {
    if series.points.is_empty() {
        return;
    }
    let points: Vec<_> = series.points.iter().map(|p| proj.project(*p)).collect();
    match series.kind {
        SeriesKind::Line if points.len() >= 2 => stroke(dc, &points, series, proj.plot),
        SeriesKind::Stick => {
            let segments = series
                .points
                .iter()
                .map(|p| [proj.project([p[0], 0.]), proj.project(*p)])
                .collect::<Vec<_>>();
            dc.segments(&segments, series.color, series.width);
            return;
        }
        _ => {}
    }
    let marker = series.appearance.marker(series.kind);
    if marker != Marker::None {
        for p in points {
            marker_at(
                dc,
                p,
                marker,
                series.appearance.marker_radius(series.width),
                series.color,
                series.width,
            );
        }
    }
}
pub(super) fn stroke(dc: &mut Dc, points: &[(f32, f32)], series: &Series, clip: Rect) {
    if series.appearance.line_style == LineStyle::Solid {
        dc.polyline(points, series.color, series.width);
    } else {
        dc.segments(
            &dashed_segments(points, series.appearance.line_style, series.width, clip),
            series.color,
            series.width,
        );
    }
}
pub(super) fn marker_at(
    dc: &mut Dc,
    p: (f32, f32),
    marker: Marker,
    r: f32,
    color: Color,
    width: f32,
) {
    if marker == Marker::Circle {
        dc.ellipse(
            Rect::new(p.0 - r, p.1 - r, r * 2., r * 2.),
            Some(color),
            None,
        );
    }
    let polygon = marker_polygon(marker, p, r);
    if !polygon.is_empty() {
        dc.polygon(&polygon, Some(color), None);
    }
    dc.segments(&marker_segments(marker, p, r), color, width);
}
