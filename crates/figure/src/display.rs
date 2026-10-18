//! Native curve appearance and axis display projections, shared by all renderers.
use crate::{FigureTypography, PositiveFiniteF32, SeriesKind};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
    DashDot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Marker {
    None,
    Circle,
    Square,
    Diamond,
    TriangleUp,
    TriangleDown,
    Plus,
    Cross,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CurveAppearance {
    pub line_style: LineStyle,
    /// No override retains the encoding kind's native marker.
    pub marker: Option<Marker>,
    /// Diameter in points. No override retains the native point radius (width).
    pub marker_size: Option<PositiveFiniteF32>,
}
impl CurveAppearance {
    pub fn marker(self, kind: SeriesKind) -> Marker {
        self.marker.unwrap_or(if kind == SeriesKind::Points {
            Marker::Circle
        } else {
            Marker::None
        })
    }
    pub fn marker_radius(self, width: f32) -> f32 {
        self.marker_size.map_or(width, |size| size.get() * 0.5)
    }
}

/// Affine coordinate-to-display conversion. Points and viewport stay in the
/// plot's coordinate system; ticks are generated in display space then mapped back.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AxisDisplay {
    pub scale: f64,
    pub offset: f64,
    pub unit: String,
}
impl AxisDisplay {
    pub fn to_display(&self, value: f64) -> f64 {
        value * self.scale + self.offset
    }
    pub fn to_coordinate(&self, value: f64) -> f64 {
        (value - self.offset) / self.scale
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FigureTypographyOverrides {
    pub tick_pt: Option<PositiveFiniteF32>,
    pub label_pt: Option<PositiveFiniteF32>,
    pub legend_pt: Option<PositiveFiniteF32>,
}
impl FigureTypographyOverrides {
    pub fn apply_to(self, typography: &mut FigureTypography) {
        if let Some(v) = self.tick_pt {
            typography.tick_pt = v.get();
        }
        if let Some(v) = self.label_pt {
            typography.label_pt = v.get();
        }
        if let Some(v) = self.legend_pt {
            typography.legend_pt = v.get();
        }
    }
}
