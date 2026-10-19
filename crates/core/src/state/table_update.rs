//! Shared typed revision adoption for sheet edits and explicit refreshes.
use super::{Dataset, DatasetId, PlotiumApp, TableEditDelta, TypedTableState};
use plotium_data::{CellPatch, CodecRegistry, RevisionId, TableTransaction};
use std::collections::BTreeSet;

pub fn prepare_table_cells(
    before: &TypedTableState,
    edits: &[CellPatch],
) -> Result<TypedTableState, String> {
    let mut seen = BTreeSet::new();
    let mut transaction = TableTransaction::new(&before.envelope.revision);
    for edit in edits {
        if !seen.insert((edit.row, edit.column)) {
            return Err("A patch addresses the same cell more than once.".into());
        }
        transaction.set(edit.row, edit.column, edit.value.clone());
    }
    let revision = transaction
        .execute_and_commit(
            &before.envelope.revision,
            before.store.as_ref(),
            &CodecRegistry::with_arrow_ipc(),
            env!("CARGO_PKG_VERSION"),
        )
        .map_err(|e| e.to_string())?;
    // A no-op must neither invalidate analyses nor create an Undo step.
    if revision.snapshot.fingerprint == before.envelope.revision.snapshot.fingerprint {
        return Ok(before.clone());
    }
    let mut envelope = before.envelope.clone();
    envelope.advance(revision).map_err(|e| e.to_string())?;
    Ok(TypedTableState {
        envelope,
        store: before.store.clone(),
    })
}

impl PlotiumApp {
    pub fn commit_table_revision(
        &mut self,
        dataset: DatasetId,
        expected: RevisionId,
        after: TypedTableState,
    ) -> Result<(), String> {
        let table = self
            .doc
            .dataset_by_id(dataset)
            .and_then(Dataset::as_table)
            .ok_or("The data table no longer exists.")?;
        let current = &table.typed_state.envelope.revision;
        if current.id != expected {
            return Err("The table revision changed; prepare the edit again.".into());
        }
        if current.table_id != after.envelope.revision.table_id {
            return Err("The prepared revision belongs to another table.".into());
        }
        let delta = TableEditDelta::from_revision(table, after);
        self.try_execute_action(crate::actions::Action::edit_table(dataset, delta))
            .map_err(|e| e.to_string())
    }
}
