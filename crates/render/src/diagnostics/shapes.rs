use super::{expanded, union};
use crate::{OverlayShapeKind, Rect, arrow_head};
/// Extrema of the native overlay's painted geometry: SVG uses mitered rectangle
/// corners, ellipse extrema, and butt-capped line/arrow segments. No page or panel
/// clipping is inferred here; callers supply the actual output clip.
pub fn shape_bounds(frame: Rect, kind: OverlayShapeKind, width: f32) -> Rect {
    let width = width.max(0.5);
    let a = (frame.left, frame.top);
    let b = (frame.right(), frame.bottom());
    match kind {
        OverlayShapeKind::Rect | OverlayShapeKind::Ellipse => expanded(frame, width * 0.5),
        OverlayShapeKind::Line => segment_bounds(a, b, width),
        OverlayShapeKind::Arrow => {
            let [c, d] = arrow_head(a, b, 1.);
            union(
                segment_bounds(a, b, width),
                union(segment_bounds(b, c, width), segment_bounds(b, d, width)),
            )
        }
    }
}
fn segment_bounds(a: (f32, f32), b: (f32, f32), width: f32) -> Rect {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let length = dx.hypot(dy);
    let nx = dy.abs() / length * width * 0.5;
    let ny = dx.abs() / length * width * 0.5;
    Rect::new(
        a.0.min(b.0) - nx,
        a.1.min(b.1) - ny,
        dx.abs() + 2. * nx,
        dy.abs() + 2. * ny,
    )
}
