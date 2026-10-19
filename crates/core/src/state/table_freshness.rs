//! Freshness is derived from immutable provenance and current source revisions.
//! Undo restores those revisions, so there is no second mutable invalidation flag.
use super::{Dataset, DatasetId, DerivationKind, Document, TypedTableState};
use plotium_data::{RevisionReason, TableRevision};
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TableFreshnessState {
    Source,
    Current,
    Stale,
    Unavailable,
}

#[derive(Clone, Debug, Serialize)]
pub struct TableFreshness {
    pub status: TableFreshnessState,
    pub refreshable: bool,
    pub reason: String,
    pub sources: Vec<DatasetId>,
}

/// Manual edits retain the recipe in their ancestry, including previously
/// rebased edits. Follow parents, not history order (which can contain branches).
pub fn table_recipe_revision(typed: &TypedTableState) -> Result<&TableRevision, String> {
    let mut revision = &typed.envelope.revision;
    let mut seen = BTreeSet::new();
    while matches!(
        revision.reason,
        RevisionReason::ManualEdit | RevisionReason::Rebase
    ) {
        if !seen.insert(revision.id) {
            return Err("The table revision history contains a cycle.".into());
        }
        let parent = revision
            .parents
            .first()
            .ok_or("The edited table has no recorded base revision.")?;
        revision = typed
            .envelope
            .history
            .iter()
            .find(|r| r.id == *parent)
            .ok_or("The edited table's base revision is missing.")?;
    }
    if !matches!(
        revision.reason,
        RevisionReason::Transform | RevisionReason::Refresh
    ) || revision.operation.plan.is_none()
    {
        return Err("This table has no saved relational recipe to refresh.".into());
    }
    Ok(revision)
}

impl Document {
    pub fn table_freshness(&self, id: DatasetId) -> TableFreshness {
        self.table_freshness_inner(id, &mut BTreeSet::new())
    }

    fn table_freshness_inner(
        &self,
        id: DatasetId,
        visiting: &mut BTreeSet<DatasetId>,
    ) -> TableFreshness {
        let unavailable = |reason: String, sources: Vec<DatasetId>| TableFreshness {
            status: TableFreshnessState::Unavailable,
            refreshable: false,
            reason,
            sources,
        };
        let Some(table) = self.dataset_by_id(id).and_then(Dataset::as_table) else {
            return unavailable(
                "A source data table is missing or is no longer a table.".into(),
                vec![id],
            );
        };
        let Some(lineage) = &table.lineage else {
            return TableFreshness {
                status: TableFreshnessState::Source,
                refreshable: false,
                reason: "Source table; no saved relational recipe to refresh.".into(),
                sources: vec![],
            };
        };
        let sources = lineage.sources.clone();
        if lineage.kind != DerivationKind::RelationalTransform {
            return unavailable(
                "This derivation has no supported saved relational refresh recipe.".into(),
                sources,
            );
        }
        if !visiting.insert(id) {
            return unavailable("The source lineage contains a cycle.".into(), sources);
        }
        let result = (|| {
            let recipe = match table_recipe_revision(&table.typed_state) {
                Ok(recipe) => recipe,
                Err(reason) => return unavailable(reason, sources.clone()),
            };
            if sources.is_empty() {
                return unavailable(
                    "The recipe has no recorded source datasets.".into(),
                    sources.clone(),
                );
            }
            let mut stale = false;
            let mut matched = BTreeSet::new();
            for source in &sources {
                let Some(input) = self.dataset_by_id(*source).and_then(Dataset::as_table) else {
                    return unavailable(
                        format!("Source dataset {source} is missing or is no longer a table."),
                        sources.clone(),
                    );
                };
                let current = &input.typed_state.envelope.revision;
                let Some(pinned) = recipe
                    .operation
                    .inputs
                    .iter()
                    .find(|i| i.table == current.table_id)
                else {
                    return unavailable(
                        format!(
                            "Source dataset {source} no longer matches the saved recipe's table identity."
                        ),
                        sources.clone(),
                    );
                };
                matched.insert(pinned.table);
                let upstream = self.table_freshness_inner(*source, visiting);
                if matches!(
                    upstream.status,
                    TableFreshnessState::Stale | TableFreshnessState::Unavailable
                ) {
                    return TableFreshness {
                        status: TableFreshnessState::Stale,
                        refreshable: false,
                        reason: format!(
                            "Refresh source dataset {source} first: {}",
                            upstream.reason
                        ),
                        sources: sources.clone(),
                    };
                }
                stale |= current.id != pinned.revision
                    || current.snapshot.fingerprint != pinned.snapshot_fingerprint;
            }
            if matched.len() != recipe.operation.inputs.len() {
                return unavailable(
                    "The saved recipe has inputs missing from its source lineage.".into(),
                    sources.clone(),
                );
            }
            TableFreshness {
                status: if stale {
                    TableFreshnessState::Stale
                } else {
                    TableFreshnessState::Current
                },
                refreshable: true,
                reason: if stale {
                    "Source revisions changed; explicitly refresh this table's saved recipe."
                } else {
                    "The result matches its recorded source revisions."
                }
                .into(),
                sources: sources.clone(),
            }
        })();
        visiting.remove(&id);
        result
    }
}
