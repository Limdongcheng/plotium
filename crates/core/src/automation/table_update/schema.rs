use super::*;
fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
pub fn table_patch_schema() -> Value {
    let uuid = json!({"type":"string","format":"uuid"});
    let mut values = vec![object(json!({"kind":{"const":"null"}}), &["kind"])];
    for kind in [
        "boolean",
        "int64",
        "float64",
        "utf8",
        "categorical",
        "date",
        "time",
        "timestamp",
        "duration",
    ] {
        let value = match kind {
            "boolean" => json!({"type":"boolean"}),
            "float64" => json!({"oneOf":[{"type":"number"},{"enum":["NaN","+Inf","-Inf"]}]}),
            "utf8" => json!({"type":"string"}),
            "date" => json!({"type":"integer","minimum":i32::MIN,"maximum":i32::MAX}),
            "categorical" => json!({"type":"integer","minimum":0,"maximum":u32::MAX}),
            _ => {
                json!({"type":"string","pattern":"^-?[0-9]+$","description":"Exact signed 64-bit decimal string. Temporal values use nanoseconds; Date uses days since Unix epoch."})
            }
        };
        values.push(object(
            json!({"kind":{"const":kind},"value":value}),
            &["kind", "value"],
        ));
    }
    let mut result = object(
        json!({"table_revision":uuid,"edits":{"type":"array","minItems":1,"maxItems":TABLE_PATCH_CELLS,"items":object(json!({"row_id":uuid,"column_id":uuid,"value":{"oneOf":values}}), &["row_id","column_id","value"])}}),
        &["table_revision", "edits"],
    );
    result["description"] = json!(
        "One DatasetId in top-level ids. Copy actual RowId/ColumnId from table reads. Values use the same tagged kinds as read_rows; exact type, nullability and all identities are validated atomically. No source file write, new rows, schema edit or automatic refresh."
    );
    result
}
pub fn table_refresh_schema() -> Value {
    let mut schema = object(
        json!({"table_revision":{"type":"string","format":"uuid"},"memory_limit_bytes":{"type":"integer","minimum":1,"maximum":TABLE_TRANSFORM_MEMORY}}),
        &["table_revision"],
    );
    schema["description"] = json!(
        "One DatasetId in top-level ids. Explicitly rerun a saved single-source snapshot_read/project/filter/stable_sort/computed_column recipe (the data-only subset) on current source revisions. Other native recipes, including Pivot, are not exposed by MCP refresh. Inspect freshness.mcp_refreshable and mcp_refresh_reason separately from native refreshable. Prepared output must preserve Table/Column contracts and source RowIds before adoption, keeping existing plots bound. Refresh stale upstream tables first. Optional memory budget resolves explicit input, saved recipe budget, then 512 MiB. Cell edits are rebased by RowId; unresolved conflicts reject the refresh."
    );
    schema
}
pub fn table_update_limits() -> Value {
    json!({"patch_cells":TABLE_PATCH_CELLS,"parameter_bytes":TABLE_TRANSFORM_PLAN_BYTES,"prepared_summary_bytes":TABLE_RESPONSE_BYTES/2,"response_bytes":TABLE_RESPONSE_BYTES,"in_flight_shared_with_transform":1,"prepared_plans_shared_with_transform":TABLE_TRANSFORM_PREPARED,"pending_reserved_bytes_shared_with_transform":TABLE_TRANSFORM_MEMORY,"patch_reservation_bytes":TABLE_TRANSFORM_PLAN_BYTES})
}
