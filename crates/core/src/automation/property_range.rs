//! JSON shape of the shared range intent. Catalog values remain transport-neutral.
use crate::properties::AxisRangeValue;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum AxisRangeDto {
    Auto,
    Manual {
        min: f64,
        max: f64,
        unit: Option<String>,
    },
}
impl From<&AxisRangeValue> for AxisRangeDto {
    fn from(value: &AxisRangeValue) -> Self {
        match value {
            AxisRangeValue::Auto => Self::Auto,
            AxisRangeValue::Manual { min, max, unit } => Self::Manual {
                min: *min,
                max: *max,
                unit: unit.clone(),
            },
        }
    }
}
impl From<AxisRangeDto> for AxisRangeValue {
    fn from(value: AxisRangeDto) -> Self {
        match value {
            AxisRangeDto::Auto => Self::Auto,
            AxisRangeDto::Manual { min, max, unit } => Self::Manual { min, max, unit },
        }
    }
}
pub(super) fn decode(value: &serde_json::Value) -> Result<AxisRangeValue, String> {
    if value.get("mode").and_then(|v| v.as_str()) == Some("manual") && value.get("unit").is_none() {
        return Err("Manual ranges require an explicit unit code or null.".into());
    }
    serde_json::from_value::<AxisRangeDto>(value.clone())
        .map(Into::into)
        .map_err(|e| e.to_string())
}
pub(super) fn input_schema() -> serde_json::Value {
    serde_json::json!({"oneOf":[
        {"type":"object","additionalProperties":false,"required":["mode"],"properties":{"mode":{"const":"auto"}}},
        {"type":"object","additionalProperties":false,"required":["mode","min","max","unit"],"properties":{"mode":{"const":"manual"},"min":{"type":"number"},"max":{"type":"number"},"unit":{"type":["string","null"]}},"description":"Finite min < max in an explicit compatible unit; null only for unknown coordinate units."}
    ]})
}
