use super::*;
use crate::curve_style::{dash_pattern, marker_polygon, marker_segments};
use plotium_figure::{Color, LineStyle, Marker, Series, SeriesKind};

pub(super) fn write(s: &mut String, proj: &Projector<'_>, series: &Series) {
    if series.points.is_empty() {
        return;
    }
    if series.kind == SeriesKind::Stick {
        super::sticks::write(s, proj, series);
        return;
    }
    let points: Vec<_> = series.points.iter().map(|p| proj.project(*p)).collect();
    if series.kind == SeriesKind::Line && points.len() >= 2 {
        stroke(s, &points, series);
    }
    let marker = series.appearance.marker(series.kind);
    if marker != Marker::None {
        for p in points {
            marker_at(
                s,
                p,
                marker,
                series.appearance.marker_radius(series.width),
                series.color,
                series.width,
            );
        }
    }
}
pub(super) fn stroke(s: &mut String, points: &[(f32, f32)], series: &Series) {
    let pts = points
        .iter()
        .map(|(x, y)| format!("{x},{y}"))
        .collect::<Vec<_>>()
        .join(" ");
    let dash = if series.appearance.line_style == LineStyle::Solid {
        String::new()
    } else {
        format!(
            " stroke-dasharray=\"{}\"",
            dash_pattern(series.appearance.line_style, series.width)
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    let _ = write!(
        s,
        r#"<polyline points="{pts}" fill="none" stroke="{col}" stroke-width="{w}"{dash}/>"#,
        col = series.color.to_hex(),
        w = series.width
    );
}
pub(super) fn marker_at(
    s: &mut String,
    p: (f32, f32),
    marker: Marker,
    r: f32,
    color: Color,
    width: f32,
) {
    let col = color.to_hex();
    if marker == Marker::Circle {
        let _ = write!(
            s,
            r#"<circle cx="{x:.2}" cy="{y:.2}" r="{r:.2}" fill="{col}"/>"#,
            x = p.0,
            y = p.1
        );
    }
    let polygon = marker_polygon(marker, p, r);
    if !polygon.is_empty() {
        let pts = polygon
            .iter()
            .map(|(x, y)| format!("{x},{y}"))
            .collect::<Vec<_>>()
            .join(" ");
        let _ = write!(s, r#"<polygon points="{pts}" fill="{col}"/>"#);
    }
    for [a, b] in marker_segments(marker, p, r) {
        let _ = write!(
            s,
            r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{col}" stroke-width="{width}"/>"#,
            x1 = a.0,
            y1 = a.1,
            x2 = b.0,
            y2 = b.1
        );
    }
}
