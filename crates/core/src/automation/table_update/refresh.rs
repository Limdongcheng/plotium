//! Connection eligibility and output guards, not restrictions on native refresh.
use super::*;
use plotium_data::{CodecRegistry, SnapshotReader};

#[derive(Serialize)]
pub(in crate::automation) struct ConnectionTableFreshness {
    #[serde(flatten)]
    pub native: TableFreshness,
    pub mcp_refreshable: bool,
    pub mcp_refresh_reason: String,
}

pub(in crate::automation) fn connection_table_freshness(
    app: &PlotiumApp,
    id: DatasetId,
) -> ConnectionTableFreshness {
    let native = app.doc.table_freshness(id);
    let eligible = if !native.refreshable {
        Err(native.reason.clone())
    } else {
        app.doc
            .dataset_by_id(id)
            .and_then(Dataset::as_table)
            .ok_or_else(|| "The refresh table is missing.".to_owned())
            .and_then(|table| table_recipe_revision(&table.typed_state))
            .and_then(|recipe| {
                let plan = recipe.operation.plan.as_ref().ok_or("No saved recipe.")?;
                super::super::table_transform::validate_connection_plan(plan).map_err(|e| {
                    format!("MCP refresh supports only the single-source snapshot_read/project/filter/stable_sort/computed_column data-only subset; this native recipe is not exposed: {e}")
                })
            })
    };
    ConnectionTableFreshness {
        native,
        mcp_refreshable: eligible.is_ok(),
        mcp_refresh_reason: eligible.err().unwrap_or_else(|| {
            "Saved recipe is eligible for MCP refresh; Read and modify permission is required."
                .into()
        }),
    }
}

pub(super) fn validate_output(
    before: &TypedTableState,
    after: &TypedTableState,
    inputs: &[TypedTableState],
    cancel: &AtomicBool,
) -> Result<(), AutomationError> {
    let old = &before.envelope.revision;
    let output = &after.envelope.revision;
    if output.table_id != old.table_id || output.snapshot.table_id != old.snapshot.table_id {
        return Err(invalid(TABLE_REFRESH, "MCP refresh would change TableId."));
    }
    if output.snapshot.schema != old.snapshot.schema {
        return Err(invalid(
            TABLE_REFRESH,
            "MCP refresh would change ColumnId or the existing column contract; existing bindings must remain valid.",
        ));
    }
    let [input] = inputs else {
        return Err(invalid(
            TABLE_REFRESH,
            "MCP refresh requires one source table.",
        ));
    };
    let recipe = table_recipe_revision(after).map_err(|e| invalid(TABLE_REFRESH, e))?;
    let source = &input.envelope.revision;
    let corrupt = || {
        invalid(
            TABLE_REFRESH,
            "MCP refresh output does not retain its source table revision and RowIds.",
        )
    };
    if recipe.operation.inputs.len() != 1
        || recipe.operation.inputs[0].table != source.table_id
        || recipe.operation.inputs[0].revision != source.id
        || recipe.operation.inputs[0].snapshot_fingerprint != source.snapshot.fingerprint
    {
        return Err(corrupt());
    }
    let codecs = CodecRegistry::with_arrow_ipc();
    let reader =
        SnapshotReader::new(&source.snapshot, input.store.as_ref(), &codecs).map_err(execution)?;
    let mut source_rows = Vec::new();
    for batch in 0..source.snapshot.batch_count() {
        cancelled(cancel)?;
        source_rows.extend(reader.read_row_ids(batch).map_err(execution)?.1);
    }
    // Filters may add/remove rows and sorting may reorder them. Every retained
    // RowId must still be an actual current source identity, not a new derived
    // identity. The native executor retains the position/provenance mapping.
    source_rows.sort_unstable();
    let reader =
        SnapshotReader::new(&output.snapshot, after.store.as_ref(), &codecs).map_err(execution)?;
    for batch in 0..output.snapshot.batch_count() {
        cancelled(cancel)?;
        for row in reader.read_row_ids(batch).map_err(execution)?.1 {
            if source_rows.binary_search(&row).is_err() {
                return Err(corrupt());
            }
        }
    }
    Ok(())
}
