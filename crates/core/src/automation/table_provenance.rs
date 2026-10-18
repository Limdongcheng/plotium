//! Bounded wire views of persisted native provenance, never a second recipe.
use super::*;
use crate::state::{Dataset, TableDataset};
use plotium_data::{BlockStore, DerivedRowMappingV1, Relation, RowId, RowMapping};
use serde_json::{Value, json};

pub(super) fn metadata(table: &TableDataset) -> Value {
    let operation = &table.typed_state.envelope.revision.operation;
    if operation.plan.is_none() {
        return Value::Null;
    }
    json!({"operation":operation,"source_datasets":table.lineage.as_ref().map(|l| &l.sources),
        "row_sources_supported":same_row_source(table),
        "row_sources_contract":"For supported single-source project/filter/stable_sort/computed_column plans, output RowId is the original input RowId. include_provenance returns pinned source revision and native input position per row."})
}
fn same_row_source(table: &TableDataset) -> bool {
    let operation = &table.typed_state.envelope.revision.operation;
    if operation.inputs.len() != 1 {
        return false;
    }
    let Some(plan) = &operation.plan else {
        return false;
    };
    let mut root = &plan.root;
    loop {
        match root {
            Relation::Project { input, .. }
            | Relation::Filter { input, .. }
            | Relation::StableSort { input, .. }
            | Relation::ComputedColumn { input, .. } => root = input,
            Relation::SnapshotRead(read) => {
                let input = &operation.inputs[0];
                return read.table == input.table
                    && read.revision == input.revision
                    && read.fingerprint == input.snapshot_fingerprint;
            }
            _ => return false,
        }
    }
}
pub(super) fn row_sources(
    table: &TableDataset,
    offset: u64,
    rows: &[RowId],
) -> Result<Vec<Value>, AutomationError> {
    let op = &table.typed_state.envelope.revision.operation;
    if op.plan.is_none() {
        return Ok(vec![json!([]); rows.len()]);
    }
    if !same_row_source(table) {
        return Err(AutomationError::InvalidParameters { tool_id: TABLE_READ_ROWS.into(), message:"Row provenance paging supports only single-source project/filter/stable_sort/computed_column plans.".into() });
    }
    let corrupt = || {
        AutomationError::Execution("Native row provenance does not match the output page.".into())
    };
    let derived = match &op.row_mapping {
        Some(RowMapping::Derived {
            mapping_block,
            codec,
        }) if codec == DerivedRowMappingV1::CODEC => {
            let bytes = table
                .typed_state
                .store
                .get(*mapping_block)
                .map_err(|e| AutomationError::Execution(e.to_string()))?;
            Some(
                DerivedRowMappingV1::decode(&bytes)
                    .map_err(|e| AutomationError::Execution(e.to_string()))?,
            )
        }
        _ => None,
    };
    let dataset = table.lineage.as_ref().and_then(|l| {
        if l.sources.len() == 1 {
            l.sources.first().copied()
        } else {
            None
        }
    });
    rows.iter().enumerate().map(|(i, row)| {
        let position = offset + i as u64;
        let input_position = match &op.row_mapping {
            Some(RowMapping::Identity) => position,
            Some(RowMapping::Selection { runs }) => {
                let mut remaining = position;
                let mut source = None;
                for (start, length) in runs {
                    if remaining < *length { source = start.checked_add(remaining); break; }
                    remaining -= length;
                }
                source.ok_or_else(corrupt)?
            }
            Some(RowMapping::Derived { .. }) => {
                let entry = derived.as_ref().and_then(|m| usize::try_from(position).ok().and_then(|i| m.outputs.get(i))).ok_or_else(corrupt)?;
                if entry.output != *row || entry.sources.len() != 1 { return Err(corrupt()); }
                let source = &entry.sources[0];
                if source.input != 0 || source.runs.len() != 1 || source.runs[0].1 != 1 { return Err(corrupt()); }
                source.runs[0].0
            }
            _ => return Err(corrupt()),
        };
        Ok(json!([{"dataset_id":dataset,"table_id":op.inputs[0].table,"table_revision":op.inputs[0].revision,"snapshot_fingerprint":op.inputs[0].snapshot_fingerprint,"row_id":row,"input_position":input_position}]))
    }).collect()
}

// Keep the dataset view as the source of identity; a missing source remains a
// persisted lineage edge rather than being silently rebound to another table.
pub(super) fn dataset_metadata(dataset: &Dataset) -> Value {
    dataset.as_table().map_or(Value::Null, metadata)
}
