use crate::{RasterFit, Rect};
/// The SVG backend crops to integer source pixels before applying quarter turns
/// and its native centered meet/slice transform. This measures that viewport;
/// alpha pixels and successful decoding are separate from geometry.
pub fn raster_bounds(
    frame: Rect,
    pixels: [u32; 2],
    crop: [f32; 4],
    turns: u8,
    fit: RasterFit,
) -> Option<Rect> {
    let [x0, y0, x1, y1] = crate::svg::document::crop_pixels(pixels, crop)?;
    let mut size = [(x1 - x0).max(1) as f32, (y1 - y0).max(1) as f32];
    if turns % 2 == 1 {
        size.swap(0, 1);
    }
    if fit == RasterFit::Stretch {
        return Some(frame);
    }
    let sx = frame.width / size[0];
    let sy = frame.height / size[1];
    let scale = if fit == RasterFit::Contain {
        sx.min(sy)
    } else {
        sx.max(sy)
    };
    let w = size[0] * scale;
    let h = size[1] * scale;
    Some(Rect::new(
        frame.left + (frame.width - w) * 0.5,
        frame.top + (frame.height - h) * 0.5,
        w,
        h,
    ))
}
