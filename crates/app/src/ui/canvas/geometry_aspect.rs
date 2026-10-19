use super::*;

pub(crate) fn preserve_aspect_frame(
    before: ObjectFrame,
    candidate: ObjectFrame,
    kind: ObjectDragKind,
) -> ObjectFrame {
    let ObjectDragKind::Resize(handle) = kind else {
        return candidate;
    };
    let [width, height] =
        plotium_core::state::aspect_locked_size(before, candidate, MIN_OBJECT_SIZE_PT);
    let right = before.x + before.width;
    let bottom = before.y + before.height;
    match handle {
        ResizeHandle::TopLeft => ObjectFrame::new(right - width, bottom - height, width, height),
        ResizeHandle::TopRight => ObjectFrame::new(before.x, bottom - height, width, height),
        ResizeHandle::BottomLeft => ObjectFrame::new(right - width, before.y, width, height),
        ResizeHandle::BottomRight => ObjectFrame::new(before.x, before.y, width, height),
    }
}
