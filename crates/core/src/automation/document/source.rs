use crate::automation::resources::child_ref;
use crate::{
    automation::*,
    state::{Dataset, PlotiumApp, SeriesBinding},
};
use serde_json::{Value, json};

/// Report the provider's actual binding, never infer columns from display labels.
pub(super) fn describe(app: &PlotiumApp, series: &SeriesBinding) -> Value {
    let src = &series.source;
    let resource = ResourceRef::from(src.resource);
    let mut out = json!({"dataset":resource,"field":null,"field_id":src.field,"item_id":src.item,"status":"resolved","binding_mode":"field"});
    let Some(dataset) = app
        .doc
        .datasets
        .iter()
        .find(|d| d.resource_id() == src.resource)
    else {
        out["status"] = json!("missing_dataset");
        return out;
    };
    let Some(descriptor) = dataset.field_descriptor(src.field) else {
        out["status"] = json!("missing_field");
        return out;
    };
    out["field"] = json!(child_ref(src.resource, &descriptor.local_id, KIND_FIELD));
    out["field_name"] = json!(descriptor.name);
    out["units"] = json!(descriptor.units);
    out["field_capabilities"] = json!(descriptor.capabilities.iter().collect::<Vec<_>>());
    if let Some(item) = src.item
        && dataset
            .trace_collection(src.field)
            .and_then(|c| c.item(item))
            .is_none()
    {
        out["status"] = json!("missing_item");
        return out;
    }
    if !dataset.supports_encoding(src.field, &series.encoding) {
        out["status"] = json!("incompatible_encoding");
    }
    if let Dataset::Table(table) = dataset {
        out["binding_mode"] = json!("table_default");
        out["per_plot_explicit_binding_supported"] = json!(false);
        let schema = &table.typed_state.envelope.revision.snapshot.schema;
        let column = |id: plotium_data::ColumnId| json!({"resource":child_ref(src.resource,&id.to_string(),KIND_TABLE_COLUMN),"column_id":id,"exists":schema.column(id).is_some()});
        out["x_column"] = json!(table.x_binding.map(column));
        // A table default field materializes its whole figure, not just the first
        // value column. R1 describes that expansion without changing its meaning.
        out["series_columns"]=json!(table.series_bindings.iter().map(|b|json!({"y_column":column(b.value_column),"error_column":b.uncertainty_column.map(column)})).collect::<Vec<_>>());
        if table.x_binding.is_none() || table.series_bindings.is_empty() {
            out["status"] = json!("unbound");
        } else if table
            .x_binding
            .is_some_and(|id| schema.column(id).is_none())
            || table.series_bindings.iter().any(|b| {
                schema.column(b.value_column).is_none()
                    || b.uncertainty_column
                        .is_some_and(|id| schema.column(id).is_none())
            })
        {
            out["status"] = json!("missing_column");
        }
    }
    out
}
