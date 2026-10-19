//! Data-only relational transforms; computation and document adoption are separate.
mod schema;
mod validation;
use super::*;
use crate::{actions::Action, state::*};
use plotium_data::{DataError, RelPlanV1, TableId};
pub use schema::{table_transform_limits, table_transform_schema};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeSet, sync::atomic::AtomicBool};

pub const TABLE_TRANSFORM_MEMORY: u64 = 512 * 1024 * 1024;
pub const TABLE_TRANSFORM_PREPARED: usize = 4;
pub const TABLE_TRANSFORM_PLAN_BYTES: usize = 64 * 1024;
pub const TABLE_TRANSFORM_NODES: usize = 64;
pub const TABLE_TRANSFORM_DEPTH: usize = 16;
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TableTransformMode {
    TableDataOnly,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TableTransformParams {
    pub mode: TableTransformMode,
    pub name: String,
    pub plan: RelPlanV1,
    #[serde(default = "default_memory")]
    pub memory_limit_bytes: u64,
}
fn default_memory() -> u64 {
    TABLE_TRANSFORM_MEMORY
}
pub fn is_table_transform(value: &Value) -> bool {
    value.get("mode").is_some()
}
fn invalid(message: impl Into<String>) -> AutomationError {
    AutomationError::InvalidParameters {
        tool_id: "data.transform".into(),
        message: message.into(),
    }
}
fn execution(error: DataError) -> AutomationError {
    match error {
        DataError::Cancelled => AutomationError::TableTransformCancelled,
        DataError::MemoryBudget { .. } => AutomationError::TableTransformBudget(error.to_string()),
        _ => AutomationError::Execution(error.to_string()),
    }
}
/// Validate the wire value before deserializing its recursive native IR.
pub fn parse_table_transform(value: Value) -> Result<TableTransformParams, AutomationError> {
    validation::validate_wire(&value)?;
    let params: TableTransformParams = super::registry::parse("data.transform", value)?;
    validation::validate_params(&params)?;
    Ok(params)
}
/// The connection's bounded recipe subset; native executors remain unrestricted.
pub(super) fn validate_connection_plan(plan: &RelPlanV1) -> Result<(), AutomationError> {
    validation::validate_plan(plan)
}
pub struct TableTransformSnapshot {
    params: TableTransformParams,
    input_id: DatasetId,
    input: TypedTableState,
}
impl TableTransformSnapshot {
    pub fn capture(app: &PlotiumApp, request: &ToolRequest) -> Result<Self, AutomationError> {
        if request.tool_id != "data.transform" {
            return Err(invalid("Expected data.transform."));
        }
        if request.expected_revision.0 != app.doc.automation_revision {
            return Err(AutomationError::StaleRevision {
                expected: request.expected_revision.0,
                actual: app.doc.automation_revision,
            });
        }
        let params = parse_table_transform(request.parameters.clone())?;
        let TargetSelector::Explicit { ids } = &request.targets else {
            return Err(invalid("Supply exactly one explicit DatasetId."));
        };
        if ids.len() != 1 {
            return Err(invalid("Supply exactly one explicit DatasetId."));
        }
        let input_id: DatasetId = ids[0].parse().map_err(|_| invalid("Invalid DatasetId."))?;
        let input = app
            .doc
            .dataset_by_id(input_id)
            .and_then(Dataset::as_table)
            .ok_or_else(|| invalid("Input must be an existing native typed table."))?
            .typed_state
            .clone();
        validation::validate_input(&params.plan, &input)?;
        Ok(Self {
            params,
            input_id,
            input,
        })
    }
    pub fn prepare(self, cancel: &AtomicBool) -> Result<PreparedTableTransform, AutomationError> {
        let mut typed = execute_typed_plan_cancellable(
            self.params.plan.clone(),
            &[&self.input],
            TableId::new(),
            self.params.memory_limit_bytes,
            &BTreeSet::new(),
            cancel,
        )
        .map_err(execution)?;
        typed.envelope.revision.operation.parameters.insert(
            "memory_limit_bytes".into(),
            json!(self.params.memory_limit_bytes),
        );
        let mut table = TableDataset::from_typed(typed);
        table.name = Some(self.params.name.clone());
        table.lineage = Some(DatasetLineage::new(
            DerivationKind::RelationalTransform,
            [self.input_id],
        ));
        let prepared = PreparedTableTransform {
            params: self.params,
            input_id: self.input_id,
            dataset: Dataset::Table(Box::new(table)),
        };
        // Refuse an unreportable candidate before document mutation is possible.
        let summary = prepared.preflight()?;
        if serde_json::to_vec(&summary)
            .map_err(|e| invalid(e.to_string()))?
            .len()
            > TABLE_RESPONSE_BYTES / 2
        {
            return Err(AutomationError::ResponseTooLarge);
        }
        Ok(prepared)
    }
}
/// A native candidate owned by a one-use invocation, not persisted parallel state.
pub struct PreparedTableTransform {
    params: TableTransformParams,
    input_id: DatasetId,
    dataset: Dataset,
}
impl PreparedTableTransform {
    /// Reserve the declared budget, not merely the encoded snapshot size.
    pub fn reserved_bytes(&self) -> u64 {
        self.params.memory_limit_bytes
    }
    fn output_metadata(&self) -> Result<Value, AutomationError> {
        let mut metadata = super::table_read::table_metadata(&self.dataset)?;
        metadata["provenance"] = super::table_provenance::dataset_metadata(&self.dataset);
        Ok(metadata)
    }
    pub fn preflight(&self) -> Result<Value, AutomationError> {
        Ok(json!({"status":"prepared", "output":self.output_metadata()?}))
    }
    pub(super) fn commit(
        mut self,
        app: &mut PlotiumApp,
        plan: &ToolPlan,
    ) -> Result<ToolResult, AutomationError> {
        let current = TableTransformSnapshot::capture(app, &plan.request)?;
        if current.params != self.params || current.input_id != self.input_id {
            return Err(invalid(
                "Prepared transform belongs to a different invocation.",
            ));
        }
        let value = json!({"status":"succeeded", "output":self.output_metadata()?});
        let mut result = super::tool_executors::readonly_value(
            "data.transform",
            plan.frozen_targets.revision,
            value,
        );
        result.produced = vec![self.dataset.resource_id().into()];
        result.targets = plan
            .frozen_targets
            .targets
            .iter()
            .map(|r| TargetResult {
                target: TargetRef::resource(r.clone()),
                outcome: TargetOutcome::Succeeded,
                message: "Created native derived table data.".into(),
                skip_reason: None,
                fingerprints: vec![],
            })
            .collect();
        result.diagnostics = self
            .dataset
            .as_table()
            .expect("native table candidate")
            .typed_state
            .envelope
            .revision
            .operation
            .diagnostics
            .iter()
            .map(|d| d.message.clone())
            .collect();
        if serde_json::to_vec(&result)
            .map_err(|e| invalid(e.to_string()))?
            .len()
            > TABLE_RESPONSE_BYTES
        {
            return Err(AutomationError::ResponseTooLarge);
        }
        let table = self.dataset.as_table_mut().expect("native table candidate");
        let rect = table.board_rect_pt();
        table.board_pos = next_board_frame_pos(app, [rect.width, rect.height]);
        app.try_execute_action(Action::InsertTableData {
            dataset: Box::new(self.dataset),
        })
        .map_err(|e| AutomationError::Execution(e.to_string()))?;
        result.after_revision = DocumentRevision(app.doc.automation_revision);
        Ok(result)
    }
}
pub fn execute_prepared_table_transform(
    app: &mut PlotiumApp,
    plan: ToolPlan,
    prepared: PreparedTableTransform,
    authority: ExecutionAuthority,
) -> Result<ToolResult, AutomationError> {
    let run = PreparedToolRun::begin(app, &plan);
    let outcome = if authority < ExecutionAuthority::ReversibleModify {
        Err(AutomationError::InsufficientAuthority {
            granted: authority,
            required: ExecutionAuthority::ReversibleModify,
        })
    } else {
        prepared.commit(app, &plan)
    };
    run.finish(app, &outcome)?;
    outcome
}
#[cfg(test)]
#[path = "table_transform_tests.rs"]
mod tests;
