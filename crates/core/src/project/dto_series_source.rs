use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SeriesSourceDto {
    Field {
        input: String,
        field: u64,
    },
    TableColumns {
        input: String,
        field: u64,
        columns: crate::state::TableCurveColumns,
    },
    TraceItem {
        input: String,
        field: u64,
        item: plotium_data::TraceItemId,
    },
}

impl SeriesSourceDto {
    pub fn parts(&self) -> (&str, u64, crate::state::SeriesSelection) {
        match self {
            Self::Field { input, field } => {
                (input, *field, crate::state::SeriesSelection::FieldDefault)
            }
            Self::TableColumns {
                input,
                field,
                columns,
            } => (
                input,
                *field,
                crate::state::SeriesSelection::TableColumns(*columns),
            ),
            Self::TraceItem { input, field, item } => (
                input,
                *field,
                crate::state::SeriesSelection::TraceItem(*item),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tagged_source_rejects_unknown_fields_and_malformed_variants() {
        assert!(
            serde_json::from_value::<SeriesSourceDto>(serde_json::json!({
                "kind": "field", "input": "recipe_x", "field": 1, "item": "extra"
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<SeriesSourceDto>(serde_json::json!({
                "kind": "trace_item", "input": "recipe_x", "field": 1
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<SeriesSourceDto>(serde_json::json!({
                "kind": "unknown", "input": "recipe_x", "field": 1
            }))
            .is_err()
        );
    }
}
