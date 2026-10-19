//! Bounded cell patches and explicit saved-recipe refreshes. Workers only own snapshots.
mod refresh;
mod schema;
use super::*;
use crate::state::*;
use plotium_data::{CellPatch, ColumnId, FiniteOrSpecial, LiteralValue, RevisionId, RowId};
pub(super) use refresh::connection_table_freshness;
pub use schema::{table_patch_schema, table_refresh_schema, table_update_limits};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicBool, Ordering},
};

pub const TABLE_PATCH: &str = "table.patch";
pub const TABLE_REFRESH: &str = "table.refresh";
pub const TABLE_PATCH_CELLS: usize = 1000;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum CellValue {
    Null,
    Boolean(bool),
    Int64(String),
    Float64(Value),
    Utf8(String),
    Categorical(u32),
    Date(i32),
    Time(String),
    Timestamp(String),
    Duration(String),
}
impl CellValue {
    fn literal(&self) -> Result<LiteralValue, AutomationError> {
        let integer = |text: &str| {
            text.parse::<i64>().map_err(|_| {
                invalid(
                    TABLE_PATCH,
                    "Expected an exact signed 64-bit decimal string.",
                )
            })
        };
        Ok(match self {
            Self::Null => LiteralValue::Null,
            Self::Boolean(v) => LiteralValue::Boolean(*v),
            Self::Int64(v) => LiteralValue::Int64(integer(v)?),
            Self::Float64(v) => LiteralValue::Float64(match v {
                Value::Number(n) => FiniteOrSpecial::new(
                    n.as_f64()
                        .ok_or_else(|| invalid(TABLE_PATCH, "Invalid Float64 number."))?,
                ),
                Value::String(v) => match v.as_str() {
                    "NaN" => FiniteOrSpecial::Nan,
                    "+Inf" => FiniteOrSpecial::PositiveInfinity,
                    "-Inf" => FiniteOrSpecial::NegativeInfinity,
                    _ => {
                        return Err(invalid(
                            TABLE_PATCH,
                            "Float64 strings must be NaN, +Inf or -Inf.",
                        ));
                    }
                },
                _ => {
                    return Err(invalid(
                        TABLE_PATCH,
                        "Float64 requires a number or NaN/+Inf/-Inf.",
                    ));
                }
            }),
            Self::Utf8(v) => LiteralValue::Utf8(v.clone()),
            Self::Categorical(v) => LiteralValue::Categorical(*v),
            Self::Date(v) => LiteralValue::Date(*v),
            Self::Time(v) => LiteralValue::Time(integer(v)?),
            Self::Timestamp(v) => LiteralValue::Timestamp(integer(v)?),
            Self::Duration(v) => LiteralValue::Duration(integer(v)?),
        })
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CellEdit {
    pub row_id: RowId,
    pub column_id: ColumnId,
    pub value: CellValue,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TablePatchParams {
    pub table_revision: RevisionId,
    pub edits: Vec<CellEdit>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TableRefreshParams {
    pub table_revision: RevisionId,
    /// Omission resolves the saved invocation budget before the default.
    pub memory_limit_bytes: Option<u64>,
}
fn invalid(tool: &str, message: impl Into<String>) -> AutomationError {
    AutomationError::InvalidParameters {
        tool_id: tool.into(),
        message: message.into(),
    }
}
fn execution(error: plotium_data::DataError) -> AutomationError {
    match error {
        plotium_data::DataError::Cancelled => AutomationError::TableTransformCancelled,
        plotium_data::DataError::MemoryBudget { .. } => {
            AutomationError::TableTransformBudget(error.to_string())
        }
        _ => AutomationError::Execution(error.to_string()),
    }
}
fn cancelled(cancel: &AtomicBool) -> Result<(), AutomationError> {
    if cancel.load(Ordering::Relaxed) {
        Err(AutomationError::TableTransformCancelled)
    } else {
        Ok(())
    }
}
pub fn is_table_operation(tool: &str) -> bool {
    matches!(tool, "data.transform" | TABLE_PATCH | TABLE_REFRESH)
}

pub struct TableUpdateSnapshot {
    request: ToolRequest,
    dataset: Dataset,
    work: UpdateWork,
    memory: u64,
}
enum UpdateWork {
    Patch(Vec<CellPatch>),
    Refresh(Vec<TypedTableState>),
}
impl TableUpdateSnapshot {
    pub fn capture(app: &PlotiumApp, request: &ToolRequest) -> Result<Self, AutomationError> {
        if request.expected_revision.0 != app.doc.automation_revision {
            return Err(AutomationError::StaleRevision {
                expected: request.expected_revision.0,
                actual: app.doc.automation_revision,
            });
        }
        let tool = request.tool_id.as_str();
        if serde_json::to_vec(&request.parameters)
            .map_err(|e| invalid(tool, e.to_string()))?
            .len()
            > TABLE_TRANSFORM_PLAN_BYTES
        {
            return Err(invalid(tool, "Parameters exceed 64 KiB."));
        }
        let TargetSelector::Explicit { ids } = &request.targets else {
            return Err(invalid(tool, "Supply exactly one DatasetId in ids."));
        };
        if ids.len() != 1 {
            return Err(invalid(tool, "Supply exactly one DatasetId in ids."));
        }
        let id: DatasetId = ids[0]
            .parse()
            .map_err(|_| invalid(tool, "Invalid DatasetId."))?;
        let dataset = app
            .doc
            .dataset_by_id(id)
            .ok_or_else(|| invalid(tool, "DatasetId does not exist."))?;
        let table = dataset
            .as_table()
            .ok_or_else(|| invalid(tool, "Dataset is not a native typed table."))?;
        let (revision, work, memory) = match tool {
            TABLE_PATCH => {
                if table.provenance.is_some() {
                    return Err(invalid(
                        tool,
                        "Live calculated tables are read-only; save an editable snapshot first.",
                    ));
                }
                let p: TablePatchParams = super::registry::parse(tool, request.parameters.clone())?;
                if p.edits.is_empty() || p.edits.len() > TABLE_PATCH_CELLS {
                    return Err(invalid(tool, "A patch requires 1–1000 cells."));
                }
                let edits = p
                    .edits
                    .iter()
                    .map(|e| {
                        Ok(CellPatch {
                            row: e.row_id,
                            column: e.column_id,
                            value: e.value.literal()?,
                        })
                    })
                    .collect::<Result<Vec<_>, AutomationError>>()?;
                (
                    p.table_revision,
                    UpdateWork::Patch(edits),
                    TABLE_TRANSFORM_PLAN_BYTES as u64,
                )
            }
            TABLE_REFRESH => {
                let p: TableRefreshParams =
                    super::registry::parse(tool, request.parameters.clone())?;
                let freshness = connection_table_freshness(app, id);
                if !freshness.mcp_refreshable {
                    return Err(invalid(tool, freshness.mcp_refresh_reason));
                }
                let recipe =
                    table_recipe_revision(&table.typed_state).map_err(|e| invalid(tool, e))?;
                let memory = p
                    .memory_limit_bytes
                    .or_else(|| {
                        recipe
                            .operation
                            .parameters
                            .get("memory_limit_bytes")
                            .and_then(Value::as_u64)
                    })
                    .unwrap_or(TABLE_TRANSFORM_MEMORY);
                if memory == 0 || memory > TABLE_TRANSFORM_MEMORY {
                    return Err(invalid(tool, "Memory budget must be 1–536870912 bytes."));
                }
                let inputs = freshness
                    .native
                    .sources
                    .iter()
                    .map(|source| {
                        app.doc
                            .dataset_by_id(*source)
                            .and_then(Dataset::as_table)
                            .map(|t| t.typed_state.clone())
                            .ok_or_else(|| invalid(tool, "A refresh input table is missing."))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                (p.table_revision, UpdateWork::Refresh(inputs), memory)
            }
            _ => return Err(invalid(tool, "Expected table.patch or table.refresh.")),
        };
        let actual = table.typed_state.envelope.revision.id;
        if revision != actual {
            return Err(AutomationError::StaleTableRevision {
                expected: revision.to_string(),
                actual: actual.to_string(),
            });
        }
        Ok(Self {
            request: request.clone(),
            dataset: dataset.clone(),
            work,
            memory,
        })
    }
    pub fn reserved_bytes(&self) -> u64 {
        self.memory
    }
    pub fn prepare(mut self, cancel: &AtomicBool) -> Result<PreparedTableUpdate, AutomationError> {
        cancelled(cancel)?;
        let table = self.dataset.as_table_mut().expect("captured typed table");
        let before = table.typed_state.envelope.revision.id;
        let after = match self.work {
            UpdateWork::Patch(edits) => prepare_table_cells(&table.typed_state, &edits)
                .map_err(|e| invalid(TABLE_PATCH, e))?,
            UpdateWork::Refresh(inputs) => {
                let refs = inputs.iter().collect::<Vec<_>>();
                let mut after = refresh_typed_plan_cancellable(
                    &table.typed_state,
                    &refs,
                    self.memory,
                    &BTreeSet::new(),
                    cancel,
                )
                .map_err(execution)?;
                refresh::validate_output(&table.typed_state, &after, &inputs, cancel)?;
                after
                    .envelope
                    .revision
                    .operation
                    .parameters
                    .insert("memory_limit_bytes".into(), json!(self.memory));
                after
            }
        };
        cancelled(cancel)?;
        if before != after.envelope.revision.id {
            table.curve_fit_analyses.clear();
            for binding in &mut table.series_bindings {
                binding.fit = None;
            }
        }
        table.typed_state = after;
        let prepared = PreparedTableUpdate {
            request: self.request,
            dataset: self.dataset,
            before,
            memory: self.memory,
        };
        if serde_json::to_vec(&prepared.preflight()?)
            .map_err(|e| invalid(&prepared.request.tool_id, e.to_string()))?
            .len()
            > TABLE_RESPONSE_BYTES / 2
        {
            return Err(AutomationError::ResponseTooLarge);
        }
        Ok(prepared)
    }
}
pub struct PreparedTableUpdate {
    request: ToolRequest,
    dataset: Dataset,
    before: RevisionId,
    memory: u64,
}
impl PreparedTableUpdate {
    fn metadata(&self) -> Result<Value, AutomationError> {
        let mut metadata = super::table_read::table_metadata(&self.dataset)?;
        metadata["provenance"] = super::table_provenance::dataset_metadata(&self.dataset);
        Ok(metadata)
    }
    pub fn preflight(&self) -> Result<Value, AutomationError> {
        Ok(
            json!({"status":"prepared", "output":self.metadata()?, "before_table_revision":self.before,
            "changed":self.before != self.dataset.as_table().expect("typed candidate").typed_state.envelope.revision.id}),
        )
    }
    fn commit(self, app: &mut PlotiumApp, plan: &ToolPlan) -> Result<ToolResult, AutomationError> {
        if self.request != plan.request {
            return Err(invalid(
                &plan.request.tool_id,
                "Prepared update belongs to another invocation.",
            ));
        }
        TableUpdateSnapshot::capture(app, &plan.request)?;
        let id = self.dataset.resource_id();
        let mut result = super::tool_executors::readonly_value(
            &plan.request.tool_id,
            plan.frozen_targets.revision,
            json!({"status":"succeeded", "output":self.metadata()?, "before_table_revision":self.before}),
        );
        let after = self
            .dataset
            .as_table()
            .expect("typed candidate")
            .typed_state
            .clone();
        let changed = after.envelope.revision.id != self.before;
        result.value["changed"] = json!(changed);
        result.targets = vec![TargetResult {
            target: TargetRef::resource(id.into()),
            outcome: if changed {
                TargetOutcome::Succeeded
            } else {
                TargetOutcome::Skipped
            },
            message: if changed {
                "Updated the existing native table."
            } else {
                "Cell values are unchanged."
            }
            .into(),
            skip_reason: None,
            fingerprints: vec![],
        }];
        if changed {
            result.modified.push(id.into());
        }
        if serde_json::to_vec(&result)
            .map_err(|e| invalid(&plan.request.tool_id, e.to_string()))?
            .len()
            > TABLE_RESPONSE_BYTES
        {
            return Err(AutomationError::ResponseTooLarge);
        }
        app.commit_table_revision(id, self.before, after)
            .map_err(AutomationError::Execution)?;
        result.after_revision = DocumentRevision(app.doc.automation_revision);
        result.value["freshness"] = json!(connection_table_freshness(app, id));
        Ok(result)
    }
}

/// Shared connection worker/pending-plan lifecycle for transforms, patches and refreshes.
pub enum TableOperationSnapshot {
    Transform(Box<TableTransformSnapshot>),
    Update(Box<TableUpdateSnapshot>),
}
impl TableOperationSnapshot {
    pub fn capture(app: &PlotiumApp, request: &ToolRequest) -> Result<Self, AutomationError> {
        if request.tool_id == "data.transform" {
            Ok(Self::Transform(Box::new(TableTransformSnapshot::capture(
                app, request,
            )?)))
        } else {
            Ok(Self::Update(Box::new(TableUpdateSnapshot::capture(
                app, request,
            )?)))
        }
    }
    pub fn prepare(self, cancel: &AtomicBool) -> Result<PreparedTableOperation, AutomationError> {
        match self {
            Self::Transform(s) => s.prepare(cancel).map(PreparedTableOperation::Transform),
            Self::Update(s) => s.prepare(cancel).map(PreparedTableOperation::Update),
        }
    }
}
pub enum PreparedTableOperation {
    Transform(PreparedTableTransform),
    Update(PreparedTableUpdate),
}
impl PreparedTableOperation {
    pub fn reserved_bytes(&self) -> u64 {
        match self {
            Self::Transform(p) => p.reserved_bytes(),
            Self::Update(p) => p.memory,
        }
    }
    pub fn preflight(&self) -> Result<Value, AutomationError> {
        match self {
            Self::Transform(p) => p.preflight(),
            Self::Update(p) => p.preflight(),
        }
    }
}
pub fn execute_prepared_table_operation(
    app: &mut PlotiumApp,
    plan: ToolPlan,
    prepared: PreparedTableOperation,
    authority: ExecutionAuthority,
) -> Result<ToolResult, AutomationError> {
    let run = PreparedToolRun::begin(app, &plan);
    let outcome = if authority < ExecutionAuthority::ReversibleModify {
        Err(AutomationError::InsufficientAuthority {
            granted: authority,
            required: ExecutionAuthority::ReversibleModify,
        })
    } else {
        match prepared {
            PreparedTableOperation::Transform(p) => p.commit(app, &plan),
            PreparedTableOperation::Update(p) => p.commit(app, &plan),
        }
    };
    run.finish(app, &outcome)?;
    outcome
}
pub(super) fn execute(
    app: &mut PlotiumApp,
    plan: &ToolPlan,
) -> Result<ToolResult, AutomationError> {
    TableUpdateSnapshot::capture(app, &plan.request)?
        .prepare(&AtomicBool::new(false))?
        .commit(app, plan)
}

#[cfg(test)]
#[path = "table_update_tests.rs"]
mod tests;
