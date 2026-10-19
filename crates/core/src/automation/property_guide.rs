//! Narrow vector input and renderer observations for native guides.
use super::*;
use serde_json::{Value, json};

pub(super) fn decode(value: &Value) -> Result<Option<[f64; 2]>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let p: [f64; 2] = serde_json::from_value(value.clone())
        .map_err(|_| "expected null or exactly two finite numbers in 0..=1".to_owned())?;
    if !p.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)) {
        return Err("origin requires two finite numbers in 0..=1".into());
    }
    Ok(Some(p))
}

pub(super) fn input_schema() -> Value {
    json!({"oneOf":[{"type":"null"},{"type":"array","minItems":2,"maxItems":2,
        "items":{"type":"number","minimum":0,"maximum":1}}],
        "description":"Origin within available legend travel, x right and y down. null resets both origin and placement to Auto."})
}

pub(super) fn geometry(
    app: &PlotiumApp,
    address: &PropertyAddress,
) -> Result<Option<Value>, AutomationError> {
    if !crate::properties::guide::is_guide_property(address.definition) {
        return Ok(None);
    }
    let g = crate::properties::guide::geometry(app, address)
        .map_err(|e| property_error(TOOL_INSPECT, e))?;
    let rect =
        |r: plotium_render::Rect| json!({"x":r.left,"y":r.top,"width":r.width,"height":r.height});
    Ok(Some(json!({"coordinate_space":"object_local_pt","scale":1,
        "measurement_basis":"renderer_layout_text_width_estimate",
        "plot_rect":rect(g.plot),"legend_rect":g.legend.map(rect),
        "color_scale_rect":g.color_scale.map(rect),"entry_count":g.entries,
        "not_rendered_reason":g.not_rendered_reason,"available_travel_pt":g.available_travel,
        "effective_position":g.effective_position,"fits_plot":g.fits_plot})))
}
